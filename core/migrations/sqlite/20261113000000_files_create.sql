-- Files, see src/files.rs and docs/design/offline-files-decisions.md: a
-- row for a file a person added, whose bytes are in the server's bucket
-- and in the account's folder on a device. Only the name changes.
CREATE TABLE app_files (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  content_type TEXT NOT NULL,
  size INTEGER NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_files_user_id ON app_files (user_id);
CREATE INDEX app_files_last_synced_at ON app_files (last_synced_at, id);
