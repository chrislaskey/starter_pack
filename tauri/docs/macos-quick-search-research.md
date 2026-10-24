# macOS quick search popup: what Tauri can and can't do

A spike (2026-10-24) testing Tauri against a feature that is routine in a
native Swift/AppKit app: a global hotkey (⌃⌘T) opens a Spotlight-style popup
next to the text caret in whatever app is in front, the user filters a few
choices, Enter copies the first match to the clipboard, the popup closes and
the value is pasted where the caret was.

The code is `src-tauri/src/quick_search.rs` (the hotkey, panel and
commands), `src-tauri/src/macos.rs` (the AppKit, Accessibility and Core
Graphics calls Tauri has no API for) and `src/quick-search/` (the popup
page). The status page (`src/StatusPage.svelte`) shows whether Accessibility
access is granted and logs each pick.

## Verdict in short

Everything in the brief was built. Tauri itself covered about half of it; the
rest needed AppKit calls Tauri doesn't wrap. None of it needed Swift: the
`objc2-app-kit`, `core-foundation` and `core-graphics` crates expose the
same frameworks a Swift app would call, so the drop-down is "Rust calling
AppKit", not "Rust calling Swift". A Swift toolchain only becomes necessary
for things with no C/Objective-C surface (SwiftUI views, Swift-only
frameworks), none of which this feature needs.

| Step | Tauri API | Had to drop down to |
| --- | --- | --- |
| Global hotkey ⌃⌘T | `tauri-plugin-global-shortcut` (official) | — |
| Create a borderless, floating, always-on-top window | `WebviewWindowBuilder` | — |
| Make it a *non-activating* panel that takes keystrokes without activating the app | none | `NSPanel` with `nonactivatingPanel` style, via the community `tauri-nspanel` plugin (swizzles the window's class) |
| Keep it out of Mission Control, show it over full screen apps | none | `NSWindowCollectionBehavior` (also via `tauri-nspanel`) |
| Position Spotlight-like on the screen the pointer is on | `AppHandle::cursor_position`, `monitor_from_point`, `Monitor::work_area` | — |
| Position at the text caret, or text field, of the app in front | none | Accessibility API: `AXFocusedUIElement` → `AXSelectedTextRange` → `AXBoundsForRange`, else `AXFrame`; `AXManualAccessibility` to wake Electron apps (hand-declared externs, no objc2 crate covers HIServices) |
| Remember which app was in front | none | `NSWorkspace.frontmostApplication` |
| Copy text (and later html, rtf, image, files) | `tauri-plugin-clipboard` (community, CrossCopy), from Rust | — |
| Close the popup, keep the other app in front | `Panel::hide` | — (follows from the non-activating panel) |
| Paste into the other app | none | `CGEventCreateKeyboardEvent` ⌘V posted to the HID tap; `NSRunningApplication.activate` as a fallback |
| Ask for Accessibility access | none | `AXIsProcessTrustedWithOptions` with the prompt option |

## What was tried, and what happened

1. **Plain Tauri window as the popup.** A `WebviewWindowBuilder` window
   with `decorations(false)`, `always_on_top(true)`, `focused(true)`
   shows fine, but showing and focusing it activates the app: the previous
   app loses focus, its caret disappears, and a later ⌘V would land in the
   popup's own webview. Tauri has no notion of a window that is key without
   the app being active, because neither tao nor wry model `NSPanel`.
   Tauri's `focusable(false)` keeps the previous app in front but then the
   popup can't receive keystrokes either, so it's not an option for a
   search box. **Rejected.**
2. **`tauri-nspanel` (community, 2.1).** Converts the Tauri window's
   `NSWindow` into an `NSPanel` subclass at runtime (`object_setClass`) and
   exposes style mask, collection behavior, level and the key/order calls.
   With `StyleMask::empty().borderless().nonactivating_panel()`,
   `no_activate(true)` during creation and `show_and_make_key()`, the panel
   is key while the previous app stays active, which is exactly what
   Spotlight and Raycast do. **Used.** Caveats: it's a third-party plugin
   relying on an unstable trick (class swizzling of Tauri's window); its
   `panel_event!` macro requires a `-> ()` return type that clippy rejects
   under `-D warnings`, so losing focus is handled with Tauri's own
   `WindowEvent::Focused(false)` instead, which still fires because the
   plugin leaves Tauri's window delegate in place when no handler is set.
3. **Caret position.** Tauri only knows the mouse pointer. The
   Accessibility API gives the caret's screen rectangle for the focused
   element of the app in front, in points with a top-left origin, the same
   coordinate space as Tauri's logical window positions. This needs
   Accessibility access, like it would in Swift. It also depends on the
   other app: native `NSTextView`/`NSTextField`, Safari, Firefox and Xcode
   report it; terminals have no text caret to report. Chromium and Electron
   apps (Slack, Discord, VS Code, Obsidian) answer nothing until
   accessibility is switched on in them, which an assistive app does by
   setting `AXManualAccessibility` on the app's element (Electron's
   accessibility guide); `text_position` does that and looks again after
   150 ms. Three anchors, in order: the caret, the focused text field's
   frame (`AXFrame`, else `AXPosition` + `AXSize`), and Spotlight-like near
   the top of the screen the pointer is on. The `quick-search-opened` event
   tells the page which one was used and why the caret wasn't, and the
   page shows it in its footer. **Built, with fallbacks.** First test round
   (2026-10-24): every app fell back, including Firefox, which does report
   carets, and pasting failed too. Both need Accessibility access, so the
   likely cause is the development-build permission problem below rather
   than the apps; the second round shows the access state in the popup.
4. **Pasting.** Posting a ⌘V key event with Core Graphics works from Rust
   as it does from Swift, and has the same requirement: the app must have
   Accessibility access, otherwise macOS drops the events silently. Because
   the panel never activated this app, the previous app is still active and
   receives the event; `bring_previous_app_to_front` only calls
   `activateWithOptions` when this app did become active (say the user
   clicked the main window first). Since macOS 14 that activation is
   "cooperative" and the system may refuse it; the outcome is reported as
   `previous_app_not_activated`. **Built.** Apps with Secure Keyboard Entry
   on (Terminal's option, password fields) won't accept the synthetic ⌘V;
   neither would they from a Swift app.
5. **Accessibility prompt.** `AXIsProcessTrustedWithOptions` has no objc2
   binding, so `macos.rs` declares the HIServices functions by hand and
   links the ApplicationServices framework. The status page asks for access
   and re-checks when the window regains focus, because the grant happens
   in System Settings. **Built.**

## Gaps and limitations found

- **Development builds and Accessibility access.** Two separate traps.
  First, macOS attributes privacy permissions to the *responsible* app, and
  for a process started from a terminal that is the terminal app: under
  `cargo tauri dev` launched from Ghostty, `AXIsProcessTrusted` answers for
  Ghostty, the system dialog names Ghostty, and the grant has to go to
  Ghostty. Second, for a bundle, macOS ties the grant to the code
  signature; an ad-hoc signed debug build changes signature on every
  rebuild, so the grant may show as on in System Settings but stop working.
  Toggle it off and on, or `tccutil reset Accessibility
  com.starterpack.desktop`. For reliable testing use a bundle from
  `cargo tauri build --debug --bundles app` opened from Finder or `open`,
  or sign the debug binary with a stable identity. An Xcode project has the
  second issue too, but Xcode signs debug builds with a stable development
  certificate, which is why it never shows up there.
- **Pasting and finding the caret fail together** whenever the above isn't
  sorted, and they fail silently: `CGEventPost` drops the events, the AX
  calls return `kAXErrorAPIDisabled`. The status page and the popup footer
  now say whether access is granted, and the main window's log shows the
  paste outcome (`no_accessibility_access`) and the anchor reason.
- **Pointer coordinates.** Tauri reports the pointer in the *primary*
  display's physical pixels; window positions take logical points. The
  conversion in `screen_position` uses the primary display's scale, so on a
  mixed-DPI setup the pointer may be mapped to the wrong display and the
  popup open on the wrong screen. AppKit gives `NSEvent.mouseLocation` in
  points directly; a fix is to read that through `objc2-app-kit` instead
  of Tauri.
- **Accessibility calls block, and must not run on the main thread.** Each
  AX call is synchronous IPC with the other app, with a default timeout of
  several seconds; the hotkey handler runs on the main thread. Once access
  was granted the popup took 1–3 s to appear with a beachball, worst over
  this app's own windows, where the main thread was asking itself and
  could only time out. Now `start_text_lookup` runs the calls on a thread
  with `AXUIElementSetMessagingTimeout` at 0.5 s, the hotkey waits at most
  150 ms for the answer, otherwise the popup opens at the screen position
  and `move_when_found` moves it when the caret arrives
  (`quick-search-anchored`). The popup never asks this app itself.
- **A huge "text field" put the popup off screen.** In Electron apps and
  editors the focused element is often the whole web area or editor, so
  "under the focused text field" meant below the screen, and the clamp
  only looked for a screen under that point. Result: an invisible popup
  that still had the keyboard, so typing and Enter worked blind. Now the
  clamp uses the screen the text is on (else the pointer's), and a focused
  element taller than 80 points doesn't count as a text field. The main
  window's log shows every open with its anchor, position and reason.
- **`monitor_from_point` takes points on macOS, the rest of `Monitor` is
  pixels.** tao's implementation compares the point against
  `CGDisplayBounds`, which are in points, while `Monitor::position`,
  `size` and `work_area` are physical pixels. Passing a physical point
  found no display at all ("no screen under the pointer"); `screen_at`
  passes the logical point and divides the work area by the scale factor.
- **Waking an Electron app's accessibility takes a moment.** The 150 ms
  wait between `AXManualAccessibility` and the second look runs on the
  lookup thread, so the popup opens at the screen position first and moves
  if the caret turns up. The first open in such an app may stay at the
  screen position; the next ones should find the caret.
- **Blur, vibrancy, transparency.** The popup has an opaque card background
  with rounded corners (the plugin sets the content view's layer corner
  radius). A translucent `NSVisualEffectView` background like Spotlight's
  isn't reachable through Tauri: transparent windows on macOS need the
  `macos-private-api` Cargo feature (which Apple may reject in App Store
  review) and the material needs the community `window-vibrancy` crate or
  another AppKit drop-down. Not tried.
- **A panel shown in the call that creates it loses key status.** Observed
  in the logs: create + `show_and_make_key` in one go gives `Focused(true)`
  then `Focused(false)` within the same millisecond, while the webview's
  page is still loading, so the blur handler closed the popup at once. The
  second open of the same panel stayed key. The panel is therefore created
  hidden at startup (`register`), and the first ⌃⌘T only shows it. A native
  app would preload its panel the same way, but it wouldn't have a webview
  load in the middle.
- **Activation state after launch is unreliable from `cargo run`.**
  `NSApplication.isActive` reported `true` while `NSWorkspace` and
  `lsappinfo front` said Ghostty was in front, both under `cargo tauri dev`
  and from a bundle opened with `open`, and `NSApplication.deactivate` had
  no effect. `macos::is_this_app_in_front` therefore compares the frontmost
  app's pid with this process instead of asking `isActive`.
- **First keystroke.** The page focuses the input on the
  `quick-search-opened` event. If WebKit doesn't take first responder in a
  non-activating panel on some macOS version, the fix is
  `Panel::make_first_responder` on the webview's `NSView`. Not needed so
  far; keep an eye on it.
- **Global shortcut plugin.** Uses Carbon's `RegisterEventHotKey`, which
  needs no permission but can't fire when another app owns the same
  combination, and gives no error about it. The shortcut is fixed in code;
  a rebindable one would need `unregister`/`register` and a settings UI.
- **No menu bar / Dock presence choice.** A Spotlight-like tool usually
  runs as an agent (no Dock icon). Tauri can do that with
  `app.set_activation_policy(ActivationPolicy::Accessory)` and a tray icon,
  not done here.
- **Three fixed choices.** The list lives in Rust (`CHOICES`), filtering in
  the page. Real data would be a command or an API request.

## Images and other non-text clipboard content

The macOS clipboard (`NSPasteboard`) holds *items*, each with several
*representations* keyed by uniform type identifier. The copying app writes
every representation it can; the pasting app reads the first one it
understands, in the item's order. That is why one copy can paste as a file
in Slack, an image in Preview and nothing in a terminal.

### What the popup writes for an image

`macos::copy_image_to_clipboard` builds one `NSPasteboardItem` with, in
this order:

| Representation | Type | Who takes it |
| --- | --- | --- |
| File URL to a copy in the app's cache directory | `public.file-url` (AppKit serves the legacy `NSFilenamesPboardType` from it; an item refuses to hold that type itself, found by the unit test) | Finder, Slack, Discord, Mail, Messages, Notes, browsers' `clipboardData.files`: anything that attaches or uploads files. Keeps a GIF's animation and a WebP's format. |
| The bytes as they are | `public.png`, `public.jpeg`, `com.compuserve.gif` or `org.webmproject.webp` | Apps that read that exact format (few read GIF or WebP data directly). |
| A PNG rendering | `public.png` | Browsers, Electron apps, most editors. The first frame of an animated GIF. |
| A TIFF rendering | `public.tiff` | Older and native apps (TextEdit, Preview, Pages). |

The renderings come from AppKit (`NSImage` → `NSBitmapImageRep`), which
decodes PNG, JPEG, GIF and WebP (macOS 11+). Neither Tauri nor the
clipboard plugins can write several representations to one item: the
official `tauri-plugin-clipboard-manager` writes text or an RGBA image, the
community `tauri-plugin-clipboard` writes text, HTML, RTF, *an* image (as
PNG/TIFF via `clipboard-rs`) *or* a list of file URLs, each call replacing
the previous. So images go through `NSPasteboard` directly; text still goes
through the community plugin. Unit tests (`macos::tests`) write the samples
to a private pasteboard and read the representations back.

What each target app does with the paste is for the hand test. Expected,
from how these apps handle the pasteboard, to be confirmed: Slack, Discord
and Messages upload the file (animated GIF stays animated); browsers and
Electron text fields take the PNG, or the file where the site handles
`clipboardData.files`; TextEdit and Notes insert the image, static;
terminals and plain text fields take nothing, and the ⌘V is a no-op. The
popup can't know in advance which the app in front will do.

### What else lives on the macOS clipboard

Roughly in order of how often apps meet them:

- **Text** in several flavours on one item: `public.utf8-plain-text`,
  `public.rtf` (styled), `public.html`, and Apple's
  `com.apple.flat-rtfd` (RTF with embedded images). Copying styled text
  from a browser writes all of them; pasting into a plain field takes the
  plain one. The community plugin writes text, HTML and RTF.
- **URLs**: `public.url` with a `public.url-name` title, next to the plain
  text.
- **Files and folders**: `public.file-url` items, one per file, as Finder
  writes them. Both plugins can write a file list.
- **Images**: `public.tiff` and `public.png` are the lingua franca; the
  source format rides along when the copying app bothers. Screenshots are
  PNG. The plugins write one image as PNG/TIFF.
- **PDF** (`com.adobe.pdf`): Preview and vector apps write it next to an
  image; Pages, Keynote and Mail take it.
- **File promises** (`com.apple.pasteboard.promised-file-url`,
  `NSFilePromiseProvider`): the file is produced only when the receiver
  asks, how Photos, Mail attachments and browser image drags work. Writing
  one needs a provider object kept alive until the paste happens; reading
  one needs a receiver and a destination folder. Not reachable from the
  plugins, doable with AppKit.
- **Colours** (`com.apple.cocoa.pasteboard.color`), **fonts and rulers**
  (`NSFont` and `NSRuler` pasteboards, separate named pasteboards), and
  **sounds** (`NSSound`): niche, AppKit only.
- **App-private types**: Keynote shapes, Figma nodes, Sketch layers,
  Xcode's `com.apple.dt.…` types; often JSON or a plist under a reverse-DNS
  name, next to a public fallback. Any app can write one with
  `setData_forType`; only the owning app reads it.
- **Markers, not content**: `org.nspasteboard.ConcealedType` (password
  managers: "don't record this"), `org.nspasteboard.TransientType` (don't
  keep in history), `org.nspasteboard.source` (the writing app's bundle
  id). Clipboard managers honour them; the popup should add the source
  marker, and the concealed one if it ever pastes secrets.
- **Universal Clipboard** (Handoff to iPhone/iPad) is automatic for text
  and images on the general pasteboard; nothing to do, except that large
  items are transferred lazily.
- **Multiple items**: Finder writes one item per file; Photos one per
  image. Readers that want one item take the first.

Everything above is reachable from Rust through `objc2-app-kit`, as the
image code shows. The plugins cover text, HTML, RTF, one image and file
lists, which is the common 99%; anything richer (several representations,
promises, private types, markers) means the drop-down.

## Where Swift would still be needed

- SwiftUI or AppKit views inside the popup instead of a webview.
- Frameworks without a C/Objective-C surface. For those the `swift-rs` crate
  compiles a Swift package during `cargo build` and exposes `@_cdecl`
  functions to Rust; it is the usual bridge in Tauri apps. Not tried here
  because nothing required it.

## What was verified without a hand on the keyboard

The agent that built this can't press keys or take screenshots on this Mac,
so the checks were: `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test`, `pnpm check`, `pnpm build`, and runs of the app (under
`cargo tauri dev` and as a bundle launched with `open`) with a temporary
self-test that called `toggle` after startup. The logs showed the hotkey
registering, the panel being created, the pointer position and the
frontmost app (Ghostty) being read, the panel becoming key, and Ghostty
still in front afterwards per `lsappinfo front`. The second round's
self-test showed the screen fallback landing centered at a fifth of the
screen height, and the Accessibility calls failing cleanly with "API
disabled" rather than crashing, since the agent's shell has no
Accessibility access either. The third round ran with access granted (to
the terminal, which covers the agent's shell): over Ghostty the lookup
answered on its thread, reported the focused element as the whole
1728×1084 window, rejected it as a text field and opened at the screen
position, the entire toggle taking 20 ms on the main thread. By hand, the
user confirmed typing, Enter and the paste work. Not verified by the
agent: the caret and text-field anchors in apps that report them, the
Electron wake-up, and the late move. Those are the steps below.

## Testing by hand

1. `cargo tauri dev` (the Phoenix server isn't needed for the popup), or
   for the real permission behaviour
   `cargo tauri build --debug --bundles app` and open the bundle from
   `src-tauri/target/debug/bundle/macos`.
2. In the main window, **Allow Accessibility access**. The system dialog
   names the app macOS holds responsible (the terminal under `cargo tauri
   dev`); grant it to that one, come back: the section should say it's
   granted. Until it does, the popup's footer shows a red "No Accessibility
   access" line and nothing below will paste.
3. Click into a text field in another app (TextEdit is a good first test,
   Safari or Firefox a second), press ⌃⌘T: the popup opens under the caret,
   footer "At the text caret", with the input focused and the other app
   still in front.
4. Type `sh`, press Enter: the popup closes and `¯\_(ツ)_/¯` is pasted. The
   main window's log shows the pick and the paste outcome. ↑/↓ move the
   selection; Enter picks the selected one.
5. Press ⌃⌘T again and click elsewhere or press Escape: the popup closes.
6. Repeat in Slack, Discord or Obsidian: the first open may land on the
   text field or the screen while the app builds its accessibility tree,
   the next ones should find the caret. Terminals always get the screen
   position, footer "No caret found: …" with the reason.
7. Images: type `gif`, pick **Animated GIF**, Enter, in turn in Slack or
   Discord (expect a file upload that animates), Notes or TextEdit (expect
   a static picture), a browser text area (expect the PNG, or nothing) and
   a terminal (expect nothing). The main window's log lists the
   representations written, like `public.file-url, com.compuserve.gif,
   public.png, public.tiff`. Try **WebP image** in Preview with ⌘N (new
   from clipboard): it should open as an image, which means AppKit decoded
   it for the renderings.
