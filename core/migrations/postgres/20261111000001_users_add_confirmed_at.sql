-- When the user first proved their email by entering a code sent to it,
-- see src/accounts.rs; null until then, and such a user has no token.
ALTER TABLE app_users ADD COLUMN confirmed_at timestamptz;
