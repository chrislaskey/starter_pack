-- A token has a context, see src/tokens.rs: 'api' for the apps' bearer
-- tokens, 'session' for the browser's cookie, 'sign_in_code' for a
-- sign-in that passed the password and waits for its emailed code, whose
-- hash and wrong attempts so far are the two columns after, and
-- 'device_code' for a finished browser sign-in a device collects once.
ALTER TABLE users_tokens ADD COLUMN context text NOT NULL DEFAULT 'api';
ALTER TABLE users_tokens ADD COLUMN code_hash text;
ALTER TABLE users_tokens ADD COLUMN attempts integer NOT NULL DEFAULT 0;
