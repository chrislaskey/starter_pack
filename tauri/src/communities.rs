//! The communities commands: `app_core::communities` as the pages call
//! it, for the user signed in on this device. A write starts a sync.

use tauri::{AppHandle, State};

use app_core::communities::{self, Community, CommunityAndMembership};
use app_core::deployment::Deployment;

use crate::sync;
use crate::users::signed_in;

#[tauri::command]
pub async fn communities_list(
    deployment: State<'_, Deployment>,
) -> Result<Vec<CommunityAndMembership>, String> {
    let user = signed_in(&deployment).await?;
    communities::list(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn communities_create(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    name: String,
) -> Result<Community, String> {
    let user = signed_in(&deployment).await?;
    let community = communities::create(&deployment, &user, &name)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(community)
}
