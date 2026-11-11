-- A person's account, see src/accounts/users.rs: their email and when
-- they proved it. Server only, like every accounts_ table; a device has
-- the table and leaves it empty. The user's row the apps sync,
-- app_users, has the same id.
CREATE TABLE accounts_users (
  id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL,
  confirmed_at TEXT,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

-- Partial, so a deleted account's email can register again
CREATE UNIQUE INDEX accounts_users_email ON accounts_users (email) WHERE deleted_at IS NULL;
