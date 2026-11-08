-- An event records who made it: on the server the user the token
-- resolved to, on a device the user signed in.
ALTER TABLE app_events ADD COLUMN user_id uuid NOT NULL;
