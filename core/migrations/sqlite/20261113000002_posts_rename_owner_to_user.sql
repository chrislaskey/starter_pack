-- A post's owner is its `user_id`, the name every table uses.
ALTER TABLE app_posts RENAME COLUMN owner_id TO user_id;
DROP INDEX app_posts_owner_id;
CREATE INDEX app_posts_user_id ON app_posts (user_id);
