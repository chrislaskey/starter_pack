-- A trial user, see src/trial_users.rs: an account made on the web with
-- no sign-up, which says until when; null for every other account, and
-- again once a trial signs up.
ALTER TABLE accounts_users ADD COLUMN trial_expires_at timestamptz;
