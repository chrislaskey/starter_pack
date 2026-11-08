-- Passwords, see src/credentials.rs: one row per user who signed up with
-- one, holding the hash. Server only; a device has the table and leaves
-- it empty. Credentials never leave the server.
CREATE TABLE users_credentials (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  password_hash TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
