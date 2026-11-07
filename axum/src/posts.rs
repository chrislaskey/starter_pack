//! The posts API: `app_core::posts` as JSON.

use axum::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use app_core::posts::{self, Post, PostError};

pub async fn list_posts() -> Json<Vec<Post>> {
    Json(posts::list_posts())
}

pub async fn get_post(Path(id): Path<String>) -> Result<Json<Post>, ApiError> {
    Ok(Json(posts::get_post(&id)?))
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
        };
        ApiError {
            status,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
