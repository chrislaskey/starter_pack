//! The app's SQLite file: `DATABASE_PATH` when set, as in dev with several
//! worktrees, else `app.db` in the app data directory Tauri gives the app
//! on each platform. Opened as the app starts and handed to the commands
//! as managed state.

use std::env;
use std::path::PathBuf;

use app_core::database::Database;
use tauri::{App, Manager};

pub fn open_and_manage(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let path = match env::var_os("DATABASE_PATH") {
        Some(path) => PathBuf::from(path),
        None => app.path().app_data_dir()?.join("app.db"),
    };
    let database = tauri::async_runtime::block_on(Database::open_sqlite_file(&path))?;
    log::info!("database {}", path.display());
    // The sample posts, until the pages can write posts
    if cfg!(debug_assertions) {
        tauri::async_runtime::block_on(app_core::posts::seed_sample_posts(&database))?;
    }
    app.manage(database);
    Ok(())
}
