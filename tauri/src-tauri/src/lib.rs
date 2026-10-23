//! The Starter Pack desktop app.
//!
//! The app is a Tauri shell around the Phoenix server. It has two windows:
//!
//!   * `main`, a local page showing whether the websocket API is connected,
//!     with buttons to log in, open the web UI and make a test request
//!   * `phoenix`, the Phoenix web UI itself, loaded from the server
//!
//! Logging in happens in the Phoenix window. Once the user is logged in,
//! Phoenix's `/client/new` page hands the app an API token: it looks for a
//! `window.appClient` object on the page (see Phoenix's
//! `assets/js/app_client.js`), which this app injects into every page of the
//! Phoenix window, and calls its `setApiToken`, which runs the
//! `set_api_token` command. The app keeps the token (see `token`) and a
//! background task (see `phoenix`) holds a websocket connection to the API
//! with it for as long as the app runs.
//!
//! The main window also has a tmux example page, see `tmux_example`.

mod claude_code;
mod config;
mod git;
mod phoenix;
mod process;
mod terminal_app;
mod tmux;
mod tmux_example;
mod token;

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::{mpsc, oneshot};

use config::Config;
use phoenix::{Client, Control, Reply, Status};
use terminal_app::ShowOutcome;
use tmux_example::{DetectedSettings, Settings, TmuxStatus};
use token::TokenStore;

const MAIN_WINDOW: &str = "main";
const PHOENIX_WINDOW: &str = "phoenix";

/// Runs on every page of the Phoenix window before the page's own scripts.
/// Phoenix's `assets/js/app_client.js` looks for `window.appClient` to hand
/// the app an API token, so any client that provides it works the same way.
const APP_CLIENT_SCRIPT: &str = r#"
window.appClient = {
  setApiToken: (token) => window.__TAURI__.core.invoke("set_api_token", { token }),
};
"#;

pub struct AppState {
    config: Config,
    tokens: Arc<TokenStore>,
    status: Arc<Mutex<Status>>,
    control: mpsc::UnboundedSender<Control>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("desktop_lib=info"))
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let config = Config::from_env();
            let tokens = Arc::new(TokenStore::open(app.path().app_data_dir()?));
            let status = Arc::new(Mutex::new(Status {
                state: phoenix::State::Disconnected,
                detail: Some("Starting".into()),
                has_token: tokens.get().is_some(),
                server_url: config.server_url.to_string(),
            }));
            let (control, control_rx) = mpsc::unbounded_channel();

            let client = Client::new(
                app.handle().clone(),
                config.clone(),
                tokens.clone(),
                status.clone(),
                control_rx,
            );
            tauri::async_runtime::spawn(client.run());

            app.manage(AppState {
                config,
                tokens,
                status,
                control,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_connection_status,
            set_api_token,
            log_out,
            open_phoenix,
            api_request,
            get_tmux_example_settings,
            get_tmux_status,
            create_tmux_session,
            open_tmux_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// The current connection status, for the main window when it loads. Later
/// changes arrive as `connection-status` events.
#[tauri::command]
fn get_connection_status(state: State<'_, AppState>) -> Status {
    state.status.lock().unwrap().clone()
}

/// Stores the API token and connects with it. Called through
/// `window.appClient.setApiToken` by Phoenix's `/client/new` page in the
/// Phoenix window, the only remote origin allowed to call it (see
/// `capabilities/phoenix.json`).
#[tauri::command]
fn set_api_token(app: AppHandle, state: State<'_, AppState>, token: String) -> Result<(), String> {
    let token = token.trim();
    let base64url = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_';
    if token.is_empty() || token.len() > 256 || !token.bytes().all(base64url) {
        return Err("The token is not a base64url string".into());
    }

    state.tokens.set(token)?;
    notify_token_changed(&state)?;
    log::info!("Received an API token, connecting");

    if let Some(main) = app.get_webview_window(MAIN_WINDOW) {
        let _ = main.set_focus();
    }
    Ok(())
}

/// Deletes the API token on the server when connected, then forgets it
/// locally either way, which disconnects.
#[tauri::command]
async fn log_out(state: State<'_, AppState>) -> Result<(), String> {
    if state.tokens.get().is_some() {
        match request(&state.control, "auth.token.delete".into(), json!({})).await {
            Ok(_) => log::info!("Deleted the API token on the server"),
            Err(error) => log::warn!(
                "Could not delete the API token on the server, forgetting it locally only: {error}"
            ),
        }
    }

    state.tokens.clear()?;
    notify_token_changed(&state)
}

/// Shows a page of the Phoenix web UI in the Phoenix window, opening the
/// window when needed.
#[tauri::command]
fn open_phoenix(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    let url = state.config.page_url(path.as_deref().unwrap_or("/"))?;

    if let Some(window) = app.get_webview_window(PHOENIX_WINDOW) {
        window.navigate(url).map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, PHOENIX_WINDOW, WebviewUrl::External(url))
        .title("Starter Pack")
        .inner_size(1100.0, 800.0)
        .user_agent(&user_agent())
        .initialization_script(APP_CLIENT_SCRIPT)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Makes a request on the websocket API and returns its reply. `event` is a
/// request name like `auth.token.get`, see `App.Web.ApiChannel`.
#[tauri::command]
async fn api_request(state: State<'_, AppState>, event: String, payload: Option<Value>) -> Reply {
    request(&state.control, event, payload.unwrap_or_else(|| json!({}))).await
}

/// Best guesses for the tmux example page's settings form.
#[tauri::command]
async fn get_tmux_example_settings() -> DetectedSettings {
    tmux_example::detect_settings().await
}

/// The tmux sessions with what runs in each pane, and the Claude Code
/// sessions.
#[tauri::command]
async fn get_tmux_status(tmux_path: String) -> Result<TmuxStatus, String> {
    tmux_example::get_status(&tmux_path).await
}

/// Creates a tmux session in `directory`, types `command` into it and opens
/// it in the terminal app.
#[tauri::command]
async fn create_tmux_session(
    settings: Settings,
    name: String,
    directory: String,
    command: String,
) -> Result<ShowOutcome, String> {
    tmux_example::create_session(&settings, &name, &directory, &command).await
}

/// Brings a tmux session's terminal window to the front, or opens one.
#[tauri::command]
async fn open_tmux_session(settings: Settings, name: String) -> Result<ShowOutcome, String> {
    tmux_example::open_session(&settings, &name).await
}

async fn request(control: &mpsc::UnboundedSender<Control>, event: String, payload: Value) -> Reply {
    let (reply, reply_rx) = oneshot::channel();
    control
        .send(Control::Request {
            event,
            payload,
            reply,
        })
        .map_err(|_| json!({"reason": "client_stopped"}))?;
    reply_rx
        .await
        .map_err(|_| json!({"reason": "disconnected"}))?
}

fn notify_token_changed(state: &AppState) -> Result<(), String> {
    state
        .control
        .send(Control::TokenChanged)
        .map_err(|_| "The connection task stopped".to_string())
}

/// A browser-like user agent so the Phoenix pages render as in a browser,
/// with a product token the server can recognise the desktop app by.
fn user_agent() -> String {
    format!(
        "Mozilla/5.0 (Macintosh) AppleWebKit/605.1.15 (KHTML, like Gecko) StarterPackClient/{}",
        env!("CARGO_PKG_VERSION")
    )
}
