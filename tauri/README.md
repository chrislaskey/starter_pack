# Starter Pack desktop app

A [Tauri](https://tauri.app) shell around the Phoenix server in `../phoenix`.

The app has two windows:

* **main**, a local page (`src/`) with a connection indicator for the
  websocket API, and buttons to log in, open the web UI, send a test request
  and log out
* **phoenix**, the Phoenix web UI loaded from the server

## How it connects

1. The user clicks **Log in**. The app opens the Phoenix window on
   `/client/new`, which asks them to log in first when they are not.
2. The user clicks **Connect** on that page. Phoenix creates an API token
   (`App.Data.Accounts.generate_user_api_token/1`) and pushes it to the page,
   whose LiveView hook hands it to the app with the `set_api_token` command.
   That page is the only remote origin allowed to call anything on the app,
   see `src-tauri/capabilities/phoenix.json`.
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

```sh
# Terminal 1: the server, with the websocket API on (config :app, api_socket: true)
cd phoenix && mix phx.server

# Terminal 2: the app
cd tauri && cargo tauri dev
```

The app connects to `http://localhost:4000` by default. Set `PHOENIX_URL` to
use another server, and add its URL to `src-tauri/capabilities/phoenix.json`
so its `/client/new` page may hand the app a token.

Logs go to stderr. Set `RUST_LOG=desktop_lib=debug` for more.

`cargo tauri build` makes the macOS app bundle under `src-tauri/target/release/bundle`.

## Layout

```
src/                     the main window: index.html, main.js, styles.css
src-tauri/
  src/lib.rs             the Tauri app: windows, state and commands
  src/phoenix.rs         the websocket API client
  src/token.rs           the stored API token
  src/config.rs          the server URL
  capabilities/          which window may call which command
  tauri.conf.json        the Tauri configuration
```
