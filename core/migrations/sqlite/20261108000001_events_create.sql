-- Events, see src/events.rs: one change a person made to a row. On the
-- server the log of every event answered; on a device the queue of events
-- not yet answered, and the conflicts the person has not resolved.
CREATE TABLE app_events (
  id TEXT PRIMARY KEY NOT NULL,
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
