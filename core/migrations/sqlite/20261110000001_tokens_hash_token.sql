-- A token is kept as its hash, so reading the table does not give a token
-- that works, see src/tokens.rs. The rows so far held tokens in the
-- clear, from before there was a sign-in, and go.
DELETE FROM users_tokens;
ALTER TABLE users_tokens RENAME COLUMN token TO token_hash;
