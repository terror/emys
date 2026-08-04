DROP INDEX executions_timestamp;

CREATE INDEX executions_timestamp
ON executions (timestamp_ns DESC, id DESC);
