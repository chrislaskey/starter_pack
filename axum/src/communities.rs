//! The communities API: `app_core::communities` as JSON, for the
//! request's user.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use app_core::communities::{self, Community, CommunityAndMembership};
use app_core::deployment::Deployment;

use crate::api_error::ApiError;
use crate::current_user::CurrentUser;

#[derive(Deserialize)]
pub struct CommunityFields {
    pub name: String,
}

pub async fn list(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<CommunityAndMembership>>, ApiError> {
    Ok(Json(communities::list(&deployment, &user).await?))
}

pub async fn create(
    State(deployment): State<Deployment>,
    CurrentUser(user): CurrentUser,
    Json(fields): Json<CommunityFields>,
) -> Result<(StatusCode, Json<Community>), ApiError> {
    let community = communities::create(&deployment, &user, &fields.name).await?;
    Ok((StatusCode::CREATED, Json(community)))
}
