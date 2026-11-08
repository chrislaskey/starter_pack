-- The communities a device still has to ask the server for everything
-- in, see src/sync.rs: one row per community, written when a membership
-- of the device's user arrives and removed once the backfill is done.
-- The server has the table and leaves it empty.
CREATE TABLE sync_backfills (
  community_id TEXT PRIMARY KEY NOT NULL
);
