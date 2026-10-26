# Offline-first writing and syncing

How mobile and desktop clients write locally first and reconcile with the
server's database. Companion to `multi-tenancy.md`, which settles that a
tenant needing isolation is its own deployment, with one database on the
server and one file per server on the device.

Decisions already made that this doc builds on:

- Resources have one clear primary owner, so **last write wins (LWW)** is an
  acceptable conflict policy.
- **Soft deletes** everywhere, with `deleted_at` and `deleted_by_id`, and a
  separate job that purges rows deleted more than **90 days** ago. A client
  away longer than that does a full resync.
- **One replication path.** No whole-database mode for private deployments.
  If every member is in every community, the normal visibility rule already
  sends everything.
- Clients generate ids, so primary keys are **UUIDv7**.
- Sync is **per deployment**. A sync session talks to one server.
- **Ecto migrations are the schema source of truth.** Client SQLite
  migrations are generated from them. See "Schema source of truth."
- **Datetimes are UTC ISO 8601 text with microsecond precision**, exactly as
  the SQLite adapter writes them. See "Datetime format."
- **Sync runs over HTTP** on the server with the existing API token as a
  bearer token. The websocket channel later carries only a "something
  changed" nudge. See "Transport."

## The baseline design

What has worked before, stated precisely so the weak points are visible:

1. Every row in every synced table carries a server-assigned timestamp
   recording when the server last wrote it.
2. Each client keeps one cursor per server: the server timestamp of its last
   successful pull.
3. **Pull.** Client sends its cursor. Server returns every row, across all
   tables, with a server timestamp greater than the cursor. Client upserts
   them. Rows with `deleted_at` set are applied as deletes, or kept and
   filtered, depending on whether the client wants undo.
4. **Push.** Client sends every row it changed locally since its last push.
   Server upserts each one, keeping the row whose `updated_at` is later.
5. Deletes are updates that set `deleted_at`, so they flow through the same
   path. A purge job hard-deletes rows past the retention window.

This is a good design. It is small, it needs no extra tables, it is easy to
debug by reading rows, and LWW makes push idempotent. The rest of this doc is
about the four places where it leaks and the cheap fixes for each.

## Columns on every synced table

The same columns exist on the server and on every client, so one table
definition serves both and the sync code is the same shape everywhere.

| Column | Who sets it | Meaning |
| --- | --- | --- |
| `updated_at` | the client that made the edit | The time the edit was made, from that client's clock, guarded by the server. Decides last write wins. |
| `last_synced_at` | the server only | The time the server last wrote the row, from the server's clock. The pull cursor. |
| `deleted_at` | the client that deleted | Soft delete. Same clock and guard as `updated_at`. |
| `deleted_by_id` | the client that deleted | Who deleted it. |

The web app counts as a client whose clock happens to be the server's clock.
A LiveView edit writes `updated_at` and `last_synced_at` with the same value.

On a client, `last_synced_at` is copied verbatim from the server and never
written by the client. After a local edit it is simply stale until the next
push response brings the new value. Which rows need pushing is tracked in a
separate outbox table, described under "Push, in detail," so the column means
the same thing on both sides and can be declared not null on the server.

On the server, `last_synced_at` is set inside the write transaction from the
database clock, and any value a client sends for it is ignored.

`server_seq`, described under leak 1, is an optional integer alternative to
`last_synced_at` as the cursor. It is not needed to start.

## Where the baseline leaks

### Leak 1: a timestamp is not a safe cursor

A row's server timestamp is assigned when the write happens, not when it
commits. With concurrent writers, a transaction can start at T1, a client can
pull with cursor T2, and the first transaction can commit at T3 with a
timestamp of T1. The client advances its cursor to T2 and never sees that row.

On SQLite this cannot happen today because there is one writer at a time. It
can happen the day the server moves to Postgres, and it is the kind of
bug that shows up as "one user is missing one post" months later.

Two workable fixes:

- **Timestamp plus buffer.** Keep the server timestamp, assigned by the
  database clock and never by an app server, and pull from
  `cursor - buffer` where the buffer is at least the longest transaction
  timeout, typically 60 seconds. Upserts are idempotent so re-applying rows
  is harmless. Pagination must order by `(synced_at, id)` and carry both in
  the cursor, because many rows can share one timestamp and a page boundary
  inside a tie would skip rows.
- **Integer sequence.** A single counter row in the database. Every write
  to a synced row bumps the counter in the same transaction and stores the
  new value in the row's `server_seq`:

  ```sql
  UPDATE sync_counter SET value = value + 1 RETURNING value;  -- 90417
  UPDATE app_posts SET ..., server_seq = 90417 WHERE id = ?;
  ```

  Pull is `where server_seq > ? order by server_seq limit ?` and the next
  cursor is the last row's value. No clock, no ties, exact paging. On SQLite
  the single writer makes it exact with no buffer. On Postgres a plain
  sequence has the same commit-order gap as a timestamp, so either keep the
  buffer or make the counter a locked row, which serializes writers.

  The counter grows once per server write, not per client or per sync, and
  it is per deployment. As a 64-bit integer it holds about 9.2 quintillion
  values, so one million writes per second would take roughly 290,000 years
  to exhaust it. SQLite integers are always 64-bit. Postgres needs the
  column declared `bigint`, which in an Ecto migration means `:bigint`, not
  `:integer`, since `:integer` maps to a 32-bit column there and would
  overflow at 2.1 billion.

Both work. The sequence is tidier to paginate and immune to server clock
steps; the timestamp is readable, already familiar, and is the same
`last_synced_at` column the clients carry. Either way keep the overlap.

Recommendation: `last_synced_at` plus buffer as the cursor, ordered by
`(last_synced_at, id)`. Add `server_seq` only if the server moves to
Postgres and the buffer proves awkward. The rest of this doc says
`last_synced_at` for the cursor; `server_seq` substitutes everywhere.

### Leak 2: whose clock decides LWW

The cursor column answers "what changed since my last pull." It is assigned
by the server, so it is consistent. It cannot also answer "which of two edits
to the same row wins," because it only records when each edit arrived, not
when it was made. Those are two columns with two jobs: `last_synced_at` for
the cursor and `updated_at` for LWW. There are three ways to fill `updated_at`.

In every option the winner rule is the same: when two versions of a row meet,
the one with the later `updated_at` wins. The options differ only in what
goes into `updated_at`.

- **The time the server received the row.** The last push to reach the
  server wins. One column serves both jobs, which is simplest. Worked
  example: a phone edits a post offline on Monday, a laptop edits the same
  post online on Wednesday, the phone syncs on Friday. The phone's row is
  received on Friday, Friday is later than Wednesday, so the Monday edit
  overwrites the Wednesday edit.
- **The time the client made the edit.** The client sends the time from its
  own clock when the edit happened. Same example: the phone's row carries
  Monday, the laptop's row carries Wednesday, Wednesday is later, so the
  Wednesday edit stays and the phone receives it back on Friday. This is the
  behavior a person with two devices expects.

  Device clocks are unreliable, so the server guards the value before
  storing it. If the client's time is later than the server's current time,
  the server stores its own current time instead. If the client's time is
  absurdly old, the server rejects the row. This guard does not pick the
  winner; it only stops a device whose clock reads 2031 from winning every
  conflict forever. A device whose clock runs slow can still lose to an
  edit that was really made earlier, so this is a best effort, not a
  guarantee.
- **Hybrid logical clock (HLC).** A 64-bit value combining wall time and a
  counter, advanced on every local write and on every message received. It
  orders edits causally across devices, survives bad clocks, and is one
  integer column. This is what the CRDT-based engines use internally.

Recommendation: the time the client made the edit, guarded by the server as
above. With a clear primary owner the common conflict is the same person on
two devices, and they expect their latest edit to stick. If arrival order is
acceptable, drop `updated_at` from the protocol and use the server receive
time for both jobs. Either way keep the column a plain timestamp so switching
to an HLC later is a column change, not a redesign.

LWW is row-level in the baseline. Two devices editing different fields of the
same row means one edit is lost. Column-level LWW stores a per-field clock,
usually as a JSON map, and merges field by field. It is a meaningful upgrade
for forms with many independent fields and not worth it for a post with a
title and a body. Decide per resource type, default to row-level.

### Leak 3: soft deletes and the purge horizon

Soft deletes are the right call. Deletes sync as updates, undo is possible,
and `deleted_by_id` gives an audit trail for free. Three consequences to
handle:

- **Every query filters `deleted_at is null`.** Put it in the data context so
  callers cannot forget. The server already routes everything through
  `App.Data.Context`, which is the place.
- **Unique constraints collide with deleted rows.** A deleted slug blocks the
  same slug being reused. Use partial unique indexes,
  `where deleted_at is null`, which SQLite and Postgres both support.
- **A client offline longer than the retention window misses deletes.** The
  purge removed the tombstone before the client pulled it. The server must
  record its purge horizon, the oldest `last_synced_at` still guaranteed to be
  intact. A client whose cursor is older than the horizon gets told to drop
  its local file and pull fresh. This is also the recovery path for any
  corruption, so it is worth building early.

Purge has to respect foreign keys: a deleted post's community rows and
comments go with it. Purging in dependency order, oldest first, in small batches, is
enough.

### Leak 4: visibility is not a row update

The pull in the baseline sends rows that changed. In a public deployment a
device must not pull the whole database, only the rows its user can see: authored by
them, or in a community they belong to. Visibility changes without the row
changing:

- User joins a community. Thousands of old rows become visible, none of them
  with a recent `last_synced_at`.
- User leaves a community, or is removed. Rows should disappear from the
  device.
- A post is removed from community C. Members of C who cannot see it another
  way should lose it.

A worked example. Alice's phone has pulled everything visible to her up to
seq 90,000. On Tuesday she joins the Gardening community, which has had
thousands of posts added to it over two years. Her next pull asks for
visible rows above 90,000. Those posts have seqs in the low thousands, so
none arrive. The cursor is blind to her visibility expanding. The same
blindness appears at the row level: an author adds an existing post to
Gardening, the new `app_posts_communities` row arrives because it is new, but the
post it points at is old and does not.

The fix has three parts, all driven by the client so the server never has to
remember what any device holds:

- **Visibility is rows, and those rows sync.** `app_users_communities` and
  `app_posts_communities` carry the same `last_synced_at` and soft-delete columns as
  everything else. Removing a post from a community is a soft delete of its
  `app_posts_communities` row. Leaving is a soft delete of an `app_users_communities`
  row. Both arrive in the delta like any other change.
- **An `app_posts_communities` row carries its subtree.** Whenever a pull returns
  one, the server also includes the post it points at and the rows that hang
  off it, comments and reactions, regardless of their `last_synced_at`.
  Because comments reference the `app_posts_communities` row rather than the
  post, the subtree is well defined. Re-sending a post the client already
  has is a harmless upsert. This handles "existing post added to my
  community" with no extra state.
- **Joining is the client asking for every post in that community.** When
  the client applies a new `app_users_communities` row for its own user, it calls
  a backfill endpoint for that community. The endpoint returns every live
  `app_posts_communities` row for the community with the same carrying rule,
  paged. The delta pull and the backfill use one server function: "these
  `app_posts_communities` rows, plus what they point at."
- **Leaving and removal are one local cleanup.** After applying any batch,
  the client deletes every local post it did not author that no longer has a
  live `app_posts_communities` row for a community it belongs to. Comments and
  reactions go with their `app_posts_communities` row through the client's own
  `on delete cascade` foreign keys. One reachability pass covers a removed
  membership, a post removed from a community, and a deleted community. A
  removed membership also arrives with the member's own `app_posts_communities`
  rows already soft-deleted by the server, since leaving cascades through the
  membership on the server side. A revive, such as rejoining within the
  retention window, is the reverse: the membership and `app_posts_communities`
  rows arrive with `deleted_at` cleared, and the carrying rule brings back
  the posts and comments the client had pruned.

The pull endpoint therefore answers "rows visible to user U with
`last_synced_at` later than C, plus the subtree of every `app_posts_communities`
row in that set,"
not "rows changed since C." That query is the heart of the server side and
deserves its own tests.

## Push, in detail

The baseline sends dirty rows. Details that matter:

- **Tracking what to push.** A client-side outbox table of pointers:

  ```sql
  sync_outbox  (table_name, row_id, queued_at, attempts, last_error)
               unique (table_name, row_id)
  ```

  Every local write upserts one entry for the row, refreshing `queued_at`.
  A push reads the outbox, loads each row's current state, and sends it.
  On success it deletes entries whose `queued_at` is earlier than the time
  the push started, so a row edited while the push was in flight stays
  queued for the next one. A rejected row keeps its entry, with the attempt
  count and the server's reason, which is also what a pending-changes count
  or a debugging screen reads from.

  This is not the append-only change log in alternative B. It stores no
  payload and no history, because with last write wins only the row's
  current state matters. One entry per row, three small columns.
- **Dependency order.** Parents before children: communities before posts,
  posts before `app_posts_communities` rows and comments. Either push in table order or have the
  server apply the whole batch in one transaction with deferred foreign key
  checks. One transaction per batch is the better default: a batch is
  accepted or rejected as a unit.
- **Authorization on every row.** The server validates each pushed row
  against the user's memberships and ownership. A client cannot add a post
  to a community it is not in, or edit a post it does not own. Rejected rows
  come back in the response with the server's authoritative version, and the
  client overwrites its local copy. This means **permission rows are
  server-wins, not LWW**: `app_posts_communities` rows, memberships, and roles are never accepted
  on timestamp alone.
- **Idempotency.** Upsert by primary key. A retried push after a network
  failure is harmless.
- **Attachments.** Binary data does not go through row sync. The client
  uploads the file to object storage first, gets a key, and the row carries
  the key. A row referencing an unuploaded file is not pushed.

## Client bookkeeping

In the file on the device:

```sql
sync_state   (last_pulled_at, last_pulled_id, server_purge_horizon)
sync_outbox  (table_name, row_id, queued_at, attempts, last_error)
```

`last_pulled_at` and `last_pulled_id` together are the cursor: the
`last_synced_at` and `id` of the last row received. `sync_outbox` is the set
of rows with local changes not yet accepted by the server. Every synced
table carries the same four columns on both sides, listed under "Columns on
every synced table" above, and nothing device-only. The profile on the
device, the user's id and email, is a cached copy with no sync cursor.

Realtime is optional. Polling on an interval or on app foreground is enough
to start. A Phoenix channel can later send a "something changed"
nudge that triggers a pull, which is cheaper and simpler than pushing the
rows themselves over the channel.

## Schema source of truth

Ecto migrations define every table once. They are adapter-neutral, so the
same migration runs on SQLite today and on Postgres if the server moves,
with no second file. Clients are SQLite forever, so they need SQLite
migrations and nothing else.

A mix task in the style of `mix app.svelte` produces them. For each
migration it runs the migration against a scratch SQLite database with
`log_migrations_sql` on, captures the statements the adapter executed, and
writes them to `priv/client_migrations/<version>_<name>.sql`. Clients apply
the files in version order and record which they have applied, exactly as
Ecto does with `schema_migrations`. A test fails when a migration has no
up-to-date file, and the generated files are committed.

Three rules keep the export clean:

- **Only synced tables are exported.** A schema is marked as synced, the
  way a component is marked `@universal_component true`, and the task includes only
  migrations that touch marked tables. Server-only tables such as
  `users`, `users_tokens`, `app_purges` and `sync_counter` never reach a
  device.
- **Client-only tables are hand-written on the client.** `sync_state` and
  `sync_outbox` are not Ecto schemas. Each client platform owns one small
  migration for them.
- **Column types are whatever the SQLite adapter emits.** Binary ids are
  `TEXT`, datetimes are `TEXT`, booleans are `INTEGER`. The clients do not
  get to choose, which is the point.

The alternative, hand-written raw SQL migrations consumed by both Ecto and
the clients, would need one file per dialect once Postgres arrives, with the
Postgres file untested until then, and would give up the adapter neutrality
Ecto already provides. Rejected.

## Datetime format

Last write wins compares `updated_at` values written by different platforms,
so the format has to be identical everywhere. Phoenix leads: the SQLite
adapter writes UTC datetimes as ISO 8601 text, and that text is the format.

```
2026-10-25T14:03:07.412906Z
```

The trap is precision. Ecto's `:utc_datetime` writes no fractional seconds
while `:utc_datetime_usec` writes six digits, and the two sort wrongly as
strings: `...07Z` is greater than `...07.412906Z` because `Z` sorts after
`.`. So:

- Every sync column is `:utc_datetime_usec`, no exceptions. The exported
  client SQL inherits the format.
- The push endpoint parses every incoming datetime and rejects a row whose
  values are not UTC with exactly six fractional digits. The server is the
  gatekeeper, and a client with a wrong format finds out on its first push
  rather than losing a conflict silently later.
- Clients store server-provided values verbatim and write their own in the
  same shape, so every comparison on a device is between values in one
  format.

Integer microseconds since the epoch would sidestep the string-sorting
problem entirely at the cost of a custom Ecto type and unreadable values in
a SQL shell. Not needed while the rules above hold.

## Transport

Push, pull, and backfill are three HTTP endpoints on the server:

```
POST https://<host>/sync/push
POST https://<host>/sync/pull
POST https://<host>/sync/backfill/<community id>
```

The existing API token, the one the websocket connects with, goes in an
`Authorization: Bearer` header. A plug resolves the user from the token
before anything else runs. Bodies are JSON, compressed, with the cursor in the body and a page
size the server caps.

HTTP rather than the websocket because sync has two needs the channel serves
badly. Backfills are large and want compression, resumable pages, and
testing with `curl`. Background sync on mobile only works over HTTP, since
iOS and Android run background transfers for URL sessions and not for open
sockets. The channel keeps its current request-response job and later gains
a single message, "something changed," that tells a connected client to
pull. Until then, clients pull on foreground and on a timer.

## Alternatives

### A. Baseline with the fixes above

Server-assigned sequence cursor, LWW on a server-bounded client edit time,
inline soft deletes with a purge horizon, visibility as synced rows with
client-driven backfill and pruning.

Pros: smallest surface area. Plain tables, plain upserts, readable by a
human in a SQL shell. Works with SQLite or Postgres on the server. No
dependency on an engine's roadmap. Fits Ecto and `App.Data.Context` without
contortion.

Cons: you own every edge case. Visibility backfill and the purge horizon are
easy to get subtly wrong. Row-level LWW loses concurrent field edits. No
history.

### B. Append-only change log

Instead of marking each row, the server appends every write to a `changes`
table (table, row id, operation, payload, seq). Clients pull the log and
apply it. Rows themselves carry nothing sync-specific.

Pros: full history and audit for free. Cursor semantics are exact because
the log is the thing being paginated. Replay can rebuild any client or any
derived table. Easier to add new merge policies later.

Cons: a second copy of every write, with its own retention policy. Visibility
filtering now applies to log entries, which is harder than filtering rows.
Compaction is needed or the log grows forever. More to build than A, and the
history is often never used.

Worth choosing only if history or undo across devices is a product
requirement.

### C. CRDT-backed tables

Libraries such as cr-sqlite add column-level clocks and merge rules to
ordinary SQLite tables on both ends. Two databases merge in either direction
with no conflicts by construction.

Pros: column-level merge, no lost edits, peer-to-peer capable, device clocks
do not matter. Partial sync is a solved problem within the library.

Cons: per-column metadata roughly doubles storage. The merge model is foreign
to Ecto and to a relational mindset, and debugging a bad merge means
understanding the clock vectors. Foreign-key-like invariants are not what
CRDTs are good at. With a single clear owner per resource the conflict
problem they solve mostly does not arise.

### D. Sync engines

Hosted or self-hosted engines that watch the database and replicate subsets
to clients. The relevant ones at time of writing: ElectricSQL and its Phoenix
integration `phoenix_sync`, PowerSync, and Rocicorp's Replicache and Zero.
Writes generally still go through your own API; the engine handles the read
path and partial replication rules. Verify current state before choosing;
this category changes quickly, and at least one well-known product in it was
discontinued in 2024.

Pros: partial replication, backfill, and cursor correctness are the engine's
problem. Realtime included. Often a client SDK for each platform.

Cons: nearly all of them replicate from Postgres, not SQLite, which forces
the server to Postgres. One more moving part per deployment, and the
deployment-per-tenant model multiplies it. Their partial replication
rules have to express "visible through membership and presence," which is
possible but is the first thing to prototype. Vendor and license risk.

### Comparison

| | A. Baseline+ | B. Change log | C. CRDT | D. Engine |
| --- | --- | --- | --- | --- |
| Server store | SQLite or Postgres | either | SQLite | Postgres |
| Conflict model | row LWW | pluggable | column merge | usually LWW via API |
| Lost edits | concurrent field edits | per policy | none | per policy |
| History | no | yes | no | no |
| Partial sync | hand-built | hand-built, harder | library | built in |
| Fits Ecto and Data.Context | yes | yes | awkward | partly |
| Ops per deployment | none extra | log retention | none | engine instance |
| Debuggability | high | high | low | medium |

## Recommendation

Alternative A. The baseline design with four refinements:

1. `last_synced_at` as the cursor, set only by the server inside the write
   transaction, pulled with an overlap buffer and ordered by
   `(last_synced_at, id)`. Clients copy it from the server and never write
   it. A client-side outbox of row pointers tracks what needs pushing.
2. LWW on a client-supplied `updated_at`, bounded by the server's clock.
   Permission rows are server-wins.
3. Inline soft deletes with partial unique indexes, a purge job, and a
   recorded purge horizon that forces a full resync for stale clients.
4. Visibility as synced rows. `app_users_communities` and `app_posts_communities`
   sync like any table. An `app_posts_communities` row carries its subtree, joining triggers a
   client-driven backfill, and the client prunes unreachable rows after every
   batch.

Revisit if any of these become true: history or undo across devices is
required (B), many people edit the same rows concurrently (C), or the server
moves to Postgres anyway and a sync engine's partial replication can
express the visibility rule (D).

## Guiding questions

Still open. Answered ones have moved to the decisions at the top.

- **Which resource types have many independently edited fields?** Those are
  candidates for column-level LWW. None expected at the start.
- **Is undo or edit history a product feature?** If yes, alternative B earns
  its cost. Not at the start.
- **How big is a user's visible subset of a public deployment?** Determines
  whether initial sync is seconds or minutes, and whether backfill needs
  paging and progress UI.
- **When does the "something changed" nudge arrive?** Polling on foreground
  and a timer is the start. A chat-like feature is what forces the channel
  message.
- **When does the server move to Postgres?** Expected before any app's
  public deployment has enough writers to feel SQLite's single writer. When
  it happens, the cursor's overlap buffer becomes mandatory rather than
  free.
