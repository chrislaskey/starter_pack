//! The posts API: `app_core::posts` as JSON.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use app_core::database::Database;
use app_core::posts::{self, Post, PostError};

/// What a request writes: the fields of a post the user fills in.
#[derive(Deserialize)]
pub struct PostFields {
    pub body: String,
}

pub async fn list(State(database): State<Database>) -> Result<Json<Vec<Post>>, ApiError> {
    Ok(Json(posts::list(&database).await?))
}

pub async fn get(
    State(database): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<Post>, ApiError> {
    Ok(Json(posts::get(&database, id).await?))
}

pub async fn create(
    State(database): State<Database>,
    Json(fields): Json<PostFields>,
) -> Result<(StatusCode, Json<Post>), ApiError> {
    let post = posts::create(&database, &fields.body).await?;
    Ok((StatusCode::CREATED, Json(post)))
}

pub async fn update(
    State(database): State<Database>,
    Path(id): Path<Uuid>,
    Json(fields): Json<PostFields>,
) -> Result<Json<Post>, ApiError> {
    Ok(Json(posts::update(&database, id, &fields.body).await?))
}

pub async fn delete(
    State(database): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    posts::delete(&database, id).await?;
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
            PostError::BlankBody => StatusCode::UNPROCESSABLE_ENTITY,
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
