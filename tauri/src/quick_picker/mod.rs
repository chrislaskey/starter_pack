//! The quick picker: ⌃⌘T anywhere on the Mac opens a small window under
//! the text caret, listing the signed-in user's posts and images under
//! a filter box. Choosing one copies it to the clipboard, closes the
//! window and pastes it where the person was typing. With nobody signed
//! in the window says so and opens the app. macOS only, see `popup`;
//! elsewhere the commands answer that it is not available. See
//! `docs/design/quick-picker.md`.

#[cfg(target_os = "macos")]
mod items;
#[cfg(target_os = "macos")]
mod panel;
#[cfg(target_os = "macos")]
mod placement;
#[cfg(target_os = "macos")]
mod popup;
#[cfg(not(target_os = "macos"))]
#[path = "unavailable.rs"]
mod popup;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

pub use popup::register;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A post, pasted as its body.
    Text,
    /// A file that is an image the clipboard takes.
    Image,
}

/// A post or an image as the picker lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Item {
    pub kind: ItemKind,
    /// The post's or the file's.
    pub id: Uuid,
    /// The post's body, or the file's name.
    pub label: String,
    /// The image's size in bytes.
    pub size: Option<u64>,
}

/// The signed-in user's posts and images, newest first; an error when
/// nobody is signed in.
#[tauri::command]
pub async fn quick_picker_list(app: AppHandle) -> Result<Vec<Item>, String> {
    popup::list(&app).await
}

/// Copies the item, closes the picker and pastes into the app it opened
/// over.
#[tauri::command]
pub async fn quick_picker_paste(app: AppHandle, kind: ItemKind, id: Uuid) -> Result<(), String> {
    popup::paste(&app, kind, id).await
}

/// Closes the picker without pasting.
#[tauri::command]
pub fn quick_picker_close(app: AppHandle) {
    popup::close(&app);
}

/// Closes the picker and brings the app's window to the front, on its
/// accounts page, where a sign-in starts.
#[tauri::command]
pub fn quick_picker_open_app(app: AppHandle) -> Result<(), String> {
    popup::open_app(&app)
}

/// Asks macOS for Accessibility access when it isn't granted, which
/// opens the system dialog; answers whether it is granted now.
#[tauri::command]
pub fn quick_picker_accessibility_request() -> bool {
    popup::accessibility_request()
}
