-- Events, see src/events.rs: one change a person made to a row. On the
-- server the log of every event answered; on a device the queue of events
-- not yet answered, and the conflicts the person has not resolved.
CREATE TABLE app_events (
  id uuid PRIMARY KEY,
  table_name text NOT NULL,
  row_id uuid NOT NULL,
  type text NOT NULL,
  base_version bigint NOT NULL,
  base jsonb NOT NULL,
  changes jsonb NOT NULL,
  made_at timestamptz NOT NULL,
  status text NOT NULL,
  conflicts jsonb,
  version bigint,
  last_error text
);

CREATE INDEX app_events_row ON app_events (table_name, row_id);
CREATE INDEX app_events_status ON app_events (status, made_at, id);
