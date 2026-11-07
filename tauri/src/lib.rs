//! The desktop and mobile app: one window showing the Svelte pages
//! (`../svelte`), and a Tauri command per `app_core` function the pages
//! call, one module per resource. The app opens its own SQLite file, see
//! `database`, and the commands read and write it.

mod database;
mod posts;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            database::open_and_manage(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![posts::list_posts, posts::get_post])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
