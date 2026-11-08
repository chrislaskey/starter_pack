-- The app's users, see src/users.rs: one row per account, written by the
-- server at registration and synced to devices like any row, so a post
-- can show who wrote it. Credentials never leave the server and come
-- with the login, later, in a table of their own with the same ids.
CREATE TABLE app_users (
  id uuid PRIMARY KEY,
  email text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  deleted_at timestamptz,
  deleted_by_id uuid
);

-- Partial, so a deleted account's email can register again
CREATE UNIQUE INDEX app_users_email ON app_users (email) WHERE deleted_at IS NULL;
CREATE INDEX app_users_last_synced_at ON app_users (last_synced_at, id);
