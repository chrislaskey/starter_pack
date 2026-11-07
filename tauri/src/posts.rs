//! The posts commands: `app_core::posts` as the pages call it with `invoke`.
//! A command's error is the message the page shows.

use tauri::State;
use uuid::Uuid;

use app_core::database::Database;
use app_core::posts::{self, Post};

#[tauri::command]
pub async fn list_posts(database: State<'_, Database>) -> Result<Vec<Post>, String> {
    posts::list_posts(&database)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_post(database: State<'_, Database>, id: Uuid) -> Result<Post, String> {
    posts::get_post(&database, id)
        .await
        .map_err(|error| error.to_string())
}
