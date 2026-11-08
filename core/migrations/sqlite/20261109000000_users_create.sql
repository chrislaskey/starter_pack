-- The app's users, see src/users.rs: one row per account, written by the
-- server at registration and synced to devices like any row, so a post
-- can show who wrote it. Credentials never leave the server and come
-- with the login, later, in a table of their own with the same ids.
CREATE TABLE app_users (
  id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

-- Partial, so a deleted account's email can register again
CREATE UNIQUE INDEX app_users_email ON app_users (email) WHERE deleted_at IS NULL;
CREATE INDEX app_users_last_synced_at ON app_users (last_synced_at, id);
