//! The posts commands: `app_core::posts` as the pages call it with `invoke`.
//! A command's error is the message the page shows.

use app_core::posts::{self, Post};

#[tauri::command]
pub fn list_posts() -> Vec<Post> {
    posts::list_posts()
}

#[tauri::command]
pub fn get_post(id: String) -> Result<Post, String> {
    posts::get_post(&id).map_err(|error| error.to_string())
}
