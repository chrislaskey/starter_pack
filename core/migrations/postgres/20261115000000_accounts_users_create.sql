-- A person's account, see src/accounts/users.rs: their email and when
-- they proved it. Server only, like every accounts_ table. The user's
-- row the apps sync, app_users, has the same id.
CREATE TABLE accounts_users (
  id uuid PRIMARY KEY,
  email text NOT NULL,
  confirmed_at timestamptz,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  deleted_at timestamptz
);

-- Partial, so a deleted account's email can register again
CREATE UNIQUE INDEX accounts_users_email ON accounts_users (email) WHERE deleted_at IS NULL;
