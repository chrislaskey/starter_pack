-- users_tokens as accounts_tokens, of an accounts_users row, see
-- src/tokens.rs. Made anew, since the old rows' users have no account;
-- everyone signs in again.
DROP TABLE users_tokens;

CREATE TABLE accounts_tokens (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  token_hash text NOT NULL UNIQUE,
  inserted_at timestamptz NOT NULL,
  context text NOT NULL DEFAULT 'api',
  code_hash text,
  attempts integer NOT NULL DEFAULT 0
);

CREATE INDEX accounts_tokens_user_id ON accounts_tokens (user_id);
