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
(`cargo install tauri-cli --version '^2' --locked`) and Xcode command line
tools. The main window is plain HTML and JavaScript, so there is no Node
toolchain.

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
cd tauri && cargo tauri dev
```

The app connects to `http://localhost:4000` by default. Set `PHOENIX_URL` to
match the server's port or to use another server, and add its URL to `src-tauri/capabilities/phoenix.json`
so its `/client/new` page may hand the app a token.

Logs go to stderr. Set `RUST_LOG=desktop_lib=debug` for more.

`cargo tauri build` makes the macOS app bundle under `src-tauri/target/release/bundle`.

## tmux example

`src/tmux-example.html` is a spike for driving the user's tmux from the app
(research in `docs/macos-tmux-research.md`). It:

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

## Layout

```
src/                     the main window: index.html, main.js, styles.css,
                         and the tmux example page: tmux-example.*
src-tauri/
  src/lib.rs             the Tauri app: windows, state and commands
  src/phoenix.rs         the websocket API client
  src/token.rs           the stored API token
  src/config.rs          the server URL
  src/tmux_example.rs    the tmux example page's settings and actions
  src/tmux.rs            runs tmux
  src/terminal_app.rs    finds terminal apps, opens and focuses their windows
  src/process.rs         lists processes with ps
  src/claude_code.rs     reads Claude Code's session statuses
  src/git.rs             reads a directory's git status
  Info.plist             merged into the app bundle's Info.plist
  capabilities/          which window may call which command
  tauri.conf.json        the Tauri configuration
```
