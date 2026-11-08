-- An event records who made it: on the server the user the token
-- resolved to, on a device the user signed in. Made again rather than
-- altered, since the column is required and the table was just emptied.
DROP TABLE app_events;

CREATE TABLE app_events (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  table_name TEXT NOT NULL,
  row_id TEXT NOT NULL,
  type TEXT NOT NULL,
  base_version INTEGER NOT NULL,
  base TEXT NOT NULL,
  changes TEXT NOT NULL,
  made_at TEXT NOT NULL,
  status TEXT NOT NULL,
  conflicts TEXT,
  version INTEGER,
  last_error TEXT
);

CREATE INDEX app_events_row ON app_events (table_name, row_id);
CREATE INDEX app_events_status ON app_events (status, made_at, id);
