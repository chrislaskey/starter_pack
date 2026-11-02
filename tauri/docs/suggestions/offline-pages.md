# Suggestions for the offline-first pages

Not built; each needs a decision first.

## Creating and joining a community

The web's communities page creates communities and joins any of them. The
desktop page only opens and leaves the user's own.

- **Joining** needs every community's name, and the device only syncs the
  communities the user is in. An `api_request` such as
  `communities.communities.list` over the websocket would list them while
  online; the join itself can then go through the outbox as a membership
  row, as leaving does.
- **Creating** offline is a community row and a membership row in the
  outbox. The server's `Memberships.join/2` makes the membership with its
  own id, so the device's membership comes back as a different row and its
  own is dropped, with any post rows in that community written meanwhile
  through the foreign key. Either the server keeps the device's id for a
  new membership, or a create stays online only.

## A post written then taken out of its community before a sync

The post's community row is new and already deleted, so the server
answers it `server_wins` with no row and the device drops it. That is
right, but the post's history never shows it was there. Fine for now;
worth a look if the history screen comes.

## Editing a note or reordering a post's tags

The events allow both, see "Open" in
`phoenix/docs/design/related-resources.md`; neither the web nor the
desktop has a screen for them yet.
