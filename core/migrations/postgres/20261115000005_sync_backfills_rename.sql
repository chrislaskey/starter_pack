-- Every table of the app's has its prefix, see docs/design/multi-app.md.
ALTER TABLE sync_backfills RENAME TO app_sync_backfills;
