# Offline-first realms

The realms page shows the user's realms and, for the chosen one, its
communities and feed, read from a SQLite file on this device. Writing a
post works with the network off. A sync pushes local changes, pulls the
server's, and the two converge. The design is
`phoenix/docs/design/offline-sync.md`; this is what the desktop app does
with it, built 2026-10-25 as phase 6 of
`phoenix/docs/design/build-order.md`.

## Code

```
src/realms.html, src/realms/           the realms page
src-tauri/src/sync/
  mod.rs      the identity store, the commands and the half-minute timer
  schema.rs   opening a realm's file: the embedded client migrations
  store.rs    reads and local writes on a file, applying server rows
  http.rs     the three sync endpoints over HTTP with the API token
  worker.rs   one sync of one realm
src-tauri/build.rs                     embeds phoenix/priv/client_migrations/*.sql
```

The page is a thin view. Every decision is on the Rust side: which rows
to apply, what to push, what to prune.

### On the device

```
<app data dir>/identity.json     the user and their realms, from identity.realms.list
<app data dir>/realms/<slug>.db  one SQLite file per realm
```

`identity.json` is the realm directory: who the user is and which realms
they may enter, each with the host its web UI and sync endpoints live on.
It comes from the `identity.realms.list` request on the websocket API and
is kept so the page opens offline. Logging out deletes it and every realm
file.

A realm file has the same tables as the server's realm database, created
from the SQL files Phoenix exports with `mix app.migrations`, which
`build.rs` embeds so the app builds without Elixir. It adds the two
client-only tables, `sync_state` for the cursor and `sync_outbox` for the
rows to push, see `phoenix/priv/client_migrations/README.md`.

### A local write

`write_post` inserts the post and one `posts_communities` row per chosen
community, with a UUIDv7 from this device and the device's time as
`updated_at`, and queues each row in `sync_outbox`. The page shows the
post at once, marked as waiting to sync, and a sync starts in the
background.

### A sync

`worker::sync_realm` runs after every local write, when a realm is
opened, from the Sync now button, and every half minute:

1. **Push** the outbox. An accepted row gets the server's `last_synced_at`.
   A rejected row is replaced by the server's version, or dropped when the
   server has none; memberships and `posts_communities` rows always come
   back this way, since the server wins on them. A row edited again while
   the push was in flight stays queued.
2. **Pull** every page from the stored cursor, applying each page as
   upserts in one transaction with foreign keys deferred, then store the
   new cursor. A pull a minute apart re-delivers the last minute's rows,
   which the upserts absorb. A device away longer than the server's
   retention window gets `resync` instead of rows: it may have missed a
   purged tombstone, so it drops every synced row and the cursor and
   pulls from nothing.
3. **Backfill** each community the pull showed the user joined, since the
   cursor cannot see rows that became visible without changing.
4. **Prune** every post not by the user that is no longer in a community
   the user is a live member of. Its community rows go with it through
   the foreign key.

The report, or the error, reaches the page as a `realm-synced` event.

### Commands

| Command | Does |
| --- | --- |
| `list_realms` | the realm directory, from the server when connected, else as saved |
| `get_realm_page` | a realm's communities, feed, pending count and cursor, from its file |
| `write_post` | writes a post into the file and starts a sync |
| `edit_post` | edits the user's own post in the file and starts a sync |
| `sync_realm` | syncs the realm now and returns the report |

### Development

Browsers resolve `acme.localhost` to this machine but the system resolver
does not, so `SyncServer` points any `*.localhost` host at the server's
address, see `Config::dev_address`. The server refuses sync requests
unless `API_SOCKET=true`, like the websocket API.

Run the Rust tests with `cargo test` in `src-tauri`. They exercise the
file, the outbox and the pruning against an in-memory database; the
server side is tested in Phoenix with its own simulated client,
`App.SyncClient`.

One test runs two simulated devices against a live server and is ignored
by default. With the Phoenix dev server running with `API_SOCKET=true`,
a user connected to a realm and a member of one of its communities, and
that user's API token:

```sh
SYNC_SERVER_URL=http://localhost:4000 SYNC_REALM_HOST=acme.localhost \
SYNC_TOKEN=... SYNC_USER_ID=... SYNC_COMMUNITY_ID=... \
cargo test two_devices_converge -- --ignored
```

It proves that a post written offline reaches the server and the other
device, and that the same post edited on both devices converges to the
later edit.
