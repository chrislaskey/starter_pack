# Offline-first communities

The main window's data pages mirror the Phoenix app's: communities, a
community with its comments, posts with their links and tags, tags, and
the profile, each read from a SQLite file on this device. Every write on
them works with the network off. A sync pushes local changes, pulls the
server's, and the two converge. The design is
`phoenix/docs/design/offline-sync.md`, and what a client must do is
`phoenix/docs/design/offline-sync-client.md`; this is what the desktop app does
with it, built 2026-10-25 as phase 6 of
`phoenix/docs/design/build-order.md`.

## Code

```
src/communities/CommunitiesPage.svelte  /communities, App.Web.CommunitiesLive
src/communities/CommunityPage.svelte    /communities/:id, App.Web.CommunityLive
src/posts/PostsPage.svelte              /posts, App.Web.PostsLive
src/tags/TagsPage.svelte                /tags, App.Web.TagsLive
src/profile/ProfilePage.svelte          /profile, App.Web.ProfileLive
src/lib/SyncBar.svelte                  on every data page: Sync now, pending changes, conflicts
src/lib/write.js                        a page's write: the command, then the page read again
src-tauri/src/sync/
  mod.rs          the profile, a sync and the half-minute timer
  commands.rs     the commands the pages call
  pages.rs        what each page shows, read from the file
  schema.rs       opening the file: the embedded client migrations
  store.rs        the file's rows, outbox and events; posts and the profile
  posting.rs      a post's links and its place in a community
  tagging.rs      tags, and tags on posts
  commenting.rs   comments
  memberships.rs  leaving a community
  deletes.rs      a delete copied onto the rows below it
  http.rs         the three sync endpoints over HTTP with the API token
  worker.rs       one sync
src-tauri/build.rs                     embeds phoenix/priv/client_migrations/*.sql
```

The pages are thin views over the same markup and Tailwind classes as the
web's HEEx, with Phoenix's generated components (`$phoenix/...`). Every
decision is on the Rust side: who may change what, which rows to apply,
what to push, what to prune. A page reads itself with one command, a write
returns nothing, and the page reads itself again; the sync bar reads it
again after every sync. Each page shows what is waiting to sync faded.

### On the device

```
<app data dir>/profile.json  the user, from auth.user.get
<app data dir>/app.db        the synced tables
```

`profile.json` is who the user is, from the `auth.user.get` request on
the websocket API, kept so the pages open offline. Logging out deletes it
and `app.db`.

`app.db` has the same `app_` tables as the server's database, created
from the SQL files Phoenix exports with `mix app.migrations`, which
`build.rs` embeds so the app builds without Elixir. It adds the two
client-only tables, `sync_state` for the cursor and `sync_outbox` for the
rows to push, plus `sync_backfills` and `sync_events` for the changes to
posts, see `phoenix/priv/client_migrations/README.md`.

### A local write

The post form is `App.Web.PostsLive.post_form/0`'s definition, as the
`post_form.dynamic_form.json` that `mix app.dynamic_forms` writes beside
it, validated on every change by `show_post_form` and on submit by
`write_post` with the `dynamic_form` crate, see `phoenix/docs/design/dynamic-forms.md`. Once the
form is valid, `write_post` checks the post with the server's own rules,
`App.Data.Post.changeset/3` as the Rust `mix app.rust` writes next to the
schema and `src-tauri/src/changesets.rs` compiles in by path, so a post the
server would refuse is refused here before it is saved. It then inserts the
post and one `app_posts_communities` row per chosen community, with a
UUIDv7 from this device, the device's time as `updated_at` and version 0,
not yet on the server. The post is queued as a `create` event in
`sync_events`, and its community rows in `sync_outbox`.
The page shows the post at once, marked as waiting to sync, and a sync
starts in the background.

Every other write follows the same pattern as the server's flow of the
same name, checked first with the server's own changeset where it has one
(`src-tauri/src/changesets.rs`):

| Write | Server flow | Queued as |
| --- | --- | --- |
| edit, delete a post | `App.Data.Posting` | `update` event on `app_posts` |
| add, remove a link | `Posting.add_link/3`, `remove_link/2` | `create`, `update` event on `app_links` |
| remove a post from a community | `Posting.remove_from_community/3` | `app_posts_communities` row in the outbox, server-wins |
| make, rename, delete a tag | `App.Data.Tagging` | `create`, `update` event on `app_tags` |
| tag a post | `Tagging.tag_post/3` | the tag's `create` when it is new, then a `create` of the `app_posts_tags` row, or an `update` reviving the row the post had |
| take a tag off a post | `Tagging.untag/2` | `update` event on `app_posts_tags` |
| comment, delete a comment | `App.Data.Commenting` | `create`, `update` event on `app_comments` |
| leave a community | `App.Data.Memberships.leave/2` | `app_users_communities` row in the outbox, server-wins |
| edit the profile | `App.Data.Profiles` | `update` event of the fields that changed |

An `update` carries the version the device has and the changed fields'
values before the change, so the server can tell a field someone else
changed meanwhile. The device never changes a version itself; the
server's comes back with the row. The profile is the user's own,
created by the server at registration.

A delete is copied onto the rows below it on the device at once, as
`App.Data.Deletes` does on the server, so the pages hide them: a post's
links, tags and community rows, a tag's rows on posts, a membership's
post rows, and a community row's comments. The copies are not queued;
the server makes its own and they arrive on the next pull. Leaving a
community also prunes at once what the user can no longer see.

Creating and joining communities stay on the web for now, see
`docs/suggestions/offline-pages.md`.

### A sync

`worker::sync` runs after every local write, when a data page is opened,
from the Sync now button, and every half minute:

1. **Push** the outbox and the pending events. An accepted row gets the
   server's `last_synced_at` and `version`.
   A rejected row is replaced by the server's version when there is one;
   memberships and `app_posts_communities` rows always come back this way,
   since the server wins on them, and one the server answers `server_wins`
   without a row is dropped, since the server has none. Without one, the
   row and its entry stay, with the attempt counted and the reason in
   `last_error`, for the user to fix. A row edited again while the push was in flight stays
   queued. An applied event is dropped. An event the server answers with
   conflicts, a field someone else changed first, is kept as a conflict
   for the person. A rejected `create` drops the row it made, its other
   events and the rows below it, and the server's row, if any, takes its
   place: a tag another device made first, or a tag row the post already
   has. Another rejected event is dropped when the server sends its row,
   else kept pending with the reason. Then each answered event's
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
   the user is a live member of. Its community rows, links, tag rows and
   comments go with it through the foreign keys, and so does someone
   else's tag left on none of the posts here.

The report, or the error, reaches the pages as a `synced` event.

Two rules keep local edits safe. One sync runs at a time, on a lock. And a row pulled from the server, or returned with a
rejection, never replaces a row that has a local change waiting in the
outbox or a pending event: the push decides that row's fate first.

### Conflicts

When there are conflicts the sync bar shows "N changes were not saved
on the server. See details" on every data page. Each lists what it was
on, such as the post or the tag, the person's value and the
value now on the device, which is the server's as of the last sync, with
**Reapply**, which queues the person's value again as a new event from
the current version and syncs, and **Let it go**, which drops it. A
conflict on a post that is gone can only be let go.

### Commands

| Command | Does |
| --- | --- |
| `get_profile` | the user, from the server when connected, else as saved |
| `get_sync_status` | the pending count, conflicts and cursor, for the sync bar |
| `sync_now` | syncs now and returns the report |
| `resolve_conflict` | `reapply` queues a conflicted change again and starts a sync, `ignore` lets it go |
| `get_communities_page`, `get_community_page`, `get_posts_page`, `get_tags_page`, `get_profile_page` | a page, from the file |
| `show_post_form`, `change_post_form` | the post form as it first shows, and after a change, with its errors once a submit failed |
| `write_post` | submits the post form: writes the post and starts a sync, or returns the form with its errors |
| `edit_post`, `delete_post`, `add_link`, `remove_link`, `tag_post`, `untag_post`, `create_tag`, `rename_tag`, `delete_tag`, `remove_post_from_community`, `create_comment`, `delete_comment`, `leave_community`, `edit_profile` | a write on the file, then a sync in the background; an error is the reason to show |

### Development

The sync endpoints are on the server at `PHOENIX_URL`, the same one the
websocket API uses. The server refuses sync requests unless
`API_SOCKET=true`, like the websocket API.

Run the Rust tests with `cargo test` in `src-tauri`. They exercise the
file, the outbox, the events, conflicts and the answers to a push, every
write above with its permission checks and copied deletes, and the
pruning, against an in-memory database; the
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
either clock says, and that the conflict reapplied reaches both devices, and that different
fields of the profile edited on each device both apply.
