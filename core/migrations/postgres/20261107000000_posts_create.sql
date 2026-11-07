-- Posts, see src/posts.rs. Ids are UUIDv7, made by whichever device
-- writes the row; times are kept to the microsecond.
CREATE TABLE app_posts (
  id uuid PRIMARY KEY,
  body text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);

CREATE INDEX app_posts_inserted_at ON app_posts (inserted_at);
