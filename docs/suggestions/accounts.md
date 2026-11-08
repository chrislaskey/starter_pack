# Accounts: what was left out

The sign-up and sign-in with an email and a password and the emailed
code (`core/src/accounts.rs`, the design in
`docs/design/accounts-sign-in.md`), and the apps signing in through the
browser, are the smallest flow that puts a real, confirmed user on every
deployment. The Phoenix app has the rest, and each is a layer on top of
this one:

- **Magic links.** Phoenix logs a user in by a link sent to the email;
  here the email carries a code. A link is the same pending sign-in
  with the pending token and the code in the URL, and on a phone it
  can open the app directly once the universal links exist.
- **Changing the password, and resetting a forgotten one** by a link
  sent to the email.
- **Changing the email**, with a confirmation link, which also has to
  update the synced `app_users` row.
- **Sudo mode**: asking for the password again before a sensitive
  change, as Phoenix does within twenty minutes of a sign-in.
- **Deleting an account**, see the Phoenix build order's phase 7.
- **Keeping a browser signed in across its sessions.** The cookie lasts
  the browser session; a "remember me" would give it the token's year.
- **Signing out everywhere**: deleting every token of the user, not
  just the one the request carried, and a page listing the tokens.
- **Signing out, or removing an account, while offline.** A device
  cannot sign out without reaching the server to forget its token, and
  Remove from device signs out first, so a person offline cannot hand
  the device over. The alternative, dropping the data and leaving the
  token to expire, is a choice to make deliberately.
- **Encrypting the account files.** A signed-out account's file stays
  readable on disk until it is removed, the same exposure the active
  account has; SQLCipher would close it for every file.
- **Returning to the page an account was last on** after a switch,
  instead of the landing page.
- **Remembering a browser** for thirty days, so the code is not asked
  on every new browser session: Phoenix's remember-me, a longer cookie
  on the `session` token.
- **The automatic returns to the app** after a browser sign-in, in place
  of pasting the code: a loopback redirect on desktop, and universal
  links on iOS and Android, see "To decide" 6 in the design.
- **A dev mailbox page** showing the emails the `Memory` mailer holds,
  as Phoenix's `/dev/mailbox` does, instead of reading the log.
- **HTML emails and a sender name**; the emails are text only.
- **Continue as the person signed in here** on the sign-in page a device
  opened, when the browser already has a session.
- **Rate limits shared between several servers**: `governor`'s buckets
  are this process's; several copies of the server need a shared store
  or a limit at the proxy.
