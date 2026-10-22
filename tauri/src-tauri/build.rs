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
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
