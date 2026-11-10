//! The quick picker where there is none: every platform but macOS.

use tauri::AppHandle;
use uuid::Uuid;

use super::{Item, ItemKind};

const UNAVAILABLE: &str = "The quick picker is not available on this device";

pub fn register(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

pub async fn list(_app: &AppHandle) -> Result<Vec<Item>, String> {
    Err(UNAVAILABLE.to_string())
}

pub async fn paste(_app: &AppHandle, _kind: ItemKind, _id: Uuid) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

pub fn close(_app: &AppHandle) {}

pub fn open_app(_app: &AppHandle) -> Result<(), String> {
    Err(UNAVAILABLE.to_string())
}

pub fn accessibility_request() -> bool {
    false
}
