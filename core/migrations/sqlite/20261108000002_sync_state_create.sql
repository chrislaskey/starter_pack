-- A device's pull cursor, see src/sync.rs: one row, the key of the last
-- row pulled. The server has the table too and leaves it empty.
CREATE TABLE sync_state (
  id INTEGER PRIMARY KEY NOT NULL,
  cursor_last_synced_at TEXT NOT NULL,
  cursor_id TEXT NOT NULL
);
