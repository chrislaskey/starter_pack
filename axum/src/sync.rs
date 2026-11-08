//! The sync API: `app_core::sync`'s push and pull as JSON, what a device
//! calls to converge with this server, see `docs/design/offline-sync.md`.

use axum::Json;
use axum::extract::State;

use app_core::deployment::Deployment;
use app_core::sync::{self, Page, PullRequest, PushRequest, PushResponse};

use crate::posts::ApiError;

pub async fn push(
    State(deployment): State<Deployment>,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    Ok(Json(sync::push(&deployment, request).await?))
}

pub async fn pull(
    State(deployment): State<Deployment>,
    Json(request): Json<PullRequest>,
) -> Result<Json<Page>, ApiError> {
    Ok(Json(sync::pull(&deployment, request).await?))
}
