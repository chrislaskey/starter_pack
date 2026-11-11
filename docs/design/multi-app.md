# Multi-app

One deployment carries the whole product, not only the application:
the public website, account management, support, a forum, and the
staff's tools. Each is an **app**: a boundary inside the one web
server, the one database and the one sign-in, not a service of its own.

Status: the structure is built, 2026-11-10: the tables' names, the two
rows of a user, who is staff, the paths, the nav and a placeholder
landing page for `accounts`, `support`, `forum` and `ops`. `public` is
built, see `public-pages.md`. Each other app's own tables, API and
pages are planned one at a time; what is still to decide is at the end.

## The apps

| App | What | Tables | On a device |
|---|---|---|---|
| `app` | the application itself | `app_*`, synced | all of it, offline |
| `public` | the public website | none | never |
| `accounts` | sign-in, the account, purchasing | `accounts_*` | online only; a phone buys with the store's in-app purchase |
| `support` | support requests | `support_*` | online only |
| `forum` | discussion | `forum_*` | online only |
| `ops` | the staff's dashboard | `ops_*` | never |
| `sales` | deferred | | |

The forum is `forum`, not "community": a community is already the
boundary inside a deployment.

## An app is a name in four places

`app` is the default everywhere, so what exists stays where it is. Any
other app carries its name:

| Place | `app` | Another app |
|---|---|---|
| Table | `app_posts` | `support_tickets` |
| Core module | `core/src/posts.rs` | `core/src/support/` |
| API path | `/api/posts` | `/api/support/...` |
| Page path and folder | `/posts`, `svelte/src/posts/` | `/support/...`, `svelte/src/support/` |

- **Paths**: `app` owns the top level and is the fallback; another app
  owns its subpath. The public website owns `/` and its pages' names.
- **Pages**: one Svelte app, each page loaded when visited, as today.
  `ops` is the exception: an entry of its own, `svelte/src/ops.html`
  with `svelte/src/ops/`, so the staff's tools are not in what users
  load. The web server answers `/ops` and every path under it with it
  (`axum/src/ops.rs`), and a link to it loads the page in full.
- **The nav** (`svelte/src/lib/AppNav.svelte`) links the apps: Support,
  Forum, the person's email to their account, and Ops for staff.
- **Depends on**: `accounts`, then `app`, then every other app. Never
  the other way.

## Only `app_*` syncs

The prefix says which app a table is, and that says whether it leaves
the server: `app_*` tables sync to devices, every other table is the
server's alone.

- The sync's own tables are the app's: `app_events`, `app_jobs`,
  `app_purges`, `app_sync_state`, `app_sync_backfills`.
- `device_*` tables are a device's own and never synced.
- On a device, `accounts`, `support` and `forum` are online only: their
  pages call the web server's API with the device's token, and a change
  is the server's at once. Nothing of theirs is in the device's
  database.
- Purchasing on a phone is the store's in-app purchase. What a device
  must know offline, the person's plan, it keeps from the last time it
  was online, in `device_settings`, never through `app_users`, which
  other members see.

## A user is two rows

One id, two tables, written together at sign-up:

| Table | Holds | Leaves the server |
|---|---|---|
| `accounts_users` | email, `confirmed_at`, later the subscription | never |
| `app_users` | name, `avatar_url`, the sync columns | synced, to the members of the user's communities |

- An email never reaches other people's devices. A page shows a user
  by **name**, asked for at sign-up (`accounts::sign_up`), which writes
  both rows in one transaction.
- `accounts_tokens` and `accounts_credentials` reference
  `accounts_users`. A device has the `accounts_` tables and leaves them
  empty.
- A person is shown their own email by `GET /api/current-user`, which
  adds it and `is_staff` to their user; a device adds it from its list
  of accounts, which a sign-in's answer fills.
- The purge removes an unconfirmed sign-up's two rows after a day, and
  the account of any user it removes.
- `forum` and `support` show the same name and picture, from
  `app_users`, and read `accounts_users` to email someone.

## Staff

`ops_staff` has a row per account that may use `ops`
(`core/src/ops/staff.rs`).

- Every route under `/api/ops` is behind one check,
  `axum/src/ops.rs`: 401 from nobody, 403 from someone who is not
  staff.
- Someone becomes staff from the server's command line, never from a
  page: `app_web ops staff create <email>`, and `ops staff delete
  <email>`.
- The ops page itself is served to anyone and says why they cannot use
  it; what it shows comes from the API.

## Still to decide

- The build order of the apps. Proposed: `ops` with a users list;
  `support`; purchasing in `accounts`; `forum`.
- How a device's pages call an online-only app's API: the first one
  built sets the pattern in `svelte/src/lib/core/`.
- Whether a deployment can leave an app out, so `ops` can run where
  only staff reach it, and keeping the ops entry out of the device
  bundles, which today carry every built file.
