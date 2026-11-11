-- The address a pending sign-in's code was sent to, when it is not yet
-- the account's: a trial user signing up, whose account takes the
-- address once the code matches, see src/accounts.rs. Null otherwise.
ALTER TABLE accounts_tokens ADD COLUMN sent_to TEXT;
