//! The posts API: `app_core::posts` as JSON.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use uuid::Uuid;

use app_core::database::Database;
use app_core::posts::{self, Post, PostError};

pub async fn list_posts(State(database): State<Database>) -> Result<Json<Vec<Post>>, ApiError> {
    Ok(Json(posts::list_posts(&database).await?))
}

pub async fn get_post(
    State(database): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<Post>, ApiError> {
    Ok(Json(posts::get_post(&database, id).await?))
}

/// An error as the API answers it: a status and `{"error": "..."}`.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<PostError> for ApiError {
    fn from(error: PostError) -> Self {
        let status = match error {
            PostError::NotFound { .. } => StatusCode::NOT_FOUND,
            PostError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError {
            status,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!("{}", self.message);
        }
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
