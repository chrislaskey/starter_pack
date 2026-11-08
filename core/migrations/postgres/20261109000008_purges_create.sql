-- One row per purge run, see src/purge.rs: the horizon, the oldest
-- deletion still kept after it. Server only; a device whose cursor is
-- older than the latest horizon resyncs from nothing.
CREATE TABLE app_purges (
  id uuid PRIMARY KEY,
  horizon timestamptz NOT NULL,
  inserted_at timestamptz NOT NULL
);
