-- The accounts on this device, see src/device_accounts.rs: one row per
-- account, pointing at the account's own database file under accounts/.
-- No token and no user data: those are in the account's file.
CREATE TABLE device_accounts (
  user_id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL,
  file TEXT NOT NULL,
  signed_in INTEGER NOT NULL DEFAULT 0,
  last_active_at TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0
);
