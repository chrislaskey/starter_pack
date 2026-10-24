//! The quick search popup (`src/quick-search.html`): ⌃⌘T anywhere on the Mac
//! opens a small panel under the text caret, else under the focused text
//! field, else near the top of the screen like Spotlight, with a filter box
//! over a few text snippets and sample images. Enter copies the selected
//! match to the clipboard, closes the panel and pastes it where the user was
//! typing.
//!
//! The panel is an `NSPanel` with the non-activating style (through
//! `tauri-nspanel`), like Spotlight: it takes keyboard input without making
//! this app the active one, so the app the user was typing in stays in front
//! and receives the paste. See `docs/macos-quick-search-research.md` for what
//! Tauri could and couldn't do here.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use base64::Engine as _;
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Size, WebviewUrl, WindowEvent,
};
use tauri_nspanel::{
    CollectionBehavior, ManagerExt as _, PanelBuilder, PanelHandle, PanelLevel, StyleMask,
    tauri_panel,
};
use tauri_plugin_clipboard::Clipboard;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::macos::{self, FrontmostApp, ImageForClipboard, ScreenRect, TextBounds};

pub const WINDOW: &str = "quick-search";
const PAGE: &str = "quick-search.html";
const SHORTCUT_LABEL: &str = "⌃⌘T";
const PANEL_SIZE: (f64, f64) = (380.0, 392.0);
/// Below the caret or text field, so the panel doesn't cover the line
/// being typed.
const TEXT_GAP: f64 = 6.0;
/// A focused element taller than this is an editor or a whole web page,
/// not a text field: no point opening under it.
const MAX_TEXT_FIELD_HEIGHT: f64 = 80.0;
/// Where the panel's top goes when there is no caret: like Spotlight, in
/// the upper part of the screen the pointer is on.
const SCREEN_TOP_FRACTION: f64 = 0.2;
/// How long the hotkey waits for the caret before opening at the screen
/// position and moving later. Accessibility calls are blocking IPC with
/// the other app, which may be slow or stuck.
const ANCHOR_WAIT: Duration = Duration::from_millis(150);
/// How long a Chromium or Electron app gets to build its accessibility
/// tree after being asked to, before the caret is looked up again.
const ACCESSIBILITY_TREE_DELAY: Duration = Duration::from_millis(150);
/// How long the app in front gets to take the keyboard back, and the user
/// to release Enter, before ⌘V arrives.
const PASTE_DELAY: Duration = Duration::from_millis(200);

tauri_panel! {
    panel!(QuickSearchPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

/// What the popup offers. Text goes on the clipboard through the clipboard
/// plugin; images through `NSPasteboard` directly, see
/// `macos::copy_image_to_clipboard`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Content {
    Text {
        value: &'static str,
    },
    Image {
        file_name: &'static str,
        uti: &'static str,
        bytes: &'static [u8],
    },
}

struct Sample {
    id: &'static str,
    label: &'static str,
    content: Content,
}

/// The sample images are the ones the popup page shows as thumbnails too.
const SAMPLES: [Sample; 8] = [
    Sample {
        id: "shrug",
        label: "Shrug",
        content: Content::Text {
            value: r"¯\_(ツ)_/¯",
        },
    },
    Sample {
        id: "done",
        label: "Done checkmark",
        content: Content::Text { value: "✅ Done" },
    },
    Sample {
        id: "lorem",
        label: "Lorem ipsum",
        content: Content::Text {
            value: "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
        },
    },
    Sample {
        id: "png",
        label: "PNG image",
        content: Content::Image {
            file_name: "sample.png",
            uti: "public.png",
            bytes: include_bytes!("../../src/quick-search/images/sample.png"),
        },
    },
    Sample {
        id: "jpeg",
        label: "JPEG image",
        content: Content::Image {
            file_name: "sample.jpg",
            uti: "public.jpeg",
            bytes: include_bytes!("../../src/quick-search/images/sample.jpg"),
        },
    },
    Sample {
        id: "gif",
        label: "GIF image",
        content: Content::Image {
            file_name: "sample-static.gif",
            uti: "com.compuserve.gif",
            bytes: include_bytes!("../../src/quick-search/images/sample-static.gif"),
        },
    },
    Sample {
        id: "animated-gif",
        label: "Animated GIF",
        content: Content::Image {
            file_name: "sample-animated.gif",
            uti: "com.compuserve.gif",
            bytes: include_bytes!("../../src/quick-search/images/sample-animated.gif"),
        },
    },
    Sample {
        id: "webp",
        label: "WebP image",
        content: Content::Image {
            file_name: "sample.webp",
            uti: "org.webmproject.webp",
            bytes: include_bytes!("../../src/quick-search/images/sample.webp"),
        },
    },
];

/// A choice as the popup page shows it.
#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    /// `text` or `image`.
    pub kind: &'static str,
    /// The text itself, or the image's file name and size.
    pub value: String,
    /// A `data:` URL of the image, for a thumbnail.
    pub preview: Option<String>,
}

impl Sample {
    fn choice(&self) -> Choice {
        match &self.content {
            Content::Text { value } => Choice {
                id: self.id,
                label: self.label,
                kind: "text",
                value: (*value).to_string(),
                preview: None,
            },
            Content::Image {
                file_name,
                uti,
                bytes,
            } => Choice {
                id: self.id,
                label: self.label,
                kind: "image",
                value: format!("{file_name}, {} KB", bytes.len().div_ceil(1024)),
                preview: Some(format!(
                    "data:{};base64,{}",
                    mime_type(uti),
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                )),
            },
        }
    }
}

fn mime_type(uti: &str) -> &'static str {
    match uti {
        "public.png" => "image/png",
        "public.jpeg" => "image/jpeg",
        "com.compuserve.gif" => "image/gif",
        "org.webmproject.webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// What the popup was positioned next to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    Caret,
    TextField,
    Screen,
}

/// Where the popup opened, sent to the popup page and the main window's
/// log. Sent again as `quick-search-anchored` when the caret arrives after
/// the popup already opened at the screen position.
#[derive(Clone, Debug, Serialize)]
pub struct Opened {
    pub anchor: Anchor,
    /// Why the caret wasn't used, when it wasn't.
    pub anchor_detail: Option<String>,
    /// The panel's top left corner, in points from the main display's top
    /// left.
    pub position: (f64, f64),
    /// The usable area of the screen it was kept on.
    pub screen: ScreenRect,
    pub previous_app: Option<FrontmostApp>,
    pub accessibility_trusted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct QuickSearchStatus {
    pub shortcut: &'static str,
    pub accessibility_trusted: bool,
    pub choices: Vec<Choice>,
}

/// What happened to the paste after copying.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "result", content = "detail")]
pub enum PasteOutcome {
    Pasted,
    NotRequested,
    /// The user hasn't granted Accessibility access, so ⌘V couldn't be sent.
    NoAccessibilityAccess,
    /// This app had become active and the previous app couldn't be brought
    /// back in front, so ⌘V would have gone to this app.
    PreviousAppNotActivated(String),
    Failed(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct PickOutcome {
    pub label: &'static str,
    /// The pasteboard types written, like `public.file-url` and
    /// `public.png`; just `public.utf8-plain-text` for text.
    pub clipboard_types: Vec<String>,
    pub previous_app: Option<FrontmostApp>,
    pub paste: PasteOutcome,
}

#[derive(Default)]
pub struct QuickSearch {
    /// The app in front when the popup opened, to paste into.
    previous_app: Mutex<Option<FrontmostApp>>,
    /// Counts the opens, so a caret found late moves only the popup it was
    /// asked for.
    opens: Mutex<u64>,
}

/// Registers ⌃⌘T and creates the panel, hidden.
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

    // Created now, hidden: a panel shown in the same call that creates it
    // loses key status again while its page loads, so the first ⌃⌘T would
    // open and immediately close it.
    panel(app)?;

    Ok(())
}

pub fn status() -> QuickSearchStatus {
    QuickSearchStatus {
        shortcut: SHORTCUT_LABEL,
        accessibility_trusted: macos::is_accessibility_trusted(false),
        choices: choices(),
    }
}

pub fn choices() -> Vec<Choice> {
    SAMPLES.iter().map(Sample::choice).collect()
}

/// Shows the popup, or hides it when it's showing. Main thread only, like
/// everything touching the panel.
///
/// The popup opens within `ANCHOR_WAIT` whatever the app in front does:
/// the caret is looked up on another thread, and when it isn't known in
/// time the popup opens at the screen position and moves once it is.
pub fn toggle(app: &AppHandle) -> Result<(), String> {
    let panel = panel(app)?;
    if panel.is_visible() {
        log::debug!("The quick search popup is showing, hiding it");
        panel.hide();
        return Ok(());
    }

    // Before showing anything, so the popup itself isn't "in front".
    let previous_app = macos::frontmost_app();
    let accessibility_trusted = macos::is_accessibility_trusted(false);
    let state = app.state::<QuickSearch>();
    *state.previous_app.lock().unwrap() = previous_app.clone();
    let open = {
        let mut opens = state.opens.lock().unwrap();
        *opens += 1;
        *opens
    };

    let fallback_screen = pointer_screen(app)?;
    let lookup = start_text_lookup(previous_app.as_ref(), accessibility_trusted);
    let (placement, pending) = match lookup.recv_timeout(ANCHOR_WAIT) {
        Ok(placement) => (placement, false),
        Err(RecvTimeoutError::Timeout) => (
            Err("the app in front hasn't answered yet".to_string()),
            true,
        ),
        Err(RecvTimeoutError::Disconnected) => (Err("the caret lookup stopped".to_string()), false),
    };

    let opened = place(
        app,
        placement,
        &fallback_screen,
        previous_app,
        accessibility_trusted,
    )?;
    log::debug!("Opening the quick search popup: {opened:?}");
    announce(app, "quick-search-opened", &opened)?;
    panel.show_and_make_key();

    if pending {
        move_when_found(app.clone(), lookup, open, fallback_screen, opened);
    }
    Ok(())
}

pub fn hide(app: &AppHandle) {
    if let Ok(panel) = app.get_webview_panel(WINDOW)
        && panel.is_visible()
    {
        panel.hide();
    }
}

/// Copies the choice, hides the popup and, when asked, pastes into the app
/// the popup opened over.
pub async fn pick(app: AppHandle, choice_id: String, paste: bool) -> Result<PickOutcome, String> {
    let sample = SAMPLES
        .iter()
        .find(|sample| sample.id == choice_id)
        .ok_or_else(|| format!("No choice {choice_id:?}"))?;
    let clipboard_types = copy_to_clipboard(&app, sample).await?;
    let previous_app = app
        .state::<QuickSearch>()
        .previous_app
        .lock()
        .unwrap()
        .clone();

    let hidden = app.clone();
    app.run_on_main_thread(move || hide(&hidden))
        .map_err(|error| error.to_string())?;

    let paste = if !paste {
        PasteOutcome::NotRequested
    } else if !macos::is_accessibility_trusted(false) {
        PasteOutcome::NoAccessibilityAccess
    } else {
        match bring_previous_app_to_front(&app, previous_app.as_ref()).await {
            Err(error) => PasteOutcome::PreviousAppNotActivated(error),
            Ok(()) => {
                tokio::time::sleep(PASTE_DELAY).await;
                match macos::press_paste_shortcut() {
                    Ok(()) => PasteOutcome::Pasted,
                    Err(error) => PasteOutcome::Failed(error),
                }
            }
        }
    };

    let outcome = PickOutcome {
        label: sample.label,
        clipboard_types,
        previous_app,
        paste,
    };
    log::info!(
        "Quick search picked {:?} as {:?}: {:?}",
        outcome.label,
        outcome.clipboard_types,
        outcome.paste
    );
    if let Err(error) = app.emit_to(crate::MAIN_WINDOW, "quick-search-picked", outcome.clone()) {
        log::debug!("No main window to tell about the pick: {error}");
    }
    Ok(outcome)
}

/// Text goes through the clipboard plugin. An image is written to a file in
/// the app's cache directory first, so apps that take files get one, then
/// put on the pasteboard on the main thread.
async fn copy_to_clipboard(
    app: &AppHandle,
    sample: &'static Sample,
) -> Result<Vec<String>, String> {
    match &sample.content {
        Content::Text { value } => {
            app.state::<Clipboard>().write_text((*value).to_string())?;
            Ok(vec!["public.utf8-plain-text".to_string()])
        }
        Content::Image {
            file_name,
            uti,
            bytes,
        } => {
            let file = image_file(app, file_name, bytes).await?;
            let (reply, outcome) = tokio::sync::oneshot::channel();
            app.run_on_main_thread(move || {
                let _ = reply.send(macos::copy_image_to_clipboard(ImageForClipboard {
                    bytes,
                    uti,
                    file: &file,
                }));
            })
            .map_err(|error| error.to_string())?;
            outcome
                .await
                .map_err(|_| "The main thread dropped the clipboard request".to_string())?
        }
    }
}

async fn image_file(app: &AppHandle, file_name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
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

/// Normally the non-activating panel leaves the previous app in front and
/// there is nothing to do. When this app is in front (say the user pressed
/// ⌃⌘T in the main window), it asks macOS to put the previous app back.
async fn bring_previous_app_to_front(
    app: &AppHandle,
    previous_app: Option<&FrontmostApp>,
) -> Result<(), String> {
    let (reply, outcome) = tokio::sync::oneshot::channel();
    let pid = previous_app.map(|app| app.pid);
    app.run_on_main_thread(move || {
        let result = if !macos::is_this_app_in_front() {
            Ok(())
        } else {
            match pid {
                None => Err("This app is active and no previous app is known".to_string()),
                Some(pid) if macos::activate_app(pid) => Ok(()),
                Some(pid) => Err(format!("macOS refused to activate the app with pid {pid}")),
            }
        };
        let _ = reply.send(result);
    })
    .map_err(|error| error.to_string())?;
    outcome
        .await
        .map_err(|_| "The main thread dropped the activation request".to_string())?
}

fn panel(app: &AppHandle) -> Result<PanelHandle<tauri::Wry>, String> {
    if let Ok(panel) = app.get_webview_panel(WINDOW) {
        return Ok(panel);
    }

    let panel = PanelBuilder::<_, QuickSearchPanel>::new(app, WINDOW)
        .url(WebviewUrl::App(PAGE.into()))
        .title("Quick search")
        .size(Size::Logical(LogicalSize::new(PANEL_SIZE.0, PANEL_SIZE.1)))
        .resizable(false)
        .level(PanelLevel::Floating)
        .floating(true)
        .has_shadow(true)
        .corner_radius(12.0)
        // Non-activating: the panel can be key without activating this app.
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        // Follows the user to any Space, including a full screen app's, and
        // stays out of Mission Control.
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .transient(),
        )
        .no_activate(true)
        .hides_on_deactivate(false)
        .with_window(|window| {
            window
                .decorations(false)
                .skip_taskbar(true)
                .accept_first_mouse(true)
                .visible(false)
        })
        .build()
        .map_err(|error| error.to_string())?;

    // Clicking anywhere else closes the popup. Tauri's window delegate stays
    // in place on the panel, so its focus events still arrive.
    if let Some(window) = app.get_webview_window(WINDOW) {
        let on_blur = app.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                log::debug!("The quick search popup lost key status, hiding it");
                hide(&on_blur);
            }
        });
    }

    log::info!("Created the quick search panel");
    Ok(panel)
}

type Placement = Result<(Anchor, ScreenRect), String>;

/// Looks up the caret of the app in front on another thread, since the
/// Accessibility calls block until the app answers. Asking this app
/// itself from the main thread would wait on that very thread, so the
/// popup over this app's own windows takes the screen position.
fn start_text_lookup(
    previous_app: Option<&FrontmostApp>,
    accessibility_trusted: bool,
) -> mpsc::Receiver<Placement> {
    let (sender, receiver) = mpsc::channel();
    let immediate = match previous_app {
        None => Some("no app in front".to_string()),
        Some(app) if app.pid == std::process::id() as i32 => {
            Some("the popup opened over this app".to_string())
        }
        Some(_) if !accessibility_trusted => Some("no Accessibility access".to_string()),
        Some(_) => None,
    };
    match (immediate, previous_app) {
        (None, Some(app)) => {
            let app = app.clone();
            std::thread::spawn(move || {
                let _ = sender.send(text_bounds(&app));
            });
        }
        (reason, _) => {
            let _ = sender.send(Err(reason.unwrap_or_default()));
        }
    }
    receiver
}

/// The caret, or a one-line text field. Chromium and Electron apps answer
/// only after accessibility is switched on in them, so a failed first look
/// asks for that and looks once more.
fn text_bounds(previous_app: &FrontmostApp) -> Placement {
    let bounds = match macos::focused_text_bounds(previous_app.pid) {
        Ok(bounds) => bounds,
        Err(first) => {
            macos::enable_accessibility(previous_app.pid)
                .map_err(|error| format!("{first}; enabling accessibility: {error}"))?;
            std::thread::sleep(ACCESSIBILITY_TREE_DELAY);
            macos::focused_text_bounds(previous_app.pid)
                .map_err(|retry| format!("{first}; after enabling accessibility: {retry}"))?
        }
    };
    match bounds {
        TextBounds::Caret(rect) => Ok((Anchor::Caret, rect)),
        TextBounds::TextField(rect) if rect.height <= MAX_TEXT_FIELD_HEIGHT => {
            Ok((Anchor::TextField, rect))
        }
        TextBounds::TextField(rect) => Err(format!(
            "no caret, and the focused element is {}×{} points, too big for a text field",
            rect.width.round(),
            rect.height.round()
        )),
    }
}

/// Turns the lookup's answer into a position on screen, and moves the
/// panel there.
fn place(
    app: &AppHandle,
    placement: Placement,
    fallback_screen: &ScreenRect,
    previous_app: Option<FrontmostApp>,
    accessibility_trusted: bool,
) -> Result<Opened, String> {
    let (anchor, anchor_detail, screen, wanted) = match placement {
        Ok((anchor, rect)) => {
            let below = LogicalPosition::new(rect.x, rect.y + rect.height + TEXT_GAP);
            // The screen the text is on, else the pointer's: never a
            // position on no screen at all.
            let screen =
                screen_at(app, LogicalPosition::new(rect.x, rect.y)).unwrap_or(*fallback_screen);
            (anchor, None, screen, below)
        }
        Err(detail) => (
            Anchor::Screen,
            Some(detail),
            *fallback_screen,
            screen_position(fallback_screen),
        ),
    };
    let position = keep_on_screen(wanted, &screen);

    app.get_webview_window(WINDOW)
        .ok_or("The quick search window is gone")?
        .set_position(position)
        .map_err(|error| error.to_string())?;

    Ok(Opened {
        anchor,
        anchor_detail,
        position: (position.x, position.y),
        screen,
        previous_app,
        accessibility_trusted,
    })
}

/// Waits for a late caret and moves the popup there, if it's still the
/// same popup and still showing.
fn move_when_found(
    app: AppHandle,
    lookup: mpsc::Receiver<Placement>,
    open: u64,
    fallback_screen: ScreenRect,
    opened: Opened,
) {
    std::thread::spawn(move || {
        let Ok(placement) = lookup.recv() else {
            return;
        };
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let current = *handle.state::<QuickSearch>().opens.lock().unwrap();
            let showing = handle
                .get_webview_panel(WINDOW)
                .is_ok_and(|panel| panel.is_visible());
            if current != open || !showing {
                return;
            }
            match place(
                &handle,
                placement,
                &fallback_screen,
                opened.previous_app,
                opened.accessibility_trusted,
            ) {
                Ok(moved) if moved.anchor != Anchor::Screen => {
                    log::debug!("Moving the quick search popup: {moved:?}");
                    let _ = announce(&handle, "quick-search-anchored", &moved);
                }
                Ok(stayed) => {
                    let _ = announce(&handle, "quick-search-anchored", &stayed);
                }
                Err(error) => log::warn!("Could not move the quick search popup: {error}"),
            }
        });
    });
}

/// Tells the popup page, for its footer, and the main window, for its log.
fn announce(app: &AppHandle, event: &str, opened: &Opened) -> Result<(), String> {
    app.emit_to(WINDOW, event, opened.clone())
        .map_err(|error| error.to_string())?;
    if let Err(error) = app.emit_to(crate::MAIN_WINDOW, event, opened.clone()) {
        log::debug!("No main window to tell about the popup: {error}");
    }
    Ok(())
}

/// Centered in the upper part of a screen.
fn screen_position(screen: &ScreenRect) -> LogicalPosition<f64> {
    LogicalPosition::new(
        screen.x + (screen.width - PANEL_SIZE.0) / 2.0,
        screen.y + screen.height * SCREEN_TOP_FRACTION,
    )
}

fn keep_on_screen(position: LogicalPosition<f64>, screen: &ScreenRect) -> LogicalPosition<f64> {
    let x = position
        .x
        .min(screen.x + screen.width - PANEL_SIZE.0)
        .max(screen.x);
    let y = position
        .y
        .min(screen.y + screen.height - PANEL_SIZE.1)
        .max(screen.y);
    LogicalPosition::new(x, y)
}

/// The usable area of the screen the pointer is on, else the main one.
fn pointer_screen(app: &AppHandle) -> Result<ScreenRect, String> {
    let pointer = app.cursor_position().map_err(|error| error.to_string())?;
    // Tauri reports the pointer in the main display's physical pixels.
    let primary = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("no main display")?;
    let pointer: LogicalPosition<f64> = pointer.to_logical(primary.scale_factor());
    Ok(screen_at(app, pointer).unwrap_or_else(|| screen_rect(&primary)))
}

/// The usable area of the display containing `position`, in points.
fn screen_at(app: &AppHandle, position: LogicalPosition<f64>) -> Option<ScreenRect> {
    // On macOS Tauri's `monitor_from_point` compares against display bounds
    // in points, unlike `Monitor`'s position, size and work area, which
    // are physical pixels.
    app.monitor_from_point(position.x, position.y)
        .ok()
        .flatten()
        .map(|monitor| screen_rect(&monitor))
}

fn screen_rect(monitor: &tauri::Monitor) -> ScreenRect {
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    ScreenRect {
        x: f64::from(area.position.x) / scale,
        y: f64::from(area.position.y) / scale,
        width: f64::from(area.size.width) / scale,
        height: f64::from(area.size.height) / scale,
    }
}
