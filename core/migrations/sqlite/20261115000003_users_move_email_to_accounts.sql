-- The email and when it was proved are the account's, accounts_users,
-- and stay on the server; what syncs of a user is the name the pages
-- show them by, see src/users.rs.
DROP INDEX app_users_email;
ALTER TABLE app_users DROP COLUMN email;
ALTER TABLE app_users DROP COLUMN confirmed_at;
ALTER TABLE app_users ADD COLUMN name TEXT NOT NULL DEFAULT '';
