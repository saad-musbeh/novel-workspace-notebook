CREATE TABLE IF NOT EXISTS scene_revision (
  id TEXT PRIMARY KEY NOT NULL,
  parent TEXT REFERENCES scene_revision(id),
  payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE TABLE IF NOT EXISTS scene_head (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
  revision_id TEXT NOT NULL REFERENCES scene_revision(id)
);
PRAGMA user_version = 1;
