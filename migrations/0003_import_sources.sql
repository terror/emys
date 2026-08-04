CREATE TABLE import_sources (
  id          TEXT PRIMARY KEY NOT NULL,
  format      TEXT NOT NULL CHECK (format <> ''),
  path        BLOB NOT NULL CHECK (LENGTH(path) > 0),
  generation  INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  UNIQUE (format, path)
) STRICT;

CREATE TABLE source_records (
  source_id     TEXT NOT NULL,
  position      INTEGER NOT NULL CHECK (position >= 0),
  fingerprint   BLOB NOT NULL,
  execution_id  TEXT NOT NULL,
  PRIMARY KEY (source_id, position),
  FOREIGN KEY (source_id) REFERENCES import_sources(id) ON DELETE CASCADE,
  FOREIGN KEY (execution_id) REFERENCES executions(id) ON DELETE CASCADE
) STRICT;
