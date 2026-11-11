-- A local user: made by Skip on this device, with no account on the
-- server, see src/local_users.rs. When they sign up or sign in, the row
-- becomes that account's and keeps its folder, so `file` no longer
-- carries the account's own id.
ALTER TABLE device_accounts ADD COLUMN local INTEGER NOT NULL DEFAULT 0;
