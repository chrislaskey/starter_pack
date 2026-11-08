-- A post has an owner, and a delete records who deleted it. The posts
-- before users had no owner, so they go, with their events; SQLite cannot
-- add a required column that references another table, so the table is
-- made again.
DROP TABLE app_posts;
DELETE FROM app_events;

CREATE TABLE app_posts (
  id TEXT PRIMARY KEY NOT NULL,
  owner_id TEXT NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_posts_owner_id ON app_posts (owner_id);
CREATE INDEX app_posts_inserted_at ON app_posts (inserted_at);
CREATE INDEX app_posts_last_synced_at ON app_posts (last_synced_at, id);
