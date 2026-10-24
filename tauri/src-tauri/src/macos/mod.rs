//! macOS calls Tauri has no API for, through the `objc2-app-kit`,
//! `core-foundation` and `core-graphics` bindings: the Accessibility API,
//! running apps, the pasteboard and keyboard events.

mod accessibility;
mod keyboard;
mod pasteboard;
mod running_app;

pub use accessibility::{
    TextBounds, enable_accessibility, get_focused_text_bounds, is_accessibility_trusted,
};
pub use keyboard::press_paste_shortcut;
pub use pasteboard::{ClipboardImage, copy_image_to_clipboard};
pub use running_app::{RunningApp, activate_app, get_frontmost_app, is_this_app_frontmost};

use serde::Serialize;

/// A rectangle in screen points with the origin at the top left of the main
/// display, the same space as Tauri's logical window positions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
