//! The user signed in on this device, for every command that acts as
//! them, see `app_core::users::signed_in`, and as the pages ask for it:
//! an error, "Nobody is signed in", until someone does, see `accounts`.

use tauri::State;

use app_core::deployment::Deployment;
use app_core::users::{self, User};

pub async fn signed_in(deployment: &Deployment) -> Result<User, String> {
    users::signed_in(deployment)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn users_me(deployment: State<'_, Deployment>) -> Result<User, String> {
    signed_in(&deployment).await
}
