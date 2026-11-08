//! The posts commands: `app_core::posts` as the pages call it with `invoke`,
//! for the user signed in on this device. A command is named resource
//! first, `posts_list`, like the page's `postsList`, since the commands
//! are one namespace across resources, and that is its name on the wire.
//! A command's error is the message the page shows. A write starts a
//! sync in the background once it is on the device, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::deployment::Deployment;
use app_core::posts::{self, Post, PostWithOwnerAndCommunities};

use crate::sync;
use crate::users::signed_in;

#[tauri::command]
pub async fn posts_list(
    deployment: State<'_, Deployment>,
) -> Result<Vec<PostWithOwnerAndCommunities>, String> {
    let user = signed_in(&deployment).await?;
    posts::list(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn posts_get(
    deployment: State<'_, Deployment>,
    id: Uuid,
) -> Result<PostWithOwnerAndCommunities, String> {
    let user = signed_in(&deployment).await?;
    posts::get(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn posts_create(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    body: String,
    community_ids: Vec<Uuid>,
) -> Result<Post, String> {
    let user = signed_in(&deployment).await?;
    let post = posts::create(&deployment, &user, &body, &community_ids)
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
    community_ids: Vec<Uuid>,
) -> Result<Post, String> {
    let user = signed_in(&deployment).await?;
    let post = posts::update(&deployment, &user, id, &body, &community_ids)
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
    let user = signed_in(&deployment).await?;
    posts::delete(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}
