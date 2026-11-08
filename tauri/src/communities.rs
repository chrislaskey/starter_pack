//! The communities commands: `app_core::communities` as the pages call
//! it, for the user signed in on this device. A write starts a sync.

use tauri::{AppHandle, State};

use app_core::communities::{self, Community, CommunityAndMembership};

use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::users::signed_in;

#[tauri::command]
pub async fn communities_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<CommunityAndMembership>, String> {
    let (deployment, user) = signed_in(&device).await?;
    communities::list(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn communities_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    name: String,
) -> Result<Community, String> {
    let (deployment, user) = signed_in(&device).await?;
    let community = communities::create(&deployment, &user, &name)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(community)
}
