-- The public URL of the user's picture in object storage, see
-- src/avatars.rs; null until they upload one.
ALTER TABLE app_users ADD COLUMN avatar_url TEXT;
