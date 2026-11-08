-- A device's pull cursor, see src/sync.rs: one row, the key of the last
-- row pulled. The server has the table too and leaves it empty.
CREATE TABLE sync_state (
  id integer PRIMARY KEY,
  cursor_last_synced_at timestamptz NOT NULL,
  cursor_id uuid NOT NULL
);
