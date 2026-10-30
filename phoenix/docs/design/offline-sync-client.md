# Offline sync on the client: versions, events and conflicts

What a desktop or mobile client does to sync with the server's MVCC
versions and event-sourced resources. The server side and the reasons are
in `offline-sync.md`, "Leak 2" and "Event-sourced resources". The desktop
app does this in `tauri/src-tauri/src/sync`, see
`tauri/docs/offline-sync.md`, and the server tests' simulated client,
`test/support/sync_client.ex`, does it in Elixir.

## The rules in one place

1. **The client never writes a version.** It stores the server's with the
   row. A row created on the device and not yet pushed has version 0.
2. **Plain tables** sync as before, through `sync_outbox`: the whole row
   is pushed, and it carries the version it was read at.
3. **Event-sourced tables**, today `app_posts` only, never push rows. Each
   save is one event, queued in `sync_events`, which the server answers
   once.
4. **A row from the server never replaces a row with a queued change**: an
   outbox entry or a pending event. The push decides that row first.
5. **A conflict is kept until the person resolves it**, by reapplying it
   or letting it go. Nothing is retried automatically.

## Tables on the device

Every synced table has `version`, from the generated client migrations.
The events are in a client-only table, next to `sync_outbox`, created by
the client itself like the other client-only tables:

```sql
CREATE TABLE IF NOT EXISTS sync_events (
  id TEXT PRIMARY KEY,            -- UUIDv7 from the device, the idempotency key
  table_name TEXT NOT NULL,       -- 'app_posts'
  row_id TEXT NOT NULL,
  type TEXT NOT NULL,             -- 'create' | 'update'
  base_version INTEGER NOT NULL,  -- the row's version when the change was made, 0 for a create
  base TEXT NOT NULL,             -- JSON: the changed fields' values before the change
  changes TEXT NOT NULL,          -- JSON: the changed fields' new values
  made_at TEXT NOT NULL,          -- device time, for ordering and display only
  status TEXT NOT NULL,           -- 'pending' | 'conflict'
  conflicts TEXT,                 -- JSON from the server, when status = 'conflict'
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT
);
CREATE INDEX IF NOT EXISTS sync_events_row ON sync_events (table_name, row_id);
```

`made_at` orders events: they must be pushed in the order they were made.
Use the device clock as text in the wire format; with UUIDv7 ids, `id`
order is the same and is a fine tiebreak.

## Local writes

### Plain tables

Unchanged: upsert the row and queue it in `sync_outbox`. Do not touch
`version`. A new row is inserted with `version = 0`.

### Event-sourced tables, in one transaction

**Create.** Insert the row with `version = 0` and queue:

```json
{"type": "create", "base_version": 0, "base": {}, "changes": {"body": "hello"}}
```

`changes` holds only the fields the person set; for a post, `body`. The
server sets the author from the token.

**Update.** Read the row, then queue:

```json
{"type": "update", "base_version": <row.version>,
 "base": {"body": <row.body>}, "changes": {"body": <new body>}}
```

and update the row's fields and `updated_at`. `base` holds exactly the keys
in `changes`, with the row's values just before this save, which for a
second offline edit are the first edit's values. That is what keeps two
saves on one device from conflicting with each other.

**Delete** is an update of `deleted_at`, the device time in the wire format:
`"changes": {"deleted_at": "..."}`, `"base": {"deleted_at": null}`. The
server sets `deleted_by_id`.

Which fields an event may change is the server's rule, today `body` on
create and `body` and `deleted_at` on update. Any other field is refused
as `not_editable: <field>`.

Run the resource's universal changeset before queuing, as `write_post`
does with `changesets::post::changeset`, so an event the server would
refuse as invalid is refused on the device first.

## Push

One request carries both:

```
POST /sync/push
{"rows": [{"table": ..., "row": {...}}],   // the outbox, as before
 "events": [{"id": ..., "table": ..., "row_id": ..., "type": ...,
             "base_version": ..., "base": {...}, "changes": {...}}]}
```

Send every event with `status = 'pending'`, in `made_at, id` order. Leave
out `made_at`, `status`, `conflicts`, `attempts` and `last_error`. The
server applies, in one transaction, parents first, each table's events
before its rows, so a new post's `app_posts_communities` rows can travel
in the same push as its create event.

The response:

```json
{"accepted": [{"table": ..., "id": ..., "last_synced_at": ..., "version": 2}],
 "rejected": [{"table": ..., "id": ..., "reason": ..., "server": {"table": ..., "row": {...}} | null}],
 "events":   [{"id": ..., "table": ..., "row_id": ...,
               "status": "applied" | "conflict" | "rejected",
               "version": 5,
               "conflicts": {"body": {"yours": "...", "server": "..."}},
               "reason": "...",
               "server": {"table": ..., "row": {...}} | null}]}
```

Apply it in this order:

1. **Accepted rows**: set `last_synced_at` and `version` on the row, and
   dequeue only entries queued before the push started.
2. **Rejected rows**: dequeue first, then apply `server` when there is
   one. On `conflict` someone else wrote the row since this device read
   it, and `server` is their row; for a plain table the device's change
   is gone.
3. **Event answers**, each by `id`:
   - `applied`: delete the event.
   - `conflict`: set `status = 'conflict'` and store `conflicts`. The
     fields not in `conflicts` did apply.
   - `rejected` with `server`: delete the event; the server's row wins.
     Reasons: `not_author`, `exists`, `not_editable: ...`.
   - `rejected` without `server`: keep the event pending, count the
     attempt and keep `reason` in `last_error`, as a rejected outbox row
     is kept. Reasons: `malformed`, `bad_datetime: ...`,
     `invalid: ...`, `unknown_post`, `event_id_taken`. These are bugs
     or invalid input, for a debugging screen, not the conflict UI.
4. **Server rows of answered events**: for each row, once no event for it
   is still `pending`, apply the `server` row of its last answer, which
   brings the merged values and the new version. An event queued while
   the push was in flight is still pending, so the row is left alone
   until the next push answers that one too.

A push that fails as a whole changes nothing on the device; every event is
still pending and is sent again. That is safe: the server answers an event
it already applied with the recorded answer, and applies nothing twice.

The pending count the page shows is outbox entries plus pending events.

## Pull

Rule 4 applies: a row with an outbox entry or a pending event is skipped. A
row with only conflicted events is updated by pulls as normal, so a
conflict always shows the server's current value.

On `resync`, keep `sync_events`, both pending and conflicted: pending
events carry their own base and are pushed before any pull, and conflicts
belong to the person.

## Conflicts

A conflicted event is a save whose listed fields did not reach the server,
because someone else's change to the same field got there first. The rest
of that save did apply.

### Showing them

Wherever the resource is listed, the communities page today:

> **2 changes were not saved on the server.** See details

The count is the number of `sync_events` rows with `status = 'conflict'`.
Hidden at zero.

"See details" lists each conflicted event, newest first:

- what it was about: the post, by its first line;
- when the change was made, from `made_at`;
- for each field in `conflicts`: **Your change**, `yours`, and **Now on
  the server**, the row's current value on the device. Use the row's
  current value, not the stored `server` value, since a later pull may
  have brought something newer;
- two actions: **Reapply** and **Let it go**.

### Resolving them

- **Let it go**: delete the event. Nothing is sent.
- **Reapply**: in one transaction, queue a new `update` event from the
  row's current version, with `base` the row's current values of the
  conflicted fields and `changes` the `yours` values; update the row; delete
  the conflicted event. Then sync. The new event applies unless someone
  changed the same field again in the meantime, in which case it comes
  back as a new conflict with the newer value.

Resolving is per event, all its conflicted fields together. A post has
one editable text field today, so per field adds nothing yet; with more
fields, offer per-field reapply by putting only the chosen fields in the
new event.

A conflicted event of a row that is gone, deleted on the server and
pruned, can only be let go; show it with the text from `yours`.

## On the desktop app

| Piece | Where |
| --- | --- |
| `sync_events` table | `schema.rs`, `CLIENT_TABLES` |
| `EVENT_SOURCED` | `store.rs` |
| `write_post`, `edit_post`, `delete_post` | `store.rs` |
| `pending_events`, `conflicts`, `resolve_conflict` | `store.rs` |
| events in the push request and response | `http.rs` |
| the push answer, steps 1 to 4 above | `worker.rs`, `apply_push_response` |
| `delete_post` and `resolve_conflict` commands, conflicts on the page | `mod.rs` |
| banner, details and actions | `tauri/src/communities/CommunitiesPage.svelte` |

## Tests

The server's are in `test/app/data/sync/sync_test.exs`, against the
simulated client; the desktop's in `store.rs` and `worker.rs`, against an
in-memory file, plus the live two-device test in `worker.rs`. Both cover:

- a post is created by an event, then edited from the version the server
  has: both apply;
- the same field edited on two devices: the first to reach the server
  stays, the other becomes a conflict, and the device shows the server's
  value with the conflict kept;
- a conflict reapplied applies; one let go sends nothing;
- different fields edited on two devices both apply;
- two edits made one after another offline, pushed together, do not
  conflict with each other;
- a retried event gets the same answer and is not applied twice;
- a pulled row never replaces a row with a pending event;
- an event queued during a push stays pending, and its row is not
  replaced by the answer to the earlier event.
