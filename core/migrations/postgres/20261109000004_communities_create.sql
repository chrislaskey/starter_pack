-- Communities, see src/communities.rs: the soft boundary inside a
-- deployment, see docs/design/offline-sync.md. Any user creates one, and
-- every user sees every one, so a device can list them and join.
CREATE TABLE app_communities (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE INDEX app_communities_last_synced_at ON app_communities (last_synced_at, id);
