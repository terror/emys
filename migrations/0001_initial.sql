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

CREATE INDEX executions_timestamp
ON executions (timestamp_ns DESC, id DESC);

CREATE INDEX executions_directory_timestamp
ON executions (directory, timestamp_ns DESC);

CREATE INDEX executions_session_timestamp
ON executions (session, timestamp_ns DESC);

CREATE INDEX executions_hostname_timestamp
ON executions (hostname, timestamp_ns DESC);
