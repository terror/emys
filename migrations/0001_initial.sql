CREATE TABLE commands (
  text          TEXT PRIMARY KEY NOT NULL CHECK (text <> ''),
  timestamp_ns  INTEGER NOT NULL,
  execution_id  TEXT NOT NULL,
  exit_code     INTEGER,
  directory     TEXT
) STRICT;

CREATE INDEX commands_timestamp
ON commands (
  timestamp_ns DESC,
  execution_id DESC,
  text,
  exit_code,
  directory
);

CREATE TABLE executions (
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

CREATE INDEX executions_directory_timestamp
ON executions (directory, timestamp_ns DESC);

CREATE INDEX executions_hostname_timestamp
ON executions (hostname, timestamp_ns DESC);

CREATE INDEX executions_session_timestamp
ON executions (session, timestamp_ns DESC);

CREATE INDEX executions_timestamp
ON executions (timestamp_ns DESC, id DESC);

CREATE TABLE import_sources (
  id          TEXT PRIMARY KEY NOT NULL,
  format      TEXT NOT NULL CHECK (format <> ''),
  path        BLOB NOT NULL CHECK (LENGTH(path) > 0),
  generation  INTEGER NOT NULL DEFAULT 0 CHECK (generation >= 0),
  UNIQUE (format, path)
) STRICT;

CREATE TABLE source_records (
  source_id      TEXT NOT NULL,
  position       INTEGER NOT NULL CHECK (position >= 0),
  fingerprint    BLOB NOT NULL,
  execution_id   TEXT NOT NULL,
  PRIMARY KEY (source_id, position),
  FOREIGN KEY (source_id) REFERENCES import_sources(id) ON DELETE CASCADE,
  FOREIGN KEY (execution_id) REFERENCES executions(id) ON DELETE CASCADE
) STRICT;
