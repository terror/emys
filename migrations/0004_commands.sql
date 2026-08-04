CREATE TABLE commands (
  command       TEXT PRIMARY KEY NOT NULL CHECK (command <> ''),
  timestamp_ns  INTEGER NOT NULL,
  execution_id  TEXT NOT NULL
) STRICT;

CREATE INDEX commands_timestamp
ON commands (timestamp_ns DESC, execution_id DESC, command);

INSERT OR IGNORE INTO commands (command, timestamp_ns, execution_id)
SELECT command, timestamp_ns, id
FROM executions
ORDER BY timestamp_ns DESC, id DESC;
