//! The ops app on the web server: its API under `/api/ops`, where every
//! route is for staff, `require_staff`; its pages, an entry of their
//! own in the Svelte build, `ops.html`, at `/ops` and under it; and the
//! commands that say who is staff, `app_web ops staff create <email>`
//! and `delete`. See `docs/design/multi-app.md`.

use std::path::Path;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, get_service};
use axum::{Extension, Json, Router};
use tower_http::services::ServeFile;

use app_core::deployment::Deployment;
use app_core::ops::staff::{self, Staff, StaffError};

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;
use crate::router::AppState;

/// The API's routes, each behind `require_staff`.
pub fn api(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/ops/current-staff", get(current_staff))
        .route_layer(middleware::from_fn_with_state(state, require_staff))
}

/// The ops pages: one HTML file whose router shows the page at the path.
pub fn pages(svelte_dist: &Path) -> Router {
    let page = get_service(ServeFile::new(svelte_dist.join("ops.html")));
    Router::new()
        .route("/ops", page.clone())
        .route("/ops/{*path}", page)
}

/// Lets the request through when it is from someone who is staff, with
/// their `Staff` for the handler: 401 from nobody, 403 from anyone else.
async fn require_staff(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let staff = staff::get(&deployment, &user).await?;
    request.extensions_mut().insert(staff);
    Ok(next.run(request).await)
}

/// `GET /api/ops/current-staff`: the staff row of the request's user.
async fn current_staff(Extension(staff): Extension<Staff>) -> Json<Staff> {
    Json(staff)
}

impl From<StaffError> for ApiError {
    fn from(error: StaffError) -> Self {
        let status = match error {
            StaffError::NotStaff => StatusCode::FORBIDDEN,
            StaffError::NoAccount { .. } => StatusCode::NOT_FOUND,
            StaffError::AccountUser(error) => return error.into(),
            StaffError::ServerOnly | StaffError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

/// `ops staff create <email>` and `ops staff delete <email>`.
pub async fn command(deployment: &Deployment, words: &[&str]) -> Result<(), String> {
    match words {
        ["staff", "create", email] => {
            staff::create(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is staff");
            Ok(())
        }
        ["staff", "delete", email] => {
            staff::delete(deployment, email)
                .await
                .map_err(|error| error.to_string())?;
            println!("{email} is staff no longer");
            Ok(())
        }
        _ => Err("commands: ops staff create <email>, ops staff delete <email>".to_string()),
    }
}
