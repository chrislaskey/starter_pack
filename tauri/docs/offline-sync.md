# Offline-first communities

The communities page shows the user's communities and feed, read from a
SQLite file on this device. Writing a
post works with the network off. A sync pushes local changes, pulls the
server's, and the two converge. The design is
`phoenix/docs/design/offline-sync.md`, and what a client must do is
`phoenix/docs/design/offline-sync-client.md`; this is what the desktop app does
with it, built 2026-10-25 as phase 6 of
`phoenix/docs/design/build-order.md`.

## Code

```
src/communities/                       the communities page, at /communities (src/lib/router.js)
src-tauri/src/sync/
  mod.rs      the profile, the commands and the half-minute timer
  schema.rs   opening the file: the embedded client migrations
  store.rs    reads and local writes on the file, applying server rows
  http.rs     the three sync endpoints over HTTP with the API token
  worker.rs   one sync
src-tauri/build.rs                     embeds phoenix/priv/client_migrations/*.sql
```

The page is a thin view. Every decision is on the Rust side: which rows
to apply, what to push, what to prune.

### On the device

```
<app data dir>/profile.json  the user, from auth.user.get
<app data dir>/app.db        the synced tables
```

`profile.json` is who the user is, from the `auth.user.get` request on
the websocket API, kept so the page opens offline. Logging out deletes it
and `app.db`.

`app.db` has the same `app_` tables as the server's database, created
from the SQL files Phoenix exports with `mix app.migrations`, which
`build.rs` embeds so the app builds without Elixir. It adds the two
client-only tables, `sync_state` for the cursor and `sync_outbox` for the
rows to push, plus `sync_backfills` and `sync_events` for the changes to
posts, see `phoenix/priv/client_migrations/README.md`.

### A local write

`write_post` checks the post with the server's own rules first,
`App.Data.Post.changeset/3` as the Rust `mix app.rust` writes next to the
schema and `src-tauri/src/changesets.rs` compiles in by path, so a post the
server would refuse is refused here before it is saved. It then inserts the
post and one `app_posts_communities` row per chosen community, with a
UUIDv7 from this device, the device's time as `updated_at` and version 0,
not yet on the server. Posts are event-sourced: the post is queued as a
`create` event in `sync_events`, and its community rows in `sync_outbox`.
The page shows the post at once, marked as waiting to sync, and a sync
starts in the background.

`edit_post` and `delete_post` change the row at once and queue an `update`
event with the version the device has and the field's value before the
change, so the server can tell a field someone else changed meanwhile.
The device never changes a version itself; the server's comes back with
the row.

### A sync

`worker::sync` runs after every local write, when the page is opened,
from the Sync now button, and every half minute:

1. **Push** the outbox and the pending events. An accepted row gets the
   server's `last_synced_at` and `version`.
   A rejected row is replaced by the server's version when there is one;
   memberships and `app_posts_communities` rows always come back this way,
   since the server wins on them. Without one, the row and its entry
   stay, with the attempt counted and the reason in `last_error`, for the
   user to fix. A row edited again while the push was in flight stays
   queued. An applied event is dropped. An event the server answers with
   conflicts, a field someone else changed first, is kept as a conflict
   for the person. A rejected event is dropped when the server sends its
   row, else kept pending with the reason. Then each answered event's
   server row is applied, unless an event for that row is still pending.
   A push that fails as a whole does not stop the pull.
2. **Pull** every page from the stored cursor, applying each page as
   upserts in one transaction with foreign keys deferred, then store the
   new cursor. A pull a minute apart re-delivers the last minute's rows,
   which the upserts absorb. A device away longer than the server's
   retention window gets `resync` instead of rows: it may have missed a
   purged tombstone, so it drops every synced row and the cursor and
   pulls from nothing.
3. **Backfill** each community the pull showed the user joined, since the
   cursor cannot see rows that became visible without changing. The
   communities to backfill are recorded in `sync_backfills` with the page
   that brought the membership and cleared after the last page, so a
   backfill cut short is finished on the next sync.
4. **Prune** every post not by the user that is no longer in a community
   the user is a live member of. Its community rows go with it through
   the foreign key.

The report, or the error, reaches the page as a `synced` event.

Two rules keep local edits safe. One sync runs at a time, on a lock. And a row pulled from the server, or returned with a
rejection, never replaces a row that has a local change waiting in the
outbox or a pending event: the push decides that row's fate first.

### Conflicts

When the page has conflicts it shows "N changes were not saved on the
server. See details". Each lists the post, the person's value and the
value now on the device, which is the server's as of the last sync, with
**Reapply**, which queues the person's value again as a new event from
the current version and syncs, and **Let it go**, which drops it. A
conflict on a post that is gone can only be let go.

### Commands

| Command | Does |
| --- | --- |
| `get_profile` | the user, from the server when connected, else as saved |
| `get_communities_page` | the user's communities, feed, pending count, conflicts and cursor, from the file |
| `write_post` | writes a post into the file and starts a sync |
| `edit_post` | edits the user's own post in the file and starts a sync |
| `delete_post` | deletes the user's own post in the file and starts a sync |
| `resolve_conflict` | `reapply` queues a conflicted change again and starts a sync, `ignore` lets it go |
| `sync_now` | syncs now and returns the report |

### Development

The sync endpoints are on the server at `PHOENIX_URL`, the same one the
websocket API uses. The server refuses sync requests unless
`API_SOCKET=true`, like the websocket API.

Run the Rust tests with `cargo test` in `src-tauri`. They exercise the
file, the outbox, the events, conflicts and the answers to a push, and
the pruning against an in-memory database; the
server side is tested in Phoenix with its own simulated client,
`App.SyncClient`.

One test runs two simulated devices against a live server and is ignored
by default. With the Phoenix dev server running with `API_SOCKET=true`,
a user who is a member of a community, and that user's API token:

```sh
SYNC_SERVER_URL=http://localhost:4000 \
SYNC_TOKEN=... SYNC_USER_ID=... SYNC_COMMUNITY_ID=... \
cargo test two_devices -- --ignored
```

It proves that a post written offline reaches the server and the other
device, that the same post edited on both devices keeps the edit that
reached the server first and leaves the other as a conflict, whatever
either clock says, and that the conflict reapplied reaches both devices.
