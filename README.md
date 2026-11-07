# Starter Pack

One application in Rust, with Svelte pages, deployed three ways:

* **web**: `axum/`, a web server with the application behind a JSON API
  and the Svelte pages at every other path
* **desktop**: `tauri/`, a Tauri app with the application behind Tauri
  commands and the Svelte pages in its window
* **mobile**: the same Tauri app, built for iOS and Android

```
core/      app_core, the application: one module per resource, with
           list/get/create/update/delete functions returning plain data
axum/      app_web, the web server: app_core over HTTP under /api, and svelte/dist
tauri/     app_tauri, the desktop and mobile app: app_core behind Tauri commands
svelte/    the pages, one Svelte app the web server serves and the Tauri app bundles
bin/dev    starts a deployment in dev mode
phoenix/   the Phoenix version this one replaces in stages, kept as the reference
```

Rust is here for the one language running on the web, the desktop and
phones, not for speed; `AGENTS.md` says how the code is written.

## How a page reaches the application

A page calls `listPosts()` or `getPost(id)` from `svelte/src/lib/core/`.
Inside the Tauri app that is the `list_posts` or `get_post` command
(`tauri/src/posts.rs`); in a browser it is `GET /api/posts` or
`GET /api/posts/:id` (`axum/src/posts.rs`). Both call
`app_core::posts::list_posts` or `get_post` (`core/src/posts.rs`) and
answer with the same JSON, serialized from the same Rust types, so the
page never knows which it is on. The server answers an error as a status
and `{"error": "..."}`, the app as the message, and the page shows the
message either way.

To add to a resource, or add one: the function in `core/`, a route in
`axum/src/router.rs` with its handler, a command in `tauri/src/lib.rs`
with its function, the two calls in `svelte/src/lib/core/`, then the page.

The posts are hard-coded in `core/` for now. The database, who the user
is, and the sync between the apps' local SQLite files and the web server,
the system of record, come in later stages.

## Pages

The Svelte app is one document, `svelte/src/index.html`, and a client-side
router (`svelte/src/lib/router.js`, [sv-router](https://sv-router.dev))
shows the page at the path, swapped in place:

| Path | Shows |
| --- | --- |
| `/` | `HomePage.svelte` |
| `/posts` | `posts/PostsIndexPage.svelte` |
| `/posts/:id` | `posts/PostShowPage.svelte` |

The paths are real paths, named once in `svelte/src/lib/routes.js`, which
pages link through. Every path is reloadable: the dev server, the web
server and the Tauri app all answer an unknown path with `index.html`.

## Running

Prerequisites: Rust and Node with pnpm (`.tool-versions`), and for the
Tauri app the Tauri CLI (`cargo install tauri-cli --version '^2' --locked`)
and Xcode's command line tools.

```sh
bin/dev            # the web: the server on 4000, the pages with hot reload on 1420
bin/dev desktop    # the desktop app, starting the pages' dev server itself
```

`bin/dev` takes its ports from `PORT` when set (the server on `PORT`, the
pages on `PORT + 1000`), so worktrees don't collide; `bin/dev --help` has
the rest. By hand:

```sh
(cd svelte && pnpm install && pnpm build)   # writes svelte/dist
cargo run -p app_web                        # http://localhost:4000, serving svelte/dist
(cd tauri && cargo tauri dev)               # the desktop app on the pages' dev server
(cd tauri && cargo tauri build)             # the app bundle, under target/release/bundle
```

The web server reads `PORT` and `SVELTE_DIST` (default `svelte/dist`),
and logs with `RUST_LOG` (default `app_web=info,tower_http=info`).
`pnpm check` in `svelte/` type-checks the pages.

### Mobile

The Tauri crate is set up for mobile (`tauri::mobile_entry_point`, the
library crate types), and `tauri/gen/apple` is the Xcode project
`cargo tauri ios init` generated, committed as Tauri recommends. Building
for a phone needs:

* Rust through [rustup](https://rustup.rs), not asdf, since the phone
  targets come from it: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`,
  or for Android the four targets in
  [Tauri's prerequisites](https://tauri.app/start/prerequisites/#configure-for-mobile-targets).
* For iOS: Xcode, selected once with
  `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`
  (the Tauri CLI runs `xcodebuild` with a clean environment, so
  `DEVELOPER_DIR` doesn't reach it), and CocoaPods, `brew install cocoapods`.
  The CLI installs `xcodegen`, `libimobiledevice` and `ios-deploy` with
  Homebrew itself. A simulator needs no signing certificate; a device
  needs one, set as `bundle.iOS.developmentTeam` in `tauri.conf.json` or
  `APPLE_DEVELOPMENT_TEAM`.
* For Android: Android Studio with the SDK and NDK, and `ANDROID_HOME`
  and `NDK_HOME` set.

Then `bin/dev ios "iPhone 16"` or `bin/dev android`, which run
`cargo tauri android init` the first time (writing `tauri/gen/android`,
to commit) and open the app in a simulator or emulator. The pages come
from the Svelte dev server, which `TAURI_DEV_HOST` (set by the CLI) makes
listen on this machine's network address so a phone reaches it, see
`svelte/vite.config.js`.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
(cd svelte && pnpm check)
```

## Not yet

* The database: SQLite on the devices, the web server's database, and
  `create_post`, `update_post`, `delete_post` in `core/`
* Who the user is, and logging in
* The sync between the devices' files and the web server
* Tailwind and the daisyUI theme the Phoenix pages use
* The `main` branch's desktop app has a quick search popup, a tmux example
  page and the offline-first pages against Phoenix; they can come back
  from there once the pieces they need are here
