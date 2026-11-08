-- API tokens, see src/tokens.rs: what a device sends as a bearer token,
-- resolved to its user by the server. Server only; a device has the table
-- and leaves it empty.
CREATE TABLE users_tokens (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  token text NOT NULL UNIQUE,
  inserted_at timestamptz NOT NULL
);

CREATE INDEX users_tokens_user_id ON users_tokens (user_id);
