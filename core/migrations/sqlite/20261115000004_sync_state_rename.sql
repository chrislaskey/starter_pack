-- Every table of the app's has its prefix, see docs/design/multi-app.md.
ALTER TABLE sync_state RENAME TO app_sync_state;
