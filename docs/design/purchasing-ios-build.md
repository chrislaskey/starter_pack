# Deleting an account and iOS purchasing: the build plan

**Status: planned and agreed 2026-11-12, not started.** Steps in order, each with
what it must do to be done. The reasons are in `accounts-delete.md`
and `purchasing-phones.md`; read both, and `purchasing.md`, before
starting. Android is a later effort; nothing here may make it a
rewrite, see "iOS first, Android without a rewrite" in
`purchasing-phones.md`.

## Rules that hold in every step

- Every step ends with `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --all-targets`, `cargo audit` (the
  `libsqlite3-sys` advisory is old and known), and for pages
  `pnpm check` and `pnpm build` in `svelte/`.
- The user does the git commits. Never commit; keep building.
- Ask before a migration or a new crate, naming it, except those
  marked **approved** below (approved 2026-11-12).
- **No real credentials, ever.** The repo is public. Every store and
  Stripe call is tested through its `Memory` fake; the app through an
  Xcode StoreKit configuration file in the simulator. Settings are
  read from the environment and documented, never filled in.
- **`Starter Pack`** is the placeholder product name in every page and
  document a fork changes, so a find and replace renames it.
- Disk: the target folder fills the disk; build with
  `CARGO_INCREMENTAL=0` and `cargo clean` when free space is low.
- Each step leaves the app working, so the work can stop between any
  two. Tick the boxes as they are built.
- Names follow AGENTS.md: resource first (`deletions::create`,
  `accounts_deletions_create`, `AccountsDeletionsNewPage`).

## Decisions taken

`accounts-delete.md` 1 to 7 as recommended: a deleted user's content
goes everywhere, no undo, a Stripe plan ends at once with no refund,
an emailed code confirms, other devices remove the account's data on
`410`, an App Store plan warns and links to cancel, a mail after.
`purchasing-phones.md` To decide 1 to 4, 6 to 9. Terms and privacy
pages, step 1a.

## Step 0. Check the two crates

**Approved** if they pass these checks.

Before step 3 and step 4 use them, as `async-stripe` was checked in
`purchasing.md`'s "Checked":

- [ ] `tauri-plugin-iap` (Choochmeque, 0.8 or newer): maintained (recent
      commits and releases), license, no RustSec advisory, `cargo
      audit` clean with it; that on iOS it gives Apple's signed
      transaction (JWS) after a purchase and for unfinished and
      current transactions; that it takes `appAccountToken`; that it
      finishes a transaction only when asked; that it opens the
      subscriptions page; that the app's Rust can call it, not only
      JavaScript. Write what was found in `purchasing-phones.md`.
- [ ] `app-store-server-library` (4.x): the same, plus that it verifies
      signed transactions and notifications against Apple's root
      certificates, calls Get Transaction Info and Get All Subscription
      Statuses, and which TLS stack it pulls in (we use rustls).
- If either fails a check, stop and ask: the fallback is a small
  plugin or client of our own, which AGENTS.md says to ask about.

## Step 1a. Terms and privacy

Apple asks every app for a privacy policy link, and a plan's buy page
for the terms and the privacy policy.

- [ ] `svelte/src/public/terms.html` (`/terms`) and
      `svelte/src/public/privacy.html` (`/privacy`): standard terms of
      service and privacy policy for a SaaS web application with
      desktop and phone apps, for `Starter Pack`, with the legal
      entity, contact email, governing law and effective date as
      obvious placeholders such as `[Company name]`. They cover what
      this app actually does: accounts by email, synced content,
      uploaded files, payments by Stripe (Managed Payments as
      merchant of record) and the App Store, cookies for the session
      only, deleting an account and the 90 days before rows are gone,
      no selling of data. A comment at the top of each says it is a
      template, not legal advice, to be reviewed for a real product.
- [ ] Links in `svelte/src/public/partials/footer.html`; the sign-up
      page says signing up accepts them, with links.
- [ ] In the apps, which never show public pages, the links open
      `SERVER_URL/terms` and `/privacy` in the system browser.
- [ ] The router test that lists public pages gains both.

## Step 1. Deleting an account

`accounts-delete.md`. No migration.

- [ ] `tokens::Context::DeleteCode`, ten minutes.
- [ ] `core/src/accounts/deletions.rs`: `code_create(deployment, user)`
      mails a code (a full user only); `create(deployment, stripe,
      user, code)` checks it and does "What the delete does" 1 to 6 in
      one transaction, the soft deletes as events through each
      resource's `apply_event`; then 7 to 9 after the commit, a failure
      logged and left to the jobs. A trial user: no code, the trial's
      outright delete. A local user: refused.
- [ ] A token of a deleted account answers `410` with
      `account_deleted` wherever a token is checked (API, cookie,
      device token); `api_error.rs` maps it.
- [ ] `stripe_reconcile` cancels a live Stripe subscription of a
      deleted account.
- [ ] The purge removes the deleted user and account after 90 days;
      check it already does with a test, fix it if not.
- [ ] Routes `POST /api/accounts/deletions/code`,
      `POST /api/accounts/deletions`.
- [ ] Device: `online::deletions_code_create`, `online::deletions_create`;
      Tauri `accounts_deletions_code_create`, `accounts_deletions_create`,
      which removes the account from the device (`DeviceAccounts::remove`)
      on success. The sync and the online calls treat `410` by removing
      that account and its file and showing "This account was deleted".
- [ ] `svelte/src/lib/core/` both files; `AccountsDeletionsNewPage` at
      `/accounts/deletions/new`, linked from `AccountsSettingsPage`:
      what goes, the plan warning by store, Send code, the code, Delete
      my account. After it, the start page.
- [ ] Tests: the delete soft-deletes posts, files, memberships, the
      sole-member community, the user; another member's device pulls
      the deletes; a shared community stays; sign-in fails and the
      email signs up again; the old token gets `410`; a wrong or old
      code is refused; the Stripe fake's subscription is cancelled and
      its customer deleted; the trial path; the purge after 90 days.
- [ ] Update `accounts-delete.md`'s status and `purchasing.md` P12.

## Step 2. Store-neutral rows

`purchasing-phones.md`, "iOS first, Android without a rewrite".

- [ ] `core/src/accounts/store_transactions.rs`: `StoreTransaction`
      (the store, the user id it carried, the lookup key, one-time or
      subscription details), `StoreProof` (only `AppStore {
      signed_transaction }` for now), and `apply_in(tx, transaction,
      sent_by: Option<Uuid>)`: M3 (another user's id refused, none
      given to the sender unless the row is someone else's), lookup key
      to the active Stripe price with that key (refused if none),
      the one-plan rule, the repeatable rule, the replaced subscription
      found by its store id, the upsert into `accounts_purchases` or
      `accounts_subscriptions` with the scheduled price by lookup key.
- [ ] `plans::current` and the pages unchanged in behaviour for store
      rows; `subscriptions::update` already answers `OtherStore`.
- [ ] Tests with hand-made `StoreTransaction`s: each rule above, on
      SQLite (and Postgres when `TEST_POSTGRES_URL` is set).

## Step 3. The App Store, server

Migration, **approved**: `accounts_app_store_notifications` (Apple's
`notificationUUID` as id, type, `handled_at`), as
`accounts_stripe_events`. New crate, **approved** after step 0:
`app-store-server-library`, behind a new `app-store` feature of core
that only `axum` enables. The `Memory` fake takes notifications and
transactions as plain values, so tests need no Apple-signed data.

- [ ] `core/src/accounts/app_store.rs`: `enum AppStore { Api(..),
      Memory(..) }` as `Stripe`; settings `APP_STORE_BUNDLE_ID`,
      `APP_STORE_APP_ID`, `APP_STORE_ISSUER_ID`, `APP_STORE_KEY_ID`,
      `APP_STORE_PRIVATE_KEY` (`SecretString`), the environment taken
      from the data; Apple's root certificates checked into the repo;
      verifying in `spawn_blocking` (its OCSP call blocks); the
      transaction and the subscription statuses fetched and turned
      into `StoreTransaction`s; the status mapping onto
      `SubscriptionStatus` (billing retry is `past_due`, grace off,
      expired and revoked are `canceled`).
- [ ] `Deployment::with_app_store`; `axum` opens it when the settings
      are there.
- [ ] `core/src/accounts/app_store_notifications.rs` as
      `stripe_events.rs`: verify, record the id in the transaction that
      handles it, skip a repeat, fetch the current state by the
      original transaction id, `store_transactions::apply_in`; `REFUND`
      through `purchases::refund_in`.
- [ ] Routes `POST /api/accounts/store-transactions` (signed in, full
      user) and `POST /api/accounts/app-store-notifications` (no
      session, 404 unless configured).
- [ ] `Job::AppStoreReconcile`, `app_store_reconcile`, daily, when the
      deployment has the App Store: every live App Store subscription
      asked again.
- [ ] Tests with the fake: a purchase, a subscription, a renewal, a
      failed renewal, a downgrade scheduled, a refund, a repeated
      notification, another user's transaction, the reconcile.

## Step 4. The App Store, app

New crate, **approved** after step 0: `tauri-plugin-iap`, iOS only in
`tauri/Cargo.toml`'s `cfg(target_os = "ios")` dependencies.

- [ ] `tauri/src/store_sheet.rs`: the `StoreSheet` trait (prices for
      lookup keys, buy, unfinished transactions, finish, open
      subscriptions), the iOS one on the plugin, and an "not available
      yet" one for Android.
- [ ] Commands: the products commands add `store` and the store's
      price on a phone; `accounts_store_transactions_create(price_id)`
      runs M1 (buy with `appAccountToken` = user id, send the proof,
      finish only after the server answered); 
      `accounts_store_transactions_restore` runs M2; the app sends
      unfinished transactions at start and after sign-in;
      `accounts_subscriptions_manage_open` opens the store's page.
      `refused_on_a_phone` stays only for Android.
- [ ] `online::store_transactions_create`.
- [ ] Pages: the store's price; Buy by store; Restore purchases on
      the products page when `store` is `app_store`; Manage in the App
      Store on an App Store plan; "Manage on the web" for a Stripe plan
      on a phone and the reverse on web and desktop; the banner's link
      by store; the subscription's name, length, price and the terms
      and privacy links on a plan's page on iOS.
- [ ] An Xcode StoreKit configuration file with the catalog's four
      products, to try purchases in the simulator without App Store
      Connect.
- [ ] Tried by hand in the simulator: buy each, restore, a switch,
      Manage. Say plainly in the report what was and was not tried.

## Step 5. Documents

- [ ] "Setting up the App Store" in `purchasing-phones.md`, as
      "Setting up Stripe" is: agreements, Small Business Program, the
      subscription group and the four products with the catalog's ids,
      grace period off, the In-App Purchase key, the notification URLs
      for production and sandbox, a sandbox tester, and the
      environment settings to give the server.
- [ ] Status lines of the three documents; "As built" at the end of
      this one for anything built another way than a box says.
- [ ] `docs/suggestions/`: linking to the web checkout in the US,
      exporting one's data.
