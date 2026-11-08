//! The user signed in on the active account, for every command that acts
//! as them, with the account's deployment, see `device_accounts`; an
//! error, "Nobody is signed in", until someone is active.

use tauri::State;

use app_core::deployment::Deployment;
use app_core::users::{self, User};

use crate::device_accounts::DeviceAccounts;

pub async fn signed_in(device: &DeviceAccounts) -> Result<(Deployment, User), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let user = users::signed_in(&deployment)
        .await
        .map_err(|error| error.to_string())?;
    Ok((deployment, user))
}

#[tauri::command]
pub async fn users_me(device: State<'_, DeviceAccounts>) -> Result<User, String> {
    Ok(signed_in(&device).await?.1)
}
