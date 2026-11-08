//! The posts commands: `app_core::posts` as the pages call it with `invoke`.
//! A command is named resource first, `posts_list`, like the page's
//! `postsList`, since the commands are one namespace across resources,
//! and that is its name on the wire. A command's error is the message the
//! page shows. A write starts a sync in the background once it is on the
//! device, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::deployment::Deployment;
use app_core::posts::{self, Post};

use crate::sync;

#[tauri::command]
pub async fn posts_list(deployment: State<'_, Deployment>) -> Result<Vec<Post>, String> {
    posts::list(&deployment)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn posts_get(deployment: State<'_, Deployment>, id: Uuid) -> Result<Post, String> {
    posts::get(&deployment, id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn posts_create(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    body: String,
) -> Result<Post, String> {
    let post = posts::create(&deployment, &body)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(post)
}

#[tauri::command]
pub async fn posts_update(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    id: Uuid,
    body: String,
) -> Result<Post, String> {
    let post = posts::update(&deployment, id, &body)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(post)
}

#[tauri::command]
pub async fn posts_delete(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    id: Uuid,
) -> Result<(), String> {
    posts::delete(&deployment, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}
