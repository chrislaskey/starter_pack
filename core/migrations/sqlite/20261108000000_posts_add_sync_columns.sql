-- The sync columns, see docs/design/offline-sync.md: the server's version
-- of the row, 0 on a device's row not yet pushed; the time the server
-- last wrote it, the pull cursor, null until then; and the soft delete.
ALTER TABLE app_posts ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE app_posts ADD COLUMN last_synced_at TEXT;
ALTER TABLE app_posts ADD COLUMN deleted_at TEXT;

CREATE INDEX app_posts_last_synced_at ON app_posts (last_synced_at, id);
