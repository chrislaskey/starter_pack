# Starter Pack

* `phoenix/`, a git submodule with the Phoenix server
* `tauri/`, the macOS desktop app, a Tauri shell around the server

The desktop app shows the Phoenix web UI in a webview, logs in through it,
and holds a websocket connection to the server's API on the Rust side for as
long as it runs. See `tauri/README.md` for how that works and how to run
both.

```sh
git submodule update --init
(cd phoenix && mix setup && mix phx.server)
(cd tauri && cargo tauri dev)
```
