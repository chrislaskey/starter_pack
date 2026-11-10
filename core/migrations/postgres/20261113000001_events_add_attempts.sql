-- How many times the server or the bucket said no to an event still in
-- a device's queue, see src/events.rs: at the limit the event stops
-- being tried and waits for the person. 0 on the server, always.
ALTER TABLE app_events ADD COLUMN attempts bigint NOT NULL DEFAULT 0;
