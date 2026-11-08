# Accounts: what was left out

The sign-up, sign-in and sign-out with an email and a password
(`core/src/accounts.rs`) are the smallest flow that puts a real user on
every deployment. The Phoenix app has the rest, and each is a layer on
top of this one:

- **Magic links and email confirmation.** Phoenix logs a new user in by
  a link sent to the email, and confirms the address that way. Here a
  sign-up signs in at once and the email is never confirmed. Needs a
  mailer and a `login` token context.
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
- **Rate limiting sign-in attempts**, which belongs in front of the web
  server.
- **Tokens with a context**, `session` and `api` as Phoenix keeps them,
  if the browser's cookie and the apps' tokens should ever differ in
  validity.
