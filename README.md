# Starter Pack

One application in Rust, with Svelte pages, deployed three ways:

* **web**: `axum/`, a web server with the application behind a JSON API
  and the Svelte pages at every other path
* **desktop**: `tauri/`, a Tauri app with the application behind Tauri
  commands and the Svelte pages in its window
* **mobile**: the same Tauri app, built for iOS and Android

```
core/      app_core, the application: one module per resource, with
           list/get/create/update/delete functions returning plain data,
           the database they read and write, and its migrations
axum/      app_web, the web server: app_core over HTTP under /api, and svelte/dist
tauri/     app_tauri, the desktop and mobile app: app_core behind Tauri commands
svelte/    the pages, one Svelte app the web server serves and the Tauri app bundles
bin/dev    starts a deployment in dev mode
phoenix/   the Phoenix version this one replaces in stages, kept as the reference
```

Rust is here for the one language running on the web, the desktop and
phones, not for speed; `AGENTS.md` says how the code is written.

## How a page reaches the application

A page calls `postsList()`, `postsGet(id)`, `postsCreate(body)`,
`postsUpdate(id, body)` or `postsDelete(id)` from `svelte/src/lib/core/`.
Inside the Tauri app each is a command of the same name
(`tauri/src/posts.rs`); in a browser each is a request to the JSON API
(`axum/src/posts.rs`):

| Call | Request |
| --- | --- |
| `postsList()` | `GET /api/posts`, each post with its owner and communities |
| `postsGet(id)` | `GET /api/posts/:id` |
| `postsCreate(body, communityIds)` | `POST /api/posts` with `{"body": "...", "community_ids": [...]}`, 201 |
| `postsUpdate(id, body, communityIds)` | `PUT /api/posts/:id` with the same |
| `postsDelete(id)` | `DELETE /api/posts/:id`, 204 |
| `communitiesList()` | `GET /api/communities`, each with the user's membership or null |
| `communitiesCreate(name)` | `POST /api/communities` with `{"name": "..."}`, 201 |
| `membershipsJoin(communityId)` | `PUT /api/memberships/:community_id` |
| `membershipsLeave(communityId)` | `DELETE /api/memberships/:community_id`, 204 |
| `usersMe()` | `GET /api/users/me` |

Both call the function in `app_core::posts` (`core/src/posts.rs`),
`posts::list`, `posts::get` and so on, named through their module as the
standard library's `fs::read` is, as the user acting, see [Who the
user is](#who-the-user-is), and answer with the same JSON, serialized
from the same Rust types, so the page never knows which it is on. The
rules live there too: a post's body is trimmed and may not be blank, and
only its owner edits or deletes it. The server answers an error as a
status, 404 for a missing post, 403 for someone else's and 422 for a
blank one, and `{"error": "..."}`; the app answers the message; and the
page shows the message either way.

To add to a resource, or add one: the function in `core/`, a route in
`axum/src/router.rs` with its handler, a command in `tauri/src/lib.rs`
with its function, named with the resource (`posts_list`, its name on
the wire too), the two calls in `svelte/src/lib/core/`, then the page.

How the apps' local databases and the web server, the system of record,
converge is under "Sync".

## Who the user is

Every core function takes the `users::User` acting (`core/src/users.rs`):
what they write is theirs, `owner_id` on a post, and what they read is
what they own or what is in a community of theirs, see
[Communities](#communities). `app_users` is the app's view of each account, one row
per person with their email, written by the server at registration and
synced to devices like any row; credentials never leave the server and
come with the login, later, in a table of their own with the same ids.

The server resolves the user of a request from its `Authorization:
Bearer` token, a row of `users_tokens` (`core/src/tokens.rs`), which is
how the apps call it: the token is the `api_token` device setting, sent
by the HTTP transport, and the user is the `signed_in_user` device
setting. A token the server does not know is answered 401.

**There is no login yet.** Until there is, every deployment acts as one
stand-in, `users::dev_user`: the web server answers a request with no
token as it, which is what the pages in a browser send, and a device
starts signed in as it with no token. Those are the two places a login
replaces, `axum/src/current_user.rs` and the two settings' first values
in `core/src/device_settings.rs`. To try a second user over HTTP:

```sh
cargo run -p app_web -- users create alice@example.com   # prints her token
curl -H "Authorization: Bearer <token>" localhost:4000/api/posts
```

## Communities

A community (`core/src/communities.rs`) is the soft boundary inside a
deployment: any user creates one and joins it, every user sees every one
and may join or leave (`core/src/memberships.rs`), and a post is in the
communities its owner chose, zero or more
(`core/src/posts_communities.rs`), which is what the New and Edit pages'
checkboxes set. A user sees their own posts and the posts in their
communities; leaving a community takes the member's own posts there
along, and rejoining brings them back. The decisions are
`phoenix/docs/design/multi-tenancy.md`, the port is in
`docs/design/offline-sync.md` under "Communities and what a user sees",
and what was left out is `docs/suggestions/communities.md`.

## The database

Every deployment opens a database of its own and runs the same migrations
on it, so all three have the same tables: the web server's is the system
of record the apps will sync with; the desktop and mobile apps each keep a
SQLite file on the device, so they work with the network off. That is
`core/src/database.rs`: a `Database` enum of the open connection pool,
SQLite, or PostgreSQL for a web server that has outgrown a file. Every
core function takes a `Deployment` (`core/src/deployment.rs`), the
database with its role, `Server` for the web server and `Device` for the
apps, and runs in one `Transaction` on it, so a row and the event that
changed it are written together or not at all. Each statement goes
through the `sql!` macro to the database's own module of plain SQL
(`core/src/sqlite/posts.rs`, `core/src/postgres/posts.rs`), so the rules
of a resource are written once and the SQL once per database.

The database is [sqlx](https://github.com/launchbadge/sqlx): async, one
library for both databases, with `$1` placeholders in both. Ids are
UUIDv7, made by whichever device writes the row, so an offline write never
collides; they are text in SQLite, so the file reads well in the `sqlite3`
shell. Times are `DateTime<Utc>` to the microsecond, RFC 3339 text in SQLite
and `timestamptz` in PostgreSQL. The migrations are `.sql` files under
`core/migrations/sqlite/` and `core/migrations/postgres/`, one per
database since their SQL differs, named `<version>_<name>.sql` with the
name resource first (`posts_create`), embedded
at build time by sqlx's `migrate!` and run when a database opens; add
one by hand or with the sqlx CLI (`cargo install sqlx-cli`, then
`sqlx migrate add --source core/migrations/sqlite <name>`). `core/`'s
`sqlite` feature is on by default and the web server turns on `postgres`
too; the Tauri app only ever has SQLite, so no PostgreSQL client is
compiled into the desktop or phone build.

Where each deployment's file is:

* The web server: `DATABASE_URL`, `sqlite:<path>` relative to `axum/`,
  by default `sqlite:data/app.db`, a gitignored file in the worktree, or
  `postgres://user@host/database`, which must exist (`createdb`). So
  `DATABASE_URL=postgres://postgres@localhost/starter_pack bin/dev` runs
  the web on PostgreSQL while the apps keep their SQLite files, the
  mix the sync will face.
* The Tauri app: `DATABASE_PATH` when set, else `app.db` in the app data
  directory Tauri gives it, under `~/Library/Application Support` on macOS
  and inside the app's sandbox on a phone. Set `DATABASE_PATH` in dev to
  keep worktrees apart.

Every resource test in `core/` runs through `with_each_database`
(`core/src/database.rs`): against an in-memory SQLite database always,
and against PostgreSQL too when `TEST_POSTGRES_URL` names a database to
use, each test in a schema of its own that is dropped afterwards. That is
what keeps the two databases' modules in step: a function added to one
and not the other fails the tests wherever PostgreSQL is reachable.

```sh
createdb starter_pack_test
TEST_POSTGRES_URL=postgres://postgres@localhost/starter_pack_test cargo test -p app_core --features postgres
```

## Sync

The web server is the system of record. The desktop and mobile apps work
on their own SQLite file, with the network off, and sync with it: the
design, carried over from the Phoenix version, is
`docs/design/offline-sync.md`. In short:

* Every write, on every deployment, is an **event**
  (`core/src/events.rs`): the fields the person changed, from the row's
  version they started at, and who made it. The server applies it at
  once, increments the row's `version` and records the event in
  `app_events`; a device applies it at once too and queues it there for
  the next push.
* A **sync** on a device (`core/src/sync.rs`, `sync::run`) pushes the
  pending events to `POST /api/sync/push` with the device's token, where
  the server applies each as that user with the same function, refusing
  a change to someone else's post, and answers it; then pulls the rows
  that user sees which the server wrote since the device's cursor from
  `POST /api/sync/pull`, each after the rows it points at; then asks
  `POST /api/sync/backfill` for everything in each community the user
  just joined; then prunes the posts the user no longer sees. A row from
  the server never replaces a row with an event still pending.
* The **purge** (`core/src/purge.rs`, `cargo run -p app_web -- purge`,
  for a daily cron job) hard-deletes what was deleted more than 90 days
  ago and records the horizon; a device away longer than that is told to
  start over on its next pull, and does, keeping its pending changes.
* Conflicts are decided by versions, never clocks, field by field. A
  field someone else changed first keeps their value, and the device's
  value is kept as a **conflict** (`core/src/conflicts.rs`) the person
  reapplies or lets go on the `/conflicts` page, which the sync status in
  the nav links to. Deletes are soft, `deleted_at`, so they sync like any
  edit.

The app (`tauri/src/sync.rs`) syncs when it starts, when it comes to the
foreground, after every write, from the Sync now button in the nav, and
every half minute, unless the
person chose to work offline from the nav, a choice kept in the device's
database (`device_settings`, one JSON value per key for what a person
chooses on a device, every one declared with its first value in
`core/src/device_settings.rs` and added to a device when the app starts)
that makes a sync return at once with no request until they go online
again. It syncs against
`SYNC_SERVER_URL`: read
when the app starts, else compiled in at build time, which is how a
phone build learns it, else `http://localhost:4000`. `bin/dev` sets it to
the web server it starts. The web has nothing to sync and shows no sync
status.

The device holds exactly what its user sees. The pull brings each row
after the rows it points at, so the foreign keys hold; joining a
community records it in `sync_backfills` and the sync asks the server
for everything in it; leaving, or a post taken out of a community, is
followed by a prune of the posts no longer visible.

The sync is tested in `core/` with device databases of several users and
a server database in one process, on SQLite and PostgreSQL both, through
the `InProcess` transport, which resolves tokens as the web server does;
and once over HTTP against a running web server, as the dev user:

```sh
SYNC_SERVER_URL=http://localhost:4000 cargo test -p app_core --features sync-http two_devices -- --ignored
```

## Pages

The Svelte app is one document, `svelte/src/index.html`, and a client-side
router (`svelte/src/lib/router.js`, [sv-router](https://sv-router.dev))
shows the page at the path, swapped in place:

| Path | Shows |
| --- | --- |
| `/` | `HomePage.svelte` |
| `/posts` | `posts/PostsIndexPage.svelte` |
| `/posts/new` | `posts/PostsNewPage.svelte` |
| `/posts/:id` | `posts/PostsShowPage.svelte`, with Edit and Delete |
| `/posts/:id/edit` | `posts/PostsEditPage.svelte` |
| `/communities` | `communities/CommunitiesIndexPage.svelte`, to create, join and leave |
| `/conflicts` | `conflicts/ConflictsIndexPage.svelte`, inside the apps |

The New and Edit pages share `posts/PostsForm.svelte`, the body and a
checkbox per community the person is in. A write that worked navigates
to the post, or the list after a delete, through `navigateToUrl` in
`router.js`. The post page shows Edit and Delete to the owner, which it
tells by `usersMe()`.

The paths are real paths, named once in `svelte/src/lib/routes.js`, which
pages link through. Every path is reloadable: the dev server, the web
server and the Tauri app all answer an unknown path with `index.html`.

### Theme and font

The pages are styled with Tailwind v4 and daisyUI, with all of daisyUI's
themes (`svelte/src/lib/theme.css`), and the `hero-*` icon classes come
from the `heroicons` npm package (`svelte/vendor/heroicons.js`) for the
`Icon` component. The theme sets the font, Plus Jakarta Sans, vendored in
`svelte/src/public/fonts` (OFL licence alongside) and served from `/fonts`,
so it works offline on every deployment.

The theme is `data-theme` on `<html>`. `svelte/src/lib/theme_script.js`
sets it before the first paint and switches it on the `app:set-theme`
events the theme menu (`ThemeMenu.svelte`, `ThemeChoices.svelte`, in the
nav) dispatches, keeping the choice in `localStorage` under `app:theme`;
"System" follows the OS light or dark setting. The script isn't bundled:
`vite.config.js` inlines it at the `<!-- theme-script -->` marker in
`index.html`, in dev and in builds. The pages' own styles
(`svelte/src/styles.css`) take their colours from the theme's variables.

### Full bleed

The page runs under the window chrome and keeps its content clear of it.
On the desktop the macOS title bar is an overlay (`titleBarStyle` in
`tauri/tauri.conf.json`): the window controls are drawn over the page, the
page pads its top by the bar's height, and an empty strip there
(`.title-bar-drag` in `index.html`, allowed by the
`core:window:allow-start-dragging` permission) drags the window. On a
phone `viewport-fit=cover` lets the page run under the status bar and the
home indicator, and the page pads by the safe-area insets. `main.js` sets
`data-app` on `<html>` to `desktop`, `mobile` or `web`, which the styles
switch on; a browser gets neither inset.

## Running

Prerequisites: Rust and Node with pnpm (`.tool-versions`), and for the
Tauri app the Tauri CLI (`cargo install tauri-cli --version '^2' --locked`)
and Xcode's command line tools.

```sh
bin/dev                # the web: the server on 4000, the pages with hot reload on 1420
bin/dev desktop        # the desktop app, its pages from the dev server on 1420
bin/dev desktop ios    # both apps side by side, on the one dev server
```

Or to run everything at once:

```
bin/dev web desktop ios "iPhone 16"
```

`bin/dev` starts the Svelte dev server once and every target named loads
its pages from it, so the desktop and the iOS app can run together; two
Tauri targets compile in turn, since Cargo locks the workspace's build
directory. It takes its ports from `PORT` when set (the server on `PORT`,
the pages on `PORT + 1000`), so worktrees don't collide; `bin/dev --help`
has the rest. By hand:

```sh
(cd svelte && pnpm install && pnpm build)   # writes svelte/dist
cargo run -p app_web                        # http://localhost:4000, serving svelte/dist
(cd tauri && cargo tauri dev)               # the desktop app on the pages' dev server
(cd tauri && cargo tauri build)             # the app bundle, under target/release/bundle
```

The web server reads `PORT`, `DATABASE_URL` and `SVELTE_DIST` (default
`svelte/dist`), and logs with `RUST_LOG` (default
`app_web=info,tower_http=info`). The app reads `DATABASE_PATH` and
`SYNC_SERVER_URL`.
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

With `TEST_POSTGRES_URL` set, the core tests run against PostgreSQL too,
see [The database](#the-database).

## Not yet

* TLS to a PostgreSQL server elsewhere: sqlx's `tls-rustls-*` features
* Registering and logging in: today every deployment acts as the dev
  user, see [Who the user is](#who-the-user-is)
* A community's own page, roles, deleting an account: see
  `docs/suggestions/communities.md`
* The `main` branch's desktop app has a quick search popup, a tmux example
  page and the offline-first pages against Phoenix; they can come back
  from there once the pieces they need are here
