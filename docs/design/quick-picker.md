# Quick picker

⌃⌘T in any app on the Mac opens a small window under the text caret. It
lists the signed-in person's posts, as text, and their image files under
a filter box. ↑/↓ move the selection; Enter takes the selected item, a
click takes the one clicked: it is copied to the clipboard, the window
closes, and it is pasted where the person was typing. Escape or a click
elsewhere closes the window. With nobody signed in, the window says so
and its button brings the app's window to the front on the accounts
page, where a sign-in starts.

macOS only. The shortcut is fixed in code (`popup.rs`).

Ported 2026-11-10 from the v2 spike (branch `v2-elixir-and-rust`,
`tauri/docs/quick-search.md`), where Tauri covered about half of it; the
rest is AppKit, Accessibility and Core Graphics calls made from Rust.

## Code

```
svelte/src/quick-picker.html, quick-picker/   the window's page, an HTML entry of its own
svelte/src/lib/core/                          quickPickerList, quickPickerPaste, ... as every page's calls
tauri/src/quick_picker/
  mod.rs          the commands, and `Item`
  items.rs        posts and image files as items, the last changed first
  popup.rs        the shortcut, open and close, copy and paste
  placement.rs    where the window opens: caret, text field or screen
  panel.rs        the non-activating NSPanel
  unavailable.rs  the same functions off macOS: nothing to open
tauri/src/macos/
  accessibility.rs  Accessibility access, the caret of the app in front
  running_app.rs    the app in front, activating an app
  keyboard.rs       pressing ⌘V
tauri/src/clipboard/                          the image copy the files page uses, and text
tauri/capabilities/quick-picker.json          lets the window listen for its opening
```

The Rust side lists the items, decides where the window opens, copies
and pastes. The page asks for the list each time the window opens,
filters it as the person types, moves the selection and sends the pick.

| Command | Does |
| --- | --- |
| `quick_picker_list` | the posts the user sees and their images the clipboard takes; an error when nobody is signed in |
| `quick_picker_paste` | copies the item (`kind`, `id`), closes the window, presses ⌘V |
| `quick_picker_close` | Escape |
| `quick_picker_open_app` | closes the window, shows the app's window on `/accounts` |
| `quick_picker_accessibility_request` | opens the system's Accessibility dialog |

The `quick-picker-opened` event tells the page each opening, with
whether Accessibility access is granted.

## How it works

**The window** is an `NSPanel` with the non-activating style, through
the `tauri-nspanel` plugin, which swaps the Tauri window's class at
runtime. The panel takes keystrokes without making this app active, so
the app the person was typing in stays in front and receives the paste.
A plain Tauri window can't: showing and focusing it activates the app.
The panel is created hidden at startup: a panel shown in the call that
creates it loses key status while its page loads. Losing key status
closes it, which covers a click elsewhere.

**Placement**, in order: at the caret (`AXSelectedTextRange` and
`AXBoundsForRange` of the focused element of the app in front); under
the focused text field, when it is at most 80 points tall; near the top
of the screen the pointer is on, like Spotlight. Chromium and Electron
apps report nothing until `AXManualAccessibility` is set on them, so a
failed first look sets it and looks again 150 ms later. The
Accessibility calls block on the other app, so they run on their own
thread with a 0.5 s timeout per call; opening waits 150 ms for them,
then opens at the screen position and moves when the caret arrives, if
it is still the same opening. Tauri's `monitor_from_point` takes points
on macOS while `Monitor`'s position and size are physical pixels;
`placement.rs` converts.

**Copying.** An image goes through `files::copy_to_clipboard`, the
files page's copy: one pasteboard item with the file's URL, its own
bytes, and a PNG rendering (a TIFF too when `TIFF_RENDERING` in
`clipboard/macos.rs` is on; off, it is slow for a large image), so each app takes what it
understands and an animated GIF stays animated where GIFs are taken.
The URL is of a copy under the file's own name, in the cache's
`clipboard` folder, which holds only the last one: the account's folder
keeps a file under its id with no extension, and an app that takes the
file would not know it for an image. A post's body is written as a
string.

**Pasting.** The window closes, then ⌘V is posted with Core Graphics
after 200 ms, so the previous app has the keyboard back and Enter is
released. When this app is in front (⌃⌘T pressed in its own window),
the previous app is activated first; macOS may refuse, and then the
item is copied only. Apps with Secure Keyboard Entry drop the ⌘V.

**Accessibility access** is needed for the caret and the paste. Without
it the window opens at the screen position and copies only; its footer
says so and asks for access.

## Limitations

- **Development builds.** Under `cargo tauri dev` macOS attributes the
  Accessibility grant to the terminal app. For a bundle the grant is
  tied to the signature, which an ad-hoc signed build changes on every
  rebuild: toggle it off and on, or `tccutil reset Accessibility
  com.starterpack.app`.
- **The shortcut** can't fire when another app owns ⌃⌘T, and nothing
  reports that.
- **The theme** is read when the page loads, at startup; a theme chosen
  later reaches the picker at the next launch.
- **Thumbnails** are the images themselves, through the `files`
  protocol, so an image not on this device is fetched to be listed.
- **`tauri-nspanel`** is a community plugin that relies on class
  swizzling.

## Testing

`cargo test` covers the items' order and which files are listed, the placement
decisions and position math, the lookup's short-circuits, and the text
written to a pasteboard of the test's own. By hand:

1. `cargo tauri dev`, signed out. ⌃⌘T in another app: the window says
   to sign in; Enter or the button brings the app to the accounts page.
2. Sign in, add a post and a few images (a GIF among them). ⌃⌘T in
   TextEdit: the window opens under the caret, the filter box focused,
   TextEdit still in front. Type to filter; Enter pastes the first
   match; a click pastes the one clicked; Escape closes.
3. Without Accessibility access the footer offers to allow it, and an
   item is copied only: ⌘V pastes it.
4. An animated GIF into Slack or Discord animates; into Notes or
   TextEdit it is a still picture.
5. Sign out, ⌃⌘T: the sign-in message again.
