-- A post's owner is its `user_id`, the name every table uses.
ALTER TABLE app_posts RENAME COLUMN owner_id TO user_id;
ALTER INDEX app_posts_owner_id RENAME TO app_posts_user_id;
