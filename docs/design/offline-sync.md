# Offline sync

How the desktop and mobile apps keep a SQLite file of their own, write to
it with the network off, and converge with the web server, the system of
record. The design is the one `phoenix/docs/design/offline-sync.md` and
`offline-sync-client.md` settled, with the reasons, kept here only as the
decisions; what changes is how much code it takes, since `core/` runs on
the server and on every device.

## Decisions carried over

- **Conflicts are decided by versions, never clocks.** Every synced row
  has a `version` only the server increments. A change carries the
  version it started from.
- **A row changes only through events**, one per save, holding the fields
  the person changed, merged field by field on the server. A field someone
  else changed first keeps their value, and the device's value comes back
  as a conflict the person reapplies or lets go. Nothing is lost silently.
- **Soft deletes.** A delete is an update of `deleted_at`, so it syncs
  like any edit. Every list and get leaves deleted rows out.
- **The pull cursor is the server's write time**, `last_synced_at`, set
  by the server inside the write, pulled with an overlap of a minute and
  ordered by `(last_synced_at, id)`. A row the overlap sends again is a
  harmless upsert.
- **A row from the server never replaces a row with a queued change.**
  The push decides that row first.
- **A conflict is kept until the person resolves it.** Nothing is retried
  on its own.
- Ids are UUIDv7 from whichever device writes the row. Times are UTC to
  the microsecond.

## What is simpler than in Phoenix

- **The rules are written once.** The merge, the fields an event may
  change and a post's validation live in `core/`, and a device's local
  write is the same `posts::create` the server runs. Phoenix's desktop
  client re-implemented every rule in Rust; here there is nothing to
  mirror.
- **One schema.** Every deployment runs the same migrations, so the
  events table and the sync columns are one migration everywhere, and no
  client schema is exported.
- **The web writes the way a device does, for free.** Every write in
  `core/` is an event. The server applies it at once; a device applies it
  at once too, and queues it for the next push. There is never a second,
  privileged write path.
- **Sync is tested without HTTP.** The transport to the server is a trait
  with two implementations: HTTP for the apps, and one that calls the
  server's own functions in the same process for tests. A test holds one
  server database and two device databases and syncs them in whatever
  order it likes, on SQLite and PostgreSQL both.

## Roles

The same code has to know whether it is the system of record or a copy,
so every core function takes a `Deployment`: the `Database` it opened and
a `Role`, `Server` for the web server and `Device` for the apps. Reads are
the same in both. A write differs in two places: the server increments
`version` and records the event in its log; a device leaves the version
as the server's and queues the event. Tests make both roles over
in-memory databases.

## Columns on every synced table

| Column | Set by | Meaning |
| --- | --- | --- |
| `version` | the server only | 1 for a new row, incremented by the server on every write; 0 on a device's row not yet pushed |
| `last_synced_at` | the server only | the time the server last wrote the row, the pull cursor; null on a device's row not yet pushed |
| `deleted_at` | whoever deleted | the soft delete, the time on that device's clock |

`updated_at` is the time of the last edit on the clock of whoever made
it, shown to people and deciding nothing. The server writes its own time
over a row it applies a change to, so a device's optimistic times are
replaced by the server's when its answer comes back. `deleted_by_id`
waits for users.

## Events

An event is one change a person made, sent once and answered once:

```json
{
  "id": "0192f7c0-...",              // from the device, the idempotency key
  "table": "app_posts",
  "row_id": "0192f7a1-...",
  "type": "update",                  // or "create", with base_version 0
  "base_version": 3,                 // the row's version the change started from
  "base": {"body": "old"},           // the changed fields' values as the device saw them
  "changes": {"body": "new"},        // the values the person chose
  "made_at": "2026-11-08T10:00:00.000000Z"   // the device's clock, for ordering and display
}
```

`create` carries the fields the person set. `update` carries the fields
that changed, and `base` holds exactly those fields with the row's values
just before the save, so two saves made one after another on one device
never conflict with each other. A delete is an update with
`changes: {"deleted_at": ...}` and `base: {"deleted_at": null}`.

Which fields an event may change is the resource's rule, in its module:
for a post, `body` on create, `body` and `deleted_at` on update. Any
other field is refused as `not_editable: <field>`.

### The merge, `events::merge`

Given the row's current values at `version`, and the event's
`base_version`, `base` and `changes`:

1. At the server's version, every change applies.
2. Behind it, field by field: a field the server already holds the
   device's value for is skipped; a field whose server value is still the
   device's `base` value applies, since nobody else changed it; any other
   field was changed by someone else first. **Their change stays**, and the
   device's value is a conflict, `{"yours": ..., "server": ...}`.

The fields that applied are written in one update, which increments the
version once on the server. The answer is the event's status, `applied`,
`conflict` or `rejected`, the version after it, the conflicts, a reason
when rejected, and the row as it now stands on the server.

### The log

`app_events` is one table on both sides, created by the same migration,
and its rows never travel as rows, only as a push's events. On the server
it is the log: every event answered is recorded with its answer, in the
same transaction as the row, so a retried event gets the same answer and
nothing applies twice. On a device it is the queue: an event is `pending`
until the server answers it, deleted when applied, and kept as a
`conflict` until the person resolves it. A rejected event the server sent
no row for stays pending with the reason in `last_error`, for a debugging
screen rather than the conflict page. Posts log; a resource that should
not log is a switch for when one arrives.

### A write, in every role

`posts::update(deployment, id, body)` reads the row, builds the event
with `base_version: row.version`, `base: {"body": old}` and
`changes: {"body": new}`, and applies it through `posts::apply_event`,
which is also what the server runs for a pushed event. Apply checks the
fields, merges against the row, puts the applied fields onto the `Post`
through its JSON form so the SQL stays whole-row, validates, writes the
row, and then does the role's extra work: records the event and bumps
the version on the server, queues the event on a device. All of it is one
transaction, which is why a core function runs in one `Transaction` and
the SQL modules take a connection, never the pool.

Applying locally on a device always succeeds without conflict, since the
device's own row is the base. The conflict, if any, comes back from the
server.

## The endpoints

Two, on the web server, under the API, with no auth until users exist:

```
POST /api/sync/push
  {"events": [<event>, ...]}
  -> {"events": [{"id", "status", "version", "conflicts", "reason", "server": {"table", "row"} | null}, ...]}

POST /api/sync/pull
  {"cursor": {"last_synced_at", "id"} | null, "after": {"last_synced_at", "id"} | null, "limit": 200}
  -> {"rows": [{"table", "row"}, ...], "next": {"last_synced_at", "id"} | null}
```

A push applies the events in the order given, each in its own
transaction, and answers every one. A pull answers rows with
`(last_synced_at, id)` after the cursor, the overlap taken off the
cursor's time; `after` continues a paged pull exactly, with no overlap.
`next` is the key of the last row when the page was full. Deleted rows
are sent like any other.

## One sync on a device, `sync::run`

1. **Push** every pending event, in `made_at, id` order. Apply the answers:
   `applied` deletes the event; `conflict` keeps it with the conflicts;
   a rejected `create` deletes the event and the row it made, and the
   server's row, if any, takes its place; another `rejected` with a server
   row deletes the event, the server wins; a `rejected` without one keeps
   the event pending with the reason. Then, for each row the push
   answered, once no event for it is still pending, apply the server's
   row from its last answer, which brings the merged values and the new
   version. An event queued while the push was in flight is still
   pending, so its row is left alone until the next push.
2. **Pull** pages from the stored cursor until `next` is empty, upserting
   each row unless it has a pending event, and storing the key of the last
   row as the cursor.

A push that fails as a whole changes nothing on the device, and the pull
still runs. One sync runs at a time. A sync runs when the app starts,
after every local write, from the Sync now button, and every half minute.

The device keeps its cursor in `sync_state`, one row, which the server has
too and leaves empty, since every deployment has the same tables.

### How a sync is started and run

A sync is a task, not a job in a queue. The app (`tauri/src/sync.rs`)
starts one on four triggers: when the app starts, after every local
write, from the Sync now button, and from a timer every half minute. A
write's command awaits the row going into the device's file, then spawns
the sync on the app's async runtime without awaiting it and returns, so
the page navigates while the sync runs and the sync finishes whenever it
finishes. The server never starts one: there is no push from the server
and no websocket, and another device sees a change on its own next pull.

**Two syncs never overlap.** The task takes a Tokio mutex before running.
A second task that arrives while one is running waits on the lock, then
runs a full sync of its own. Nothing is coalesced or cancelled, and
nothing is lost by that: the only queue is the data, the pending events
in `app_events`, which a sync reads when it runs. Three writes in a row
spawn three syncs; the first pushes all three events and the next two
find nothing to push and only pull.

**The pull follows the push in the same task.** `sync::run` is one async
function: it pushes the pending events, keeps a push failure in the
report, then pulls regardless. A push that fails as a whole changes
nothing on the device. A pull that fails ends the sync with the error.
Either way the task ends with one `synced` event to the pages, carrying
the report or the error, which the nav shows and the posts pages reload
on. There are no callbacks between the steps.

Offline, every trigger still spawns a sync, which fails at the push and
the pull within the request timeout and reports the error; the events
stay pending and the next trigger tries again. A timer only runs while
the app is in front: a phone suspends the app in the background, so
nothing syncs until it comes back.

### Working offline

A person can choose to work offline, and choose to go online again, from
the nav. The choice is a device setting, a row of `device_settings`, the
table of what a person chooses on a device, one JSON value per key so a
new setting needs no migration. Every setting is declared once, with the
value a device starts with, in `core/src/device_settings.rs`, and the app
brings a device up to date with that list when it starts, adding what is
missing and leaving what the person set; that is also where a release
that changes a setting rewrites what an earlier one stored. The choice
holds across restarts, and it is a decision, not a guess: while it is
set, every trigger still runs a sync, but the sync reads the choice first
and returns at once with a report marked offline, making no request.
Writes keep queueing as they always do, the nav shows "Working offline"
with the count waiting, and going online runs a sync at once that pushes
them. The web has no such choice: it is the server.

Real loss of the network is not detected. A sync tries and fails within
the request timeout, the nav shows the error, and the next trigger tries
again. Detecting it would only guess at what the attempt finds out for
certain, and the switch covers the cases where no attempt may be made:
a metered connection, a demonstration, a test of the offline path.

## Conflicts

The nav shows the pending count and "N changes were not saved on the
server", a link to the conflicts page, only inside the apps; the web has
nothing to sync. The page lists each conflicted event, newest first: the
post by its first line, when the change was made, and for each field the
person's value and the value now on the device, which is the server's as
of the last sync. **Reapply** queues a new update from the row's current
version with the person's values and syncs; it applies unless someone
changed the field yet again. **Let it go** deletes the event. A conflict
on a post that is gone can only be let go.

## Build order

Each phase ends in a state that works, is tested and is worth keeping.

| Phase | Builds | Proves |
| --- | --- | --- |
| 1 | the columns, `app_events`, `sync_state`, the role, every write through `events` | the web behaves as before; versions climb on the server and stay on a device; a deleted post is hidden; a retried event gets the same answer |
| 2 | `sync::push` and `sync::pull` in core and on the server, the in-process transport, the tests | two devices converge; the same field edited on both keeps the first and returns the other as a conflict; different fields both apply; two offline edits on one device do not conflict with each other; a pulled row never replaces one with a pending event |
| 3 | the device: queueing, `sync::run`, the HTTP transport, the timer, the status in the nav | a post written offline reaches the web and the other device |
| 4 | the conflicts page, Reapply and Let it go | a conflict reapplied reaches both devices; one let go sends nothing |

Deferred, roughly in the order they will come: users and tokens, then
ownership and `deleted_by_id`; communities, visibility, backfill and
pruning; server-wins rows and the outbox; the purge job and the resync
signal for a device away longer than the purge window; the "something
changed" nudge; a stale-form check on the web, which needs the page to
send the version it loaded; `server_seq` in place of the time cursor if
the overlap proves awkward on PostgreSQL.
