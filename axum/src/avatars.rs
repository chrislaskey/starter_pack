//! The avatars API: `app_core::avatars` as JSON, for the request's user.
//! `POST /api/avatars/uploads` answers where to PUT the picture, and
//! `PUT /api/avatars` makes the uploaded file the user's picture. Both
//! answer 503 on a server with no bucket, see `storage`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use app_core::avatars::{self, AvatarUpdate, AvatarUpload, AvatarUploadNew};
use app_core::deployment::Deployment;
use app_core::storage::Storage;
use app_core::users::User;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

pub async fn create_upload(
    State(storage): State<Option<Storage>>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<AvatarUploadNew>,
) -> Result<(StatusCode, Json<AvatarUpload>), ApiError> {
    let storage = storage.ok_or_else(no_storage)?;
    let upload = avatars::create_upload(&storage, &user, &request).await?;
    Ok((StatusCode::CREATED, Json(upload)))
}

pub async fn update(
    State(deployment): State<Deployment>,
    State(storage): State<Option<Storage>>,
    CurrentUser(user): CurrentUser,
    Json(request): Json<AvatarUpdate>,
) -> Result<Json<User>, ApiError> {
    let storage = storage.ok_or_else(no_storage)?;
    Ok(Json(
        avatars::update(&deployment, &storage, &user, &request).await?,
    ))
}

fn no_storage() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "Uploads are not set up on this server",
    )
}
