-- Passwords, see src/credentials.rs: one row per user who signed up with
-- one, holding the hash. Server only; a device has the table and leaves
-- it empty. Credentials never leave the server.
CREATE TABLE users_credentials (
  user_id uuid PRIMARY KEY REFERENCES app_users (id) ON DELETE CASCADE,
  password_hash text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);
