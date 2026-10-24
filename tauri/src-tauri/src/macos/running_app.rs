use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use serde::Serialize;

/// A running app, as `NSWorkspace` describes it.
#[derive(Clone, Debug, Serialize)]
pub struct RunningApp {
    pub pid: i32,
    pub name: String,
    pub bundle_id: Option<String>,
}

/// The app in front.
pub fn get_frontmost_app() -> Option<RunningApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    Some(RunningApp {
        pid: app.processIdentifier(),
        name: app
            .localizedName()
            .map(|name| name.to_string())
            .unwrap_or_default(),
        bundle_id: app.bundleIdentifier().map(|id| id.to_string()),
    })
}

/// Whether this app is in front. `NSApplication.isActive` stays true after
/// launch while another app is in front, so this asks the window server.
pub fn is_this_app_frontmost() -> bool {
    get_frontmost_app().is_some_and(|app| app.pid == std::process::id() as i32)
}

/// Brings an app to the front. Since macOS 14 the system may refuse, and
/// the result only says whether the request was accepted.
pub fn activate_app(pid: i32) -> bool {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .is_some_and(|app| app.activateWithOptions(NSApplicationActivationOptions::empty()))
}
