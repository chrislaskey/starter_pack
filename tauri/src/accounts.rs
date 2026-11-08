//! The accounts commands: signing up, in and out on this device, through
//! the web server, see `app_core::accounts`. A sign-in starts a sync. A
//! sign-out syncs first, so a change still waiting reaches the server;
//! what still has not answers as changes that would be lost, and the
//! page asks the person before signing out `losing_changes`.

use tauri::{AppHandle, State};

use app_core::accounts::{self, SignOut};
use app_core::deployment::Deployment;
use app_core::users::User;

use crate::sync::{self, SyncServer};

#[tauri::command]
pub async fn accounts_sign_up(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    server: State<'_, SyncServer>,
    email: String,
    password: String,
) -> Result<User, String> {
    let user = accounts::sign_up_on_device(&deployment, server.transport(), &email, &password)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(user)
}

#[tauri::command]
pub async fn accounts_sign_in(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    server: State<'_, SyncServer>,
    email: String,
    password: String,
) -> Result<User, String> {
    let user = accounts::sign_in_on_device(&deployment, server.transport(), &email, &password)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(user)
}

#[tauri::command]
pub async fn accounts_sign_out(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    server: State<'_, SyncServer>,
    losing_changes: bool,
) -> Result<SignOut, String> {
    // Reported by the sync itself; what it could not push is answered
    // as changes that would be lost.
    let _ = sync::run(&app).await;
    accounts::sign_out_on_device(&deployment, server.transport(), losing_changes)
        .await
        .map_err(|error| error.to_string())
}
