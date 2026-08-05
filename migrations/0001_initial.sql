CREATE TABLE commands (
  text          TEXT PRIMARY KEY NOT NULL CHECK (text <> ''),
  timestamp_ns  INTEGER NOT NULL,
  execution_id  TEXT NOT NULL,
  exit_code     INTEGER,
  directory     TEXT,
  FOREIGN KEY (execution_id) REFERENCES executions(id) ON DELETE CASCADE
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

CREATE INDEX executions_command_timestamp
ON executions (command, timestamp_ns DESC, id DESC);

CREATE TRIGGER executions_insert_command
AFTER INSERT ON executions
BEGIN
  INSERT INTO commands (
    text,
    timestamp_ns,
    execution_id,
    exit_code,
    directory
  ) VALUES (
    NEW.command,
    NEW.timestamp_ns,
    NEW.id,
    NEW.exit_code,
    NEW.directory
  )
  ON CONFLICT (text) DO UPDATE SET
    timestamp_ns = excluded.timestamp_ns,
    execution_id = excluded.execution_id,
    exit_code = excluded.exit_code,
    directory = excluded.directory
  WHERE excluded.timestamp_ns > commands.timestamp_ns
    OR (
      excluded.timestamp_ns = commands.timestamp_ns
      AND excluded.execution_id > commands.execution_id
    );
END;

CREATE TRIGGER executions_update_command
AFTER UPDATE OF command, timestamp_ns, exit_code, directory ON executions
WHEN OLD.command IS NOT NEW.command
  OR OLD.timestamp_ns IS NOT NEW.timestamp_ns
  OR OLD.exit_code IS NOT NEW.exit_code
  OR OLD.directory IS NOT NEW.directory
BEGIN
  DELETE FROM commands
  WHERE execution_id = OLD.id;

  INSERT INTO commands (
    text,
    timestamp_ns,
    execution_id,
    exit_code,
    directory
  )
  SELECT command, timestamp_ns, id, exit_code, directory
  FROM executions
  WHERE command = OLD.command
  ORDER BY timestamp_ns DESC, id DESC
  LIMIT 1
  ON CONFLICT (text) DO UPDATE SET
    timestamp_ns = excluded.timestamp_ns,
    execution_id = excluded.execution_id,
    exit_code = excluded.exit_code,
    directory = excluded.directory
  WHERE excluded.timestamp_ns > commands.timestamp_ns
    OR (
      excluded.timestamp_ns = commands.timestamp_ns
      AND excluded.execution_id > commands.execution_id
    );

  INSERT INTO commands (
    text,
    timestamp_ns,
    execution_id,
    exit_code,
    directory
  ) VALUES (
    NEW.command,
    NEW.timestamp_ns,
    NEW.id,
    NEW.exit_code,
    NEW.directory
  )
  ON CONFLICT (text) DO UPDATE SET
    timestamp_ns = excluded.timestamp_ns,
    execution_id = excluded.execution_id,
    exit_code = excluded.exit_code,
    directory = excluded.directory
  WHERE excluded.timestamp_ns > commands.timestamp_ns
    OR (
      excluded.timestamp_ns = commands.timestamp_ns
      AND excluded.execution_id > commands.execution_id
    );
END;

CREATE TRIGGER executions_delete_command
BEFORE DELETE ON executions
WHEN EXISTS (
  SELECT 1
  FROM commands
  WHERE execution_id = OLD.id
)
BEGIN
  DELETE FROM commands
  WHERE execution_id = OLD.id;

  INSERT INTO commands (
    text,
    timestamp_ns,
    execution_id,
    exit_code,
    directory
  )
  SELECT command, timestamp_ns, id, exit_code, directory
  FROM executions
  WHERE command = OLD.command
    AND id != OLD.id
  ORDER BY timestamp_ns DESC, id DESC
  LIMIT 1;
END;

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
