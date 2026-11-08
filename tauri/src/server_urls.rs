//! Where the web server is, for the sync, and where its sign-in page
//! is, for the browser: from the environment, `SYNC_SERVER_URL` and
//! `BROWSER_SIGN_IN_URL`, which reaches the desktop app started from a
//! shell; else from `tauri.conf.json`, the `starter-pack` entry under
//! `plugins`, which is compiled into every build and which `bin/dev`
//! overrides with `--config` for the ports of the session, the only way
//! a value reaches a phone build, since no environment does; else the
//! dev server's port, and the sign-in page on the sync server itself,
//! where the pages are in production.

use std::env;

use tauri::AppHandle;

const CONFIG_ENTRY: &str = "starter-pack";
const DEFAULT_SYNC_SERVER_URL: &str = "http://localhost:4000";

/// The web server the sync runs against.
pub fn sync_server_url(app: &AppHandle) -> String {
    pick(
        env::var("SYNC_SERVER_URL").ok(),
        configured(app, "syncServerUrl"),
        DEFAULT_SYNC_SERVER_URL.to_string(),
    )
}

/// The web's sign-in page, which the app opens in the browser.
pub fn browser_sign_in_url(app: &AppHandle) -> String {
    pick(
        env::var("BROWSER_SIGN_IN_URL").ok(),
        configured(app, "browserSignInUrl"),
        format!("{}/accounts/sign-in", sync_server_url(app)),
    )
}

/// The environment's, else the config's, else the fallback; blanks
/// count as unset.
fn pick(from_environment: Option<String>, from_config: Option<String>, fallback: String) -> String {
    from_environment
        .into_iter()
        .chain(from_config)
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or(fallback)
}

fn configured(app: &AppHandle, key: &str) -> Option<String> {
    app.config()
        .plugins
        .0
        .get(CONFIG_ENTRY)?
        .get(key)?
        .as_str()
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_urls_pick_the_environment_then_the_config_then_the_fallback() {
        let fallback = || "http://localhost:4000".to_string();
        assert_eq!(
            pick(
                Some(" http://a ".into()),
                Some("http://b".into()),
                fallback()
            ),
            "http://a"
        );
        assert_eq!(
            pick(Some("  ".into()), Some("http://b".into()), fallback()),
            "http://b",
            "a blank environment value is unset"
        );
        assert_eq!(pick(None, None, fallback()), "http://localhost:4000");
    }
}
