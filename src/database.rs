use super::*;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
  BEGIN;

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
  ON executions (timestamp_ns DESC);

  CREATE INDEX executions_directory_timestamp
  ON executions (directory, timestamp_ns DESC);

  CREATE INDEX executions_session_timestamp
  ON executions (session, timestamp_ns DESC);

  CREATE INDEX executions_hostname_timestamp
  ON executions (hostname, timestamp_ns DESC);

  PRAGMA user_version = 1;

  COMMIT;
";

pub struct Database {
  connection: Connection,
}

impl Database {
  pub fn open(path: impl AsRef<Path>) -> Result<Self> {
    Self::try_from(Connection::open(path)?)
  }

  pub fn connection(&self) -> &Connection {
    &self.connection
  }
}

impl TryFrom<Connection> for Database {
  type Error = Error;

  fn try_from(connection: Connection) -> Result<Self> {
    match connection.query_row("PRAGMA user_version", [], |row| row.get(0))? {
      0 => connection.execute_batch(SCHEMA)?,
      SCHEMA_VERSION => {}
      version => bail!(
        "database schema version {version} is unsupported; expected \
         {SCHEMA_VERSION}"
      ),
    }

    Ok(Self { connection })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn negative_duration_is_rejected() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let error = database
      .connection()
      .execute(
        "INSERT INTO executions (
          id,
          command,
          timestamp_ns,
          duration_ns
        ) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params!["foo", "bar", 1, -1],
      )
      .unwrap_err();

    assert_eq!(
      error.sqlite_error_code(),
      Some(rusqlite::ffi::ErrorCode::ConstraintViolation),
    );

    assert_eq!(
      error.to_string(),
      "CHECK constraint failed: duration_ns IS NULL OR duration_ns >= 0",
    );
  }

  #[test]
  fn open_creates_schema() {
    let database = Database::open(":memory:").unwrap();

    let database = Database::try_from(database.connection).unwrap();

    assert_eq!(
      database
        .connection()
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .unwrap(),
      SCHEMA_VERSION,
    );

    database
      .connection()
      .execute(
        "INSERT INTO executions (
          id,
          command,
          timestamp_ns,
          duration_ns,
          exit_code,
          directory,
          session,
          hostname,
          shell
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
          "foo", "echo foo", 1, 2, 0, "/foo", "bar", "baz", "zsh",
        ],
      )
      .unwrap();

    database
      .connection()
      .execute(
        "INSERT INTO executions (id, command, timestamp_ns)
         VALUES (?1, ?2, ?3)",
        rusqlite::params!["bar", "echo foo", 3],
      )
      .unwrap();

    assert_eq!(
      database
        .connection()
        .query_row("SELECT COUNT(*) FROM executions", [], |row| {
          row.get::<_, i64>(0)
        })
        .unwrap(),
      2,
    );
  }

  #[test]
  fn unsupported_schema_is_rejected() {
    let connection = Connection::open_in_memory().unwrap();

    connection.execute_batch("PRAGMA user_version = 2").unwrap();

    let error = match Database::try_from(connection) {
      Ok(_) => panic!("expected unsupported schema to fail"),
      Err(error) => error,
    };

    assert_eq!(
      error.to_string(),
      "database schema version 2 is unsupported; expected 1",
    );
  }
}
