-- Memberships, see src/memberships.rs: a user in a community, with a
-- role that is a string the app interprets. Leaving soft-deletes the
-- row; rejoining revives the same row, so the unique index is plain.
CREATE TABLE app_users_communities (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  community_id TEXT NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE UNIQUE INDEX app_users_communities_user_community ON app_users_communities (user_id, community_id);
CREATE INDEX app_users_communities_community_id ON app_users_communities (community_id);
CREATE INDEX app_users_communities_last_synced_at ON app_users_communities (last_synced_at, id);
