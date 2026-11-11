-- users_credentials as accounts_credentials, of an accounts_users row,
-- see src/credentials.rs. Made anew, as accounts_tokens is.
DROP TABLE users_credentials;

CREATE TABLE accounts_credentials (
  user_id uuid PRIMARY KEY REFERENCES accounts_users (id) ON DELETE CASCADE,
  password_hash text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);
