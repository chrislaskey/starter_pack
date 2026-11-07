//! The posts commands: `app_core::posts` as the pages call it with `invoke`.
//! A command's name on the wire is resource first, `posts_list`, like the
//! page's `postsList`, since the names are one namespace across resources. A command's error is the
//! message the page shows.

use tauri::State;
use uuid::Uuid;

use app_core::database::Database;
use app_core::posts::{self, Post};

#[tauri::command(rename = "posts_list")]
pub async fn list(database: State<'_, Database>) -> Result<Vec<Post>, String> {
    posts::list(&database)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename = "posts_get")]
pub async fn get(database: State<'_, Database>, id: Uuid) -> Result<Post, String> {
    posts::get(&database, id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename = "posts_create")]
pub async fn create(database: State<'_, Database>, body: String) -> Result<Post, String> {
    posts::create(&database, &body)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename = "posts_update")]
pub async fn update(database: State<'_, Database>, id: Uuid, body: String) -> Result<Post, String> {
    posts::update(&database, id, &body)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command(rename = "posts_delete")]
pub async fn delete(database: State<'_, Database>, id: Uuid) -> Result<(), String> {
    posts::delete(&database, id)
        .await
        .map_err(|error| error.to_string())
}
