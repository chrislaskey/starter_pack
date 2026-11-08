# Communities: what was left out

Built in the port of `phoenix/docs/design/multi-tenancy.md`: communities,
memberships with join, leave and rejoin, a post in the communities its
owner chose, visibility, backfill and prune, see
`docs/design/offline-sync.md`. Left for later, on purpose:

- **A community's own page**, `/communities/:id`, listing its posts, and
  its members. `posts::list` already answers everything a user sees with
  each post's communities; a feed of one community is a filter on the
  same visibility predicate, and the members are the live memberships of
  the community, which a device holds once it is in it.
- **Roles beyond `member`.** `role` is a string column every row holds
  `member` in; `admin`, and whatever an admin may do, such as removing
  another member's post from the community, is an app's rule.
- **Editing or deleting a community.** An update event on
  `app_communities` is refused today. Who may change one is a role.
- **Discoverable and public communities.** Every user sees every
  community of the deployment, which is what lets a device list and join
  them. An app that wants private communities needs a rule for who sees
  which, in each table's `visible_to!` predicate.
- **A user's name or profile.** `app_users` holds an email; the pages
  show it as the author.
- **Registering, logging in, logging out, deleting an account.** Every
  deployment acts as the dev user, see "Who the user is" in
  `README.md`.
- **The event log on a device is kept forever.** Applied events are
  deleted from a device's queue, so only conflicts stay; the server's
  log grows with every write, and the purge removes only the events of
  rows that are gone.
