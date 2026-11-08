-- A post in a community, see src/posts_communities.rs, added under the
-- owner's membership, which leaving follows. Removing soft-deletes the
-- row; adding back revives the same row, so the unique index is plain.
CREATE TABLE app_posts_communities (
  id uuid PRIMARY KEY,
  post_id uuid NOT NULL REFERENCES app_posts (id) ON DELETE CASCADE,
  community_id uuid NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  users_communities_id uuid NOT NULL REFERENCES app_users_communities (id) ON DELETE CASCADE,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE UNIQUE INDEX app_posts_communities_post_community ON app_posts_communities (post_id, community_id);
CREATE INDEX app_posts_communities_community_id ON app_posts_communities (community_id);
CREATE INDEX app_posts_communities_users_communities_id ON app_posts_communities (users_communities_id);
CREATE INDEX app_posts_communities_last_synced_at ON app_posts_communities (last_synced_at, id);
