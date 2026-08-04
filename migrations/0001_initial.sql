CREATE TABLE entries (
  id            TEXT PRIMARY KEY NOT NULL,
  command       TEXT NOT NULL CHECK (command <> ''),
  timestamp_ns  INTEGER NOT NULL,
  duration_ns   INTEGER CHECK (duration_ns IS NULL OR duration_ns >= 0),
  exit_code     INTEGER,
  directory     TEXT,
  session       TEXT,
  hostname      TEXT,
  shell         TEXT
) STRICT;

CREATE INDEX entries_directory_timestamp
ON entries (directory, timestamp_ns DESC);

CREATE INDEX entries_hostname_timestamp
ON entries (hostname, timestamp_ns DESC);

CREATE INDEX entries_session_timestamp
ON entries (session, timestamp_ns DESC);

CREATE INDEX entries_timestamp
ON entries (timestamp_ns DESC, id DESC);

CREATE TABLE import_sources (
  id          TEXT PRIMARY KEY NOT NULL,
  format      TEXT NOT NULL CHECK (format <> ''),
  path        BLOB NOT NULL CHECK (LENGTH(path) > 0),
  generation  INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  UNIQUE (format, path)
) STRICT;

CREATE TABLE recency (
  command       TEXT PRIMARY KEY NOT NULL CHECK (command <> ''),
  timestamp_ns  INTEGER NOT NULL,
  entry_id      TEXT NOT NULL,
  exit_code     INTEGER,
  directory     TEXT
) STRICT;

CREATE INDEX recency_timestamp
ON recency (
  timestamp_ns DESC,
  entry_id DESC,
  command,
  exit_code,
  directory
);

CREATE TABLE source_records (
  source_id     TEXT NOT NULL,
  position      INTEGER NOT NULL CHECK (position >= 0),
  fingerprint   BLOB NOT NULL,
  entry_id      TEXT NOT NULL,
  PRIMARY KEY (source_id, position),
  FOREIGN KEY (source_id) REFERENCES import_sources(id) ON DELETE CASCADE,
  FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE
) STRICT;
