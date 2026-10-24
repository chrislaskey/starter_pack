//! The quick search popup (`src/quick-search.html`): ⌃⌘T anywhere on the Mac
//! opens a small window under the text caret with a filter box over text
//! snippets and sample images. Enter copies the selected item to the
//! clipboard, closes the window and pastes it where the user was typing.
//! See `docs/quick-search.md`.

mod items;
mod placement;
mod window;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard::Clipboard;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::macos::{self, ClipboardImage, RunningApp, ScreenRect};

use items::{Content, SampleItem, get_sample_item};
pub use items::{Item, list_items};
use placement::{Placement, TextPlacement};

const SHORTCUT_LABEL: &str = "⌃⌘T";
const OPENED_EVENT: &str = "quick-search-opened";
const MOVED_EVENT: &str = "quick-search-moved";
const PASTED_EVENT: &str = "quick-search-pasted";
/// How long opening waits for the caret before opening at the screen
/// position and moving later.
const CARET_WAIT: Duration = Duration::from_millis(150);
/// How long the app in front gets to take the keyboard back, and the user
/// to release Enter, before ⌘V arrives.
const PASTE_DELAY: Duration = Duration::from_millis(200);

#[derive(Default)]
pub struct QuickSearch {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The app in front when the popup opened, to paste into.
    previous_app: Option<RunningApp>,
    /// Counts the opens, so a caret found late moves only the popup it was
    /// asked for.
    open_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub shortcut: &'static str,
    pub accessibility_trusted: bool,
}

/// Where the popup opened. Sent as `quick-search-opened`, and again as
/// `quick-search-moved` when the caret arrives after the popup opened at
/// the screen position.
#[derive(Clone, Debug, Serialize)]
pub struct OpenedWindow {
    pub placement: Placement,
    /// Why neither the caret nor a text field was used, when so.
    pub placement_error: Option<String>,
    /// The top left corner, in points from the main display's top left.
    pub position: (f64, f64),
    /// The usable area of the screen the popup was kept on.
    pub screen: ScreenRect,
    pub previous_app: Option<RunningApp>,
    pub accessibility_trusted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "result", content = "detail")]
pub enum PasteResult {
    Pasted,
    /// ⌘V couldn't be sent without Accessibility access.
    NoAccessibilityAccess,
    /// This app was in front and the previous app couldn't be brought
    /// back, so ⌘V would have gone to this app.
    PreviousAppNotActivated(String),
    Failed(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct PasteOutcome {
    pub label: &'static str,
    /// The pasteboard types written, like `public.file-url` and
    /// `public.png`; just `public.utf8-plain-text` for text.
    pub clipboard_types: Vec<String>,
    pub previous_app: Option<RunningApp>,
    pub paste: PasteResult,
}

/// Registers ⌃⌘T and creates the window, hidden.
pub fn register(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(QuickSearch::default());

    let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::CONTROL), Code::KeyT);
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, pressed, event| {
                if pressed == &shortcut
                    && event.state() == ShortcutState::Pressed
                    && let Err(error) = toggle(app)
                {
                    log::warn!("Could not toggle the quick search popup: {error}");
                }
            })
            .build(),
    )?;
    app.global_shortcut().register(shortcut)?;
    log::info!("Registered {SHORTCUT_LABEL} for the quick search popup");

    window::create(app)?;
    Ok(())
}

pub fn get_status() -> Status {
    Status {
        shortcut: SHORTCUT_LABEL,
        accessibility_trusted: macos::is_accessibility_trusted(false),
    }
}

/// Opens the popup, or closes it when it's open. Main thread only.
pub fn toggle(app: &AppHandle) -> Result<(), String> {
    if window::is_open(app) {
        close(app);
        Ok(())
    } else {
        open(app)
    }
}

/// Opens the popup within `CARET_WAIT` whatever the app in front does: when
/// the caret isn't known in time it opens at the screen position and moves
/// once it is. Main thread only.
pub fn open(app: &AppHandle) -> Result<(), String> {
    if window::is_open(app) {
        return Ok(());
    }

    // Read before showing anything, so the popup itself isn't in front.
    let previous_app = macos::get_frontmost_app();
    let accessibility_trusted = macos::is_accessibility_trusted(false);
    let open_count = {
        let mut state = state(app);
        state.previous_app = previous_app.clone();
        state.open_count += 1;
        state.open_count
    };

    let fallback_screen = placement::get_pointer_screen(app)?;
    let lookup = placement::find_text_placement(previous_app.as_ref(), accessibility_trusted);
    let (text, caret_pending) = match lookup.recv_timeout(CARET_WAIT) {
        Ok(text) => (text, false),
        Err(RecvTimeoutError::Timeout) => {
            (Err("the app in front hasn't answered yet".into()), true)
        }
        Err(RecvTimeoutError::Disconnected) => (Err("the caret lookup stopped".into()), false),
    };

    let opened = place(
        app,
        text,
        &fallback_screen,
        previous_app,
        accessibility_trusted,
    )?;
    log::debug!("Opening the quick search popup: {opened:?}");
    emit_to_pages(app, OPENED_EVENT, &opened);
    window::show(app)?;

    if caret_pending {
        move_when_caret_found(app.clone(), lookup, open_count, fallback_screen, opened);
    }
    Ok(())
}

pub fn close(app: &AppHandle) {
    window::hide(app);
}

/// Copies the item to the clipboard, closes the popup and pastes into the
/// app the popup opened over.
pub async fn paste_item(app: AppHandle, item_id: String) -> Result<PasteOutcome, String> {
    let item = get_sample_item(&item_id).ok_or_else(|| format!("No item {item_id:?}"))?;
    let clipboard_types = copy_item_to_clipboard(&app, item).await?;
    let previous_app = state(&app).previous_app.clone();

    let to_close = app.clone();
    app.run_on_main_thread(move || close(&to_close))
        .map_err(|error| error.to_string())?;

    let paste = if !macos::is_accessibility_trusted(false) {
        PasteResult::NoAccessibilityAccess
    } else {
        match bring_previous_app_to_front(&app, previous_app.as_ref()).await {
            Err(error) => PasteResult::PreviousAppNotActivated(error),
            Ok(()) => {
                tokio::time::sleep(PASTE_DELAY).await;
                match macos::press_paste_shortcut() {
                    Ok(()) => PasteResult::Pasted,
                    Err(error) => PasteResult::Failed(error),
                }
            }
        }
    };

    let outcome = PasteOutcome {
        label: item.label,
        clipboard_types,
        previous_app,
        paste,
    };
    log::info!(
        "Quick search copied {:?} as {:?}: {:?}",
        outcome.label,
        outcome.clipboard_types,
        outcome.paste
    );
    emit_to_main_window(&app, PASTED_EVENT, &outcome);
    Ok(outcome)
}

/// Text goes through the clipboard plugin. An image is written to a file in
/// the app's cache directory first, so apps that take files get one, then
/// put on the pasteboard on the main thread.
async fn copy_item_to_clipboard(
    app: &AppHandle,
    item: &'static SampleItem,
) -> Result<Vec<String>, String> {
    match item.content {
        Content::Text(text) => {
            app.state::<Clipboard>().write_text(text.to_string())?;
            Ok(vec!["public.utf8-plain-text".to_string()])
        }
        Content::Image {
            file_name,
            uti,
            bytes,
        } => {
            let file = write_image_file(app, file_name, bytes).await?;
            run_on_main_thread(app, move || {
                macos::copy_image_to_clipboard(ClipboardImage {
                    bytes,
                    uti,
                    file: &file,
                })
            })
            .await
        }
    }
}

async fn write_image_file(
    app: &AppHandle,
    file_name: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("quick-search");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let file = directory.join(file_name);
    tokio::fs::write(&file, bytes)
        .await
        .map_err(|error| format!("Could not write {}: {error}", file.display()))?;
    Ok(file)
}

/// The non-activating panel normally leaves the previous app in front. When
/// this app is in front instead (⌃⌘T was pressed in the main window), asks
/// macOS to put the previous app back.
async fn bring_previous_app_to_front(
    app: &AppHandle,
    previous_app: Option<&RunningApp>,
) -> Result<(), String> {
    let pid = previous_app.map(|app| app.pid);
    run_on_main_thread(app, move || {
        if !macos::is_this_app_frontmost() {
            return Ok(());
        }
        match pid {
            None => Err("This app is in front and no previous app is known".to_string()),
            Some(pid) if macos::activate_app(pid) => Ok(()),
            Some(pid) => Err(format!("macOS refused to activate the app with pid {pid}")),
        }
    })
    .await
}

async fn run_on_main_thread<T: Send + 'static>(
    app: &AppHandle,
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (reply, outcome) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = reply.send(task());
    })
    .map_err(|error| error.to_string())?;
    outcome
        .await
        .map_err(|_| "The main thread dropped the task".to_string())?
}

/// Moves the window to the text, else to the screen position.
fn place(
    app: &AppHandle,
    text: TextPlacement,
    fallback_screen: &ScreenRect,
    previous_app: Option<RunningApp>,
    accessibility_trusted: bool,
) -> Result<OpenedWindow, String> {
    let (placement, placement_error, screen, wanted) = match text {
        Ok((placement, rect)) => {
            let text_screen =
                placement::get_screen_at(app, tauri::LogicalPosition::new(rect.x, rect.y));
            let screen = text_screen.unwrap_or(*fallback_screen);
            (placement, None, screen, placement::position_below(&rect))
        }
        Err(error) => (
            Placement::Screen,
            Some(error),
            *fallback_screen,
            placement::position_on_screen(fallback_screen),
        ),
    };
    let position = placement::keep_on_screen(wanted, &screen);
    window::set_position(app, position)?;

    Ok(OpenedWindow {
        placement,
        placement_error,
        position: (position.x, position.y),
        screen,
        previous_app,
        accessibility_trusted,
    })
}

/// Waits for a late caret and moves the popup to it, if it's still the same
/// open and still showing.
fn move_when_caret_found(
    app: AppHandle,
    lookup: Receiver<TextPlacement>,
    open_count: u64,
    fallback_screen: ScreenRect,
    opened: OpenedWindow,
) {
    std::thread::spawn(move || {
        let Ok(text) = lookup.recv() else {
            return;
        };
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if state(&handle).open_count != open_count || !window::is_open(&handle) {
                return;
            }
            match place(
                &handle,
                text,
                &fallback_screen,
                opened.previous_app,
                opened.accessibility_trusted,
            ) {
                Ok(moved) => {
                    log::debug!("Moving the quick search popup: {moved:?}");
                    emit_to_pages(&handle, MOVED_EVENT, &moved);
                }
                Err(error) => log::warn!("Could not move the quick search popup: {error}"),
            }
        });
    });
}

fn state(app: &AppHandle) -> MutexGuard<'_, State> {
    app.state::<QuickSearch>()
        .inner()
        .state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Tells the popup page, for its footer, and the main window, for its log.
fn emit_to_pages<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(error) = app.emit_to(window::LABEL, event, payload.clone()) {
        log::warn!("Could not send {event} to the popup: {error}");
    }
    emit_to_main_window(app, event, payload);
}

fn emit_to_main_window<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(error) = app.emit_to(crate::MAIN_WINDOW, event, payload.clone()) {
        log::debug!("No main window to send {event} to: {error}");
    }
}
