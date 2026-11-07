//! The application: what the web server (`../axum`) and the Tauri desktop
//! and mobile app (`../tauri`) both run. One module per resource, with
//! `list_*`, `get_*`, `create_*`, `update_*` and `delete_*` functions that
//! return plain data the UI shows. The server and the app only translate
//! between their transport and these functions.

pub mod posts;
