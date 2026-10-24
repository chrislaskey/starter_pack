# Quick search popup

⌃⌘T in any app opens a small popup under the text caret. It has a filter
box over a few text snippets and sample images. ↑/↓ move the selection,
Enter copies the selected item to the clipboard, closes the popup and pastes
it where the user was typing. Escape or clicking elsewhere closes it.

Built 2026-10-24 to test Tauri against a feature that is routine in a native
AppKit app. Everything in that brief works. Tauri covered about half of it;
the rest is AppKit, Accessibility and Core Graphics calls made from Rust
through the `objc2-app-kit`, `core-foundation` and `core-graphics` crates.
No Swift was needed.

## Code

```
src/quick-search.html, src/quick-search/   the popup page
src/public/quick-search/images/            the sample images, served for thumbnails
src-tauri/src/quick_search/
  mod.rs          open, close, paste; the state and events
  items.rs        the items: text snippets and the sample images, list and get
  placement.rs    where the popup opens: caret, text field or screen
  window.rs       the non-activating NSPanel
src-tauri/src/macos/
  accessibility.rs  Accessibility access, the caret of the app in front
  running_app.rs    the app in front, activating an app
  pasteboard.rs     an image on the clipboard in several representations
  keyboard.rs       pressing ⌘V
src-tauri/capabilities/quick-search.json   what the popup page may call
```

The page is a thin view: the Rust side lists and filters the items, decides
where the popup opens, copies and pastes. The page shows the items, moves
the selection and sends the pick.

### Commands

| Command | Window | Does |
| --- | --- | --- |
| `get_quick_search_status` | main | the shortcut and whether Accessibility access is granted |
| `request_accessibility_access` | main | opens the system dialog when access isn't granted |
| `open_quick_search` | main | what ⌃⌘T does |
| `list_quick_search_items` | popup | the items whose label or value contains the query |
| `paste_quick_search_item` | popup | copies the item, closes the popup, pastes |
| `close_quick_search` | popup | Escape |

### Events

| Event | To | Payload |
| --- | --- | --- |
| `quick-search-opened` | popup, main | `OpenedWindow`: placement, position, screen, the previous app, access state |
| `quick-search-moved` | popup, main | the same, when the caret arrived after the popup opened at the screen position |
| `quick-search-pasted` | main | `PasteOutcome`: the item, the pasteboard types written, the paste result |

The popup shows the placement in its footer. The main window logs every
open, move and paste.

## How it works

**The window** is an `NSPanel` with the non-activating style, through the
`tauri-nspanel` plugin, which swaps the Tauri window's class at runtime. The
panel takes keystrokes without making this app active, so the app the user
was typing in stays in front and receives the paste. A plain Tauri window
can't do this: showing and focusing it activates the app, and
`focusable(false)` keeps the previous app in front but then the window can't
take keystrokes either. The panel is created hidden at startup: a panel
shown in the call that creates it loses key status again while its page
loads, and the first ⌃⌘T would open and close it. Losing key status
(Tauri's `WindowEvent::Focused(false)`) closes it, which covers clicking
elsewhere.

**Placement**, in order:

1. At the caret: `AXSelectedTextRange` and `AXBoundsForRange` of the
   focused element of the app in front. Native text views, Safari and
   Firefox report it.
2. Under the focused text field: the element's `AXFrame`, when it is at
   most 80 points tall. Taller elements are editors or whole web pages,
   and the popup would open below the screen.
3. Near the top of the screen the pointer is on, centered, like Spotlight.

Chromium and Electron apps (Slack, Discord, VS Code, Obsidian) report
nothing until accessibility is switched on in them by setting
`AXManualAccessibility` on the app's element, so a failed first look does
that and looks again 150 ms later. Terminals have no caret to report.

The Accessibility calls are blocking IPC with the other app, so they run on
their own thread with a 0.5 s per-call timeout, and opening waits at most
150 ms for them. Past that the popup opens at the screen position and moves
when the caret arrives, as long as it is still the same open. The lookup
never asks this app itself: from the main thread it could only time out.

Positions are clamped to the screen the text is on, else the pointer's.
Tauri's `monitor_from_point` takes points on macOS while `Monitor`'s
position, size and work area are physical pixels, and the pointer comes in
the main display's physical pixels; `placement.rs` converts.

**Copying.** Text goes through the `tauri-plugin-clipboard` plugin. An
image is written to a file in the app's cache directory, then put on the
`NSPasteboard` as one item with several representations, so each app takes
the one it understands. Neither Tauri nor the clipboard plugins write
several representations to one item.

| Representation | Type | Who takes it |
| --- | --- | --- |
| The file URL | `public.file-url` | Finder, Slack, Discord, Mail, Messages, Notes, browsers' `clipboardData.files`. Keeps a GIF's animation and a WebP's format. AppKit serves the legacy `NSFilenamesPboardType` from it. |
| The bytes as they are | `public.png`, `public.jpeg`, `com.compuserve.gif`, `org.webmproject.webp` | Apps that read that exact format. |
| A PNG rendering | `public.png` | Browsers, Electron apps, most editors. The first frame of an animated GIF. |
| A TIFF rendering | `public.tiff` | Older and native apps: TextEdit, Preview, Pages. |

The renderings come from `NSImage` and `NSBitmapImageRep`, which decode
PNG, JPEG, GIF and WebP (macOS 11+).

**Pasting.** The popup closes, then a ⌘V key event is posted with Core
Graphics after 200 ms, so the previous app has the keyboard back and the
user has released Enter. When this app is in front after all (⌃⌘T pressed
in the main window), `NSRunningApplication.activate` puts the previous app
back first; since macOS 14 the system may refuse, and the outcome says so.
Apps with Secure Keyboard Entry on, and password fields, drop the synthetic
⌘V, as they would from any app.

**Accessibility access** is needed for the caret and the paste. Without it
the popup still copies, opens at the screen position, and its footer and the
paste outcome say why. `AXIsProcessTrustedWithOptions` has no objc2
binding, so `accessibility.rs` declares the HIServices functions by hand
and links the ApplicationServices framework. The status page asks for
access and re-checks when its window regains focus.

## Limitations

- **Development builds and Accessibility access.** macOS attributes
  privacy permissions to the responsible app, and under `cargo tauri dev`
  that is the terminal app: the system dialog names it, and the grant has
  to go to it. For a bundle, the grant is tied to the code signature; an
  ad-hoc signed debug build changes signature on every rebuild, so the grant
  may show as on in System Settings but stop working. Toggle it off and on,
  or `tccutil reset Accessibility com.starterpack.desktop`. For the real
  behaviour use `cargo tauri build --debug --bundles app` and open the
  bundle, or sign the debug binary with a stable identity.
- **The first open in an Electron app** may stay at the screen position
  while the app builds its accessibility tree; the next ones find the caret.
- **The global shortcut** uses Carbon's `RegisterEventHotKey`: no
  permission needed, but it can't fire when another app owns ⌃⌘T, and
  nothing reports that. The shortcut is fixed in code.
- **No translucent background.** Spotlight's vibrancy needs transparent
  windows (the `macos-private-api` feature) and `NSVisualEffectView`
  through another AppKit drop-down.
- **`tauri-nspanel`** is a community plugin relying on class swizzling. Its
  `panel_event!` macro fails clippy under `-D warnings`, which is why blur
  is handled with Tauri's own window event.

## Testing

`cargo test` covers the item list and filter, the placement decisions and
position math, the lookup's short-circuits, the Accessibility error
descriptions, and writing each sample image to a private pasteboard and
reading the representations back.

By hand:

1. `cargo tauri dev` (the Phoenix server isn't needed), or a bundle from
   `cargo tauri build --debug --bundles app` for the real permission
   behaviour.
2. In the main window, **Allow Accessibility access**, grant it to the app
   the dialog names, come back: the section says it's granted.
3. Click into a text field in TextEdit, Safari or Firefox, press ⌃⌘T: the
   popup opens under the caret, footer "At the text caret", input focused,
   the other app still in front.
4. Type `sh`, Enter: the popup closes and `¯\_(ツ)_/¯` is pasted. The main
   window's log shows the paste outcome.
5. ⌃⌘T, then Escape or a click elsewhere: the popup closes.
6. Slack, Discord or Obsidian: the first open may land on the text field or
   the screen, the next ones at the caret. Terminals get the screen
   position with the reason in the footer.
7. Images: type `gif`, pick **Animated GIF**, in turn in Slack or Discord
   (a file upload that animates), Notes or TextEdit (a static picture), a
   browser text area (the PNG, or nothing) and a terminal (nothing). Try
   **WebP image** in Preview with ⌘N: it opens as an image.
