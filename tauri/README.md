# Starter Pack desktop app

A [Tauri](https://tauri.app) shell around the Phoenix server in `../phoenix`.

The app has two windows:

* **main**, a local page (`src/`) with a connection indicator for the
  websocket API, and buttons to log in, open the web UI, send a test request
  and log out. It links to the [tmux example](#tmux-example).
* **phoenix**, the Phoenix web UI loaded from the server

## How it connects

1. The user clicks **Log in**. The app opens the Phoenix window on
   `/client/new`, which asks them to log in first when they are not.
2. That page's `ClientConnect` hook (Phoenix's `assets/js/app_client.js`)
   looks for a `window.appClient` object. The app injects one into every
   page of the Phoenix window; its `setApiToken` runs the `set_api_token`
   command. Phoenix creates an API token
   (`App.Data.Accounts.generate_user_api_token/1`), hands it over and moves
   on to the web UI. The Phoenix window is the only remote origin allowed to
   call anything on the app, see `src-tauri/capabilities/phoenix.json`.
3. The Rust side (`src-tauri/src/phoenix.rs`) stores the token in the app
   data directory and connects to `App.Web.ApiSocket` at `/api/websocket`
   with it, in the `Sec-WebSocket-Protocol` header like the `phoenix`
   JavaScript client does. It joins the `api` channel, heartbeats every 30
   seconds and reconnects with a backoff when the connection drops. The
   connection lives for as long as the app runs.
4. The main window shows the state (`disconnected`, `connecting`,
   `connected`, `joined`) from the `connection-status` events the Rust side
   emits. **Send auth.token.get** makes a request over the channel and shows
   the reply. **Log out** deletes the token on the server (`auth.token.delete`)
   and forgets it locally.

On the next launch the app reconnects with the stored token without logging
in again. A token the server rejects is dropped, and the app asks to log in.

## Running

Prerequisites: Rust (see `../phoenix/.tool-versions`), the Tauri CLI
(`cargo install tauri-cli --version '^2' --locked`), Xcode command line
tools, and Node and pnpm (see `.tool-versions`) for the main window, which is
a [Svelte](https://svelte.dev) app built with [Vite](https://vite.dev).
Install its packages once with `pnpm install`.

The server only accepts API connections when `API_SOCKET=true`. It reads
`.env` in the Phoenix directory, so the simplest is a gitignored
`phoenix/.env`:

```sh
API_SOCKET=true
PORT=4010
```

```sh
# Terminal 1: the server
cd phoenix && mix phx.server

# Terminal 2: the app
cd tauri && pnpm install && cargo tauri dev
```

The app connects to `http://localhost:4000` by default. Set `PHOENIX_URL` to
match the server's port or to use another server, and add its URL to `src-tauri/capabilities/phoenix.json`
so its `/client/new` page may hand the app a token.

Logs go to stderr. Set `RUST_LOG=desktop_lib=debug` for more.

`cargo tauri dev` starts the Vite dev server on port 1420 (`pnpm dev`) and
loads the main window from it, so changes to `src/` show up without a
restart. `pnpm check` type-checks the Svelte components.

The main window uses Svelte components Phoenix generates from its HEEx
function components (see `../phoenix/priv/svelte/README.md`), imported from
the submodule as `$phoenix/...`, like
`import Header from "$phoenix/components/Header.svelte"`. So the `phoenix`
submodule has to be checked out to build the app. Don't copy or edit those
files here: change the component in Phoenix and run `mix app.svelte` there.
They're styled like on the web: `src/app.css` runs Tailwind v4 with
Phoenix's daisyUI theme (`../phoenix/assets/css/theme.css`) over those
files only, and makes the `hero-*` icon classes from the `heroicons` npm
package (`vendor/heroicons.js`). `vite.config.js` points the theme's
daisyUI imports, which name Phoenix's mix deps, at the `daisyui` npm
package, so building needs no Elixir. Keep `tailwindcss` and `daisyui` at
the versions in Phoenix's `config/config.exs` and `mix.exs`. The app's own
styles are in Tailwind's base layer, so the components' classes win over
them. Don't give the app's own elements class names daisyUI or Tailwind
also define, like `card`, `status`, `label`, `stack`, `container` or
`grow`: once a Phoenix component uses one, its styles apply to the app's
elements too.

The status page header has Phoenix's theme toggle. Like on the web, its
buttons dispatch a `phx:set-theme` event, and Phoenix's theme script,
`phoenix/assets/js/theme_script.js`, sets `data-theme` on `<html>` and keeps
the choice in `localStorage`. Phoenix's root layout and this app both
inline that one file: `vite.config.js` replaces each page's
`<!-- phoenix-theme-script -->` marker with it, so it runs before the first
paint. The app's own dark colors follow `data-theme` too.
The app's buttons are Phoenix's `Button`, and its CSS variables avoid the
ones daisyUI sets, like `--border`. Tailwind scans `src/` too, so the app
can use Tailwind and daisyUI classes, like `btn-sm`, in its own markup.

Their `phx-click`, `phx-submit` and `phx-change` events reach the
`app:emit` function `src/lib/app-emit.js` provides, which only logs them
for now.

`cargo tauri build` builds the main window into `dist/` (`pnpm build`) and makes the macOS app bundle under `src-tauri/target/release/bundle`.

## tmux example

`src/tmux-example.html` (`src/tmux-example/`) is an example of driving the
user's tmux from the app (`docs/tmux-example.md` says how to remove it). It:

1. Guesses the user's shell, terminal app (the first installed of iTerm2,
   Ghostty, kitty and WezTerm, else the Terminal app that comes with macOS)
   and tmux, and shows them in a collapsed settings form the user can
   correct.
2. Shows the tmux sessions on the user's tmux server, refreshing every 5
   seconds. For each window and pane it shows:
   - the program in the foreground, and whether it's a shell at a prompt
   - the Claude Code status, when Claude Code runs in the pane
   - the pane's directory
   - its git status, like a shell prompt: branch, `✔` staged, `✘` unstaged,
     `✂` untracked, `⚠` conflicted, `↑`/`↓` ahead of and behind the upstream
   - the processes on the pane's tty, with their state and CPU use

   The processes come from `ps`, the Claude Code statuses from
   `~/.claude/sessions` (undocumented), and the git status from
   `git --no-optional-locks status --porcelain=v2`. A separate list shows
   every running Claude Code session, in tmux or not.
3. Creates a session running the user's login shell in a directory, types a
   command into it (`claude` by default) and opens it in the terminal app. The
   directory defaults to the root of the git repository the app runs in (the
   nearest parent with a `.git`), else the directory the app runs in. Under
   `cargo tauri dev` that's the repository root. Apps launched from Finder run
   in `/`, so for those the default is the home directory. A leading `~` is
   expanded.
4. **Open** on a session brings its terminal window to the front, or opens a
   new window attached to it. Terminal and iTerm2 find the window by the tty
   tmux reports for the session's clients (AppleScript). Ghostty
   (AppleScript) and WezTerm (`wezterm cli`) find it by the title tmux gives
   it, `tmux:<session>`. kitty always gets a new window.
5. Shows `tmux attach -t <session>` to copy into any terminal as a fallback.

Opening and focusing windows in Terminal, Ghostty and iTerm2 sends Apple events, so
macOS asks once whether the app may control that terminal app. Under
`cargo tauri dev` the question is about the terminal running the command.

## Quick search

⌃⌘T in any app opens a small popup (`src/quick-search.html`,
`src/quick-search/`) under the text caret, else under the focused text
field, else near the top of the screen like Spotlight. It has a filter box
over three text snippets and five sample images (`src/public/quick-search/images/`);
↑/↓ move the selection, Enter copies the selected item to the clipboard,
closes the popup and pastes it where the user was typing. Escape or clicking
elsewhere closes it.

The popup is a non-activating `NSPanel`, so it takes keystrokes while the
app the user was typing in stays in front and receives the paste. Finding
the caret and pasting need Accessibility access (System Settings › Privacy
& Security › Accessibility); the status page asks for it. Under
`cargo tauri dev` macOS checks the terminal app that runs it, so grant
access to that app. `docs/quick-search.md` describes how it's built, its
limitations, how to test it by hand and how to remove it; the Rust side is
`src-tauri/src/quick_search/` with the AppKit, Accessibility and Core
Graphics calls in `src-tauri/src/macos/`.

## Layout

```
src/                     the main window, a Svelte app with one HTML entry per page
  index.html, main.js    the status page, mounting StatusPage.svelte
  tmux-example.html      the tmux example page, mounting tmux-example/
  tmux-example/          the tmux example page's components and styles
  quick-search.html      the quick search popup, mounting quick-search/
  public/                files served as they are: the quick search sample images
  lib/                   shared by both pages: the log, the app:emit handler
  app.css, styles.css    Tailwind with Phoenix's theme, and the app's own styles
vite.config.js           the Vite build: the pages, Tailwind, the $phoenix alias
                         and the dev server port
vendor/heroicons.js      the Tailwind plugin making the hero-* icon classes
src-tauri/
  src/lib.rs             the Tauri app: windows, state and commands
  src/phoenix.rs         the websocket API client
  src/token.rs           the stored API token
  src/config.rs          the server URL
  src/tmux_example.rs    the tmux example page's settings and actions
  src/quick_search/      the quick search popup: hotkey, window, placement, items, paste
  src/macos/             AppKit, Accessibility and Core Graphics calls Tauri lacks
  src/tmux.rs            runs tmux
  src/terminal_app.rs    finds terminal apps, opens and focuses their windows
  src/process.rs         lists processes with ps
  src/claude_code.rs     reads Claude Code's session statuses
  src/git.rs             reads a directory's git status
  Info.plist             merged into the app bundle's Info.plist
  capabilities/          which window may call which command
  tauri.conf.json        the Tauri configuration
docs/                    how each example is built and removed; docs/suggestions/ for ideas not built
```
