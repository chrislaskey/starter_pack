# tmux example

The `/tmux-example` page (`src/tmux-example/`) lists the user's tmux sessions with what runs in
each pane, creates sessions running a command, and opens them in the
user's terminal app. The "tmux example" section of `README.md` describes
what it does and how; the code is `src-tauri/src/tmux_example.rs` with the
`tmux`, `terminal_app`, `process`, `claude_code` and `git` modules it uses,
and the page in `src/tmux-example/`.

## Removing it

The page is an example. To build an app without it, delete these and the
app builds and runs as before:

1. `src-tauri/src/tmux_example.rs`, `tmux.rs`, `terminal_app.rs`,
   `process.rs`, `claude_code.rs` and `git.rs`.
2. In `src-tauri/src/lib.rs`: their six `mod` lines, the
   `use terminal_app::…` and `use tmux_example::…` lines, the four
   `*_tmux_*` command functions and their entries in `generate_handler!`.
3. In `src-tauri/build.rs`: the same four command names.
4. The four `allow-*-tmux-*` lines in `src-tauri/capabilities/default.json`.
5. `NSAppleEventsUsageDescription` in `src-tauri/Info.plist`, which only
   the terminal app control needs.
6. `src/tmux-example/`, its route in `src/lib/router.js` and its path in
   `src/lib/routes.js`, and its styles' import in `src/app.css`.
7. The "Examples" section with the link in `src/StatusPage.svelte`.
8. This file and the "tmux example" section of `README.md`.

No Cargo dependency is only the example's: `tokio`'s `process` feature can
go from `Cargo.toml` if nothing else runs commands.

Then `cargo clippy --all-targets -- -D warnings`, `cargo test` and
`pnpm check` should pass. `grep -ri "tmux" .` finds anything missed.
