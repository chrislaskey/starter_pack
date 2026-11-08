//! The app's SQLite file: `DATABASE_PATH` when set, as in dev with several
//! worktrees, else `app.db` in the app data directory Tauri gives the app
//! on each platform. Opened as the app starts, as a device that syncs
//! with the web server, its settings brought up to date and the signed-in
//! user's row written, and handed to the commands as managed state.

use std::env;
use std::path::PathBuf;

use app_core::database::Database;
use app_core::deployment::{Deployment, Role};
use app_core::device_settings;
use app_core::users;
use tauri::{App, Manager};

pub fn open_and_manage(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let path = match env::var_os("DATABASE_PATH") {
        Some(path) => PathBuf::from(path),
        None => app.path().app_data_dir()?.join("app.db"),
    };
    let database = tauri::async_runtime::block_on(Database::open_sqlite_file(&path))?;
    log::info!("database {}", path.display());
    let deployment = Deployment::new(database, Role::Device);
    let added = tauri::async_runtime::block_on(device_settings::upgrade(&deployment))?;
    log::info!("device settings up to date, {added} added");
    let user = tauri::async_runtime::block_on(async {
        let user = users::signed_in(&deployment).await?;
        users::create_if_missing(&deployment, &user).await
    })?;
    log::info!("signed in as {}", user.email);
    app.manage(deployment);
    Ok(())
}
