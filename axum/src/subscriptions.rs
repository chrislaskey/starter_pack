//! The subscriptions API, `app_core::accounts::subscriptions` as JSON:
//! the request's user's plans, the current one and the ended ones.

use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use app_core::accounts::subscriptions::{self, SubscriptionShown};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

/// `GET /api/accounts/subscriptions`
pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<SubscriptionShown>>, ApiError> {
    Ok(Json(subscriptions::list(&deployment, &user).await?))
}

/// `GET /api/accounts/subscriptions/{id}`
pub async fn get(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionShown>, ApiError> {
    Ok(Json(subscriptions::get(&deployment, &user, id).await?))
}
