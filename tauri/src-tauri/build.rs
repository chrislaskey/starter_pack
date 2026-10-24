fn main() {
    // Listing the app's commands generates an `allow-<command>` permission for
    // each, which the capabilities in `capabilities/` reference. The Phoenix
    // window is a remote origin, and remote origins may only call commands a
    // capability explicitly allows.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_connection_status",
        "set_api_token",
        "log_out",
        "open_phoenix",
        "api_request",
        "get_tmux_example_settings",
        "get_tmux_status",
        "create_tmux_session",
        "open_tmux_session",
        "get_quick_search_status",
        "request_accessibility_access",
        "open_quick_search",
        "close_quick_search",
        "list_quick_search_items",
        "paste_quick_search_item",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
