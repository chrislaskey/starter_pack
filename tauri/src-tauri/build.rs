use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    embed_client_migrations();

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
        "list_realms",
        "get_realm_page",
        "write_post",
        "edit_post",
        "sync_realm",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}

/// Writes `client_migrations.rs` into `OUT_DIR`: the SQLite migrations the
/// Phoenix server exports for clients (`phoenix/priv/client_migrations`,
/// see its README), each as `(version, name, sql)`, oldest first, so the
/// app applies them to a realm's file with `include_str!`.
fn embed_client_migrations() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let dir = Path::new(&manifest_dir).join("../../phoenix/priv/client_migrations");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect();
    files.sort();

    let mut source = String::from("pub const CLIENT_MIGRATIONS: &[(i64, &str, &str)] = &[\n");
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("a migration file has a utf-8 name");
        let version: i64 = name
            .split('_')
            .next()
            .and_then(|version| version.parse().ok())
            .unwrap_or_else(|| panic!("{name} does not start with a version"));
        let absolute = path
            .canonicalize()
            .unwrap_or_else(|error| panic!("could not resolve {}: {error}", path.display()));
        source.push_str(&format!(
            "    ({version}, {name:?}, include_str!({:?})),\n",
            absolute.display().to_string()
        ));
    }
    source.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set");
    fs::write(Path::new(&out_dir).join("client_migrations.rs"), source)
        .expect("could not write client_migrations.rs");
}
