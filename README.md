# Starter Pack

* `phoenix/`, a git submodule with the Phoenix server
* `tauri/`, the macOS desktop app, a Tauri shell around the server

The desktop app shows the Phoenix web UI in a webview, logs in through it,
and holds a websocket connection to the server's API on the Rust side for as
long as it runs. See `tauri/README.md` for how that works and how to run
both.

```sh
git submodule update --init
(cd phoenix && mix setup)
bin/dev
```

`bin/dev` starts the server and the app, on `PORT` and `PORT + 1000` when
`PORT` is set (Code Reviewer sets one per session, so worktrees don't
collide), else on 4000 and 1420. `bin/dev phoenix` and `bin/dev tauri`
start one of them, and `bin/dev --help` has the rest.

The desktop app validates a post with the Rust the Phoenix app writes from
its `App.Data.Post.changeset/3`, and runs itself, see
`phoenix/priv/rust/README.md` and, for the spike behind it,
`phoenix/docs/design/shared-validations.md`.
