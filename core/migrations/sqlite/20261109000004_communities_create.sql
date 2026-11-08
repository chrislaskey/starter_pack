-- Communities, see src/communities.rs: the soft boundary inside a
-- deployment, see docs/design/offline-sync.md. Any user creates one, and
-- every user sees every one, so a device can list them and join.
CREATE TABLE app_communities (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_communities_last_synced_at ON app_communities (last_synced_at, id);
