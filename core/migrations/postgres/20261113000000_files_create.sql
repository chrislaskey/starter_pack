-- Files, see src/files.rs and docs/design/offline-files-decisions.md: a
-- row for a file a person added, whose bytes are in the server's bucket
-- and in the account's folder on a device. Only the name changes.
CREATE TABLE app_files (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  name text NOT NULL,
  content_type text NOT NULL,
  size bigint NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE INDEX app_files_user_id ON app_files (user_id);
CREATE INDEX app_files_last_synced_at ON app_files (last_synced_at, id);
