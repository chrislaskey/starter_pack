//! The posts API: `app_core::posts` as JSON.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use app_core::deployment::Deployment;
use app_core::posts::{self, Post, PostError};
use app_core::sync::SyncError;

/// What a request writes: the fields of a post the user fills in.
#[derive(Deserialize)]
pub struct PostFields {
    pub body: String,
}

pub async fn list(State(deployment): State<Deployment>) -> Result<Json<Vec<Post>>, ApiError> {
    Ok(Json(posts::list(&deployment).await?))
}

pub async fn get(
    State(deployment): State<Deployment>,
    Path(id): Path<Uuid>,
) -> Result<Json<Post>, ApiError> {
    Ok(Json(posts::get(&deployment, id).await?))
}

pub async fn create(
    State(deployment): State<Deployment>,
    Json(fields): Json<PostFields>,
) -> Result<(StatusCode, Json<Post>), ApiError> {
    let post = posts::create(&deployment, &fields.body).await?;
    Ok((StatusCode::CREATED, Json(post)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    Path(id): Path<Uuid>,
    Json(fields): Json<PostFields>,
) -> Result<Json<Post>, ApiError> {
    Ok(Json(posts::update(&deployment, id, &fields.body).await?))
}

pub async fn delete(
    State(deployment): State<Deployment>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    posts::delete(&deployment, id).await?;
    Ok(StatusCode::NO_CONTENT)
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
            PostError::BlankBody | PostError::Invalid(_) | PostError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            PostError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError {
            status,
            message: error.to_string(),
        }
    }
}

impl From<SyncError> for ApiError {
    fn from(error: SyncError) -> Self {
        match error {
            SyncError::Post(error) => error.into(),
            SyncError::Transport(_) | SyncError::Malformed(_) => ApiError {
                status: StatusCode::BAD_REQUEST,
                message: error.to_string(),
            },
            SyncError::Database(_) | SyncError::DeviceSettings(_) => ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: error.to_string(),
            },
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
