-- A post has an owner, and a delete records who deleted it. The posts
-- before users had no owner, so they go, with their events.
DELETE FROM app_posts;
DELETE FROM app_events;

ALTER TABLE app_posts ADD COLUMN owner_id uuid NOT NULL REFERENCES app_users (id) ON DELETE CASCADE;
ALTER TABLE app_posts ADD COLUMN deleted_by_id uuid;

CREATE INDEX app_posts_owner_id ON app_posts (owner_id);
