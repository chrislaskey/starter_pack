-- Posts, see src/posts.rs. Ids are UUIDv7 text, made by whichever device
-- writes the row; times are RFC 3339 text in UTC, as sqlx writes them.
CREATE TABLE app_posts (
  id TEXT PRIMARY KEY NOT NULL,
  body TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX app_posts_inserted_at ON app_posts (inserted_at);
