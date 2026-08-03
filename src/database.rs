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

  pub fn insert(&self, execution: &Execution) -> Result<Uuid> {
    let id = Uuid::new_v4();

    let directory = execution
      .directory
      .as_ref()
      .map(|directory| {
        directory
          .to_str()
          .context("execution directory is not valid UTF-8")
      })
      .transpose()?;

    self.connection.execute(
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
      params![
        id.to_string(),
        execution.command,
        execution.timestamp_ns,
        execution.duration_ns,
        execution.exit_code,
        directory,
        execution.session,
        execution.hostname,
        execution.shell,
      ],
    )?;

    Ok(id)
  }

  pub fn connection(&self) -> &Connection {
    &self.connection
  }
}

impl TryFrom<Connection> for Database {
  type Error = anyhow::Error;

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
  fn insert_stores_every_execution() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let execution = Execution {
      command: "foo".into(),
      timestamp_ns: 1,
      duration_ns: Some(2),
      exit_code: Some(0),
      directory: Some("/foo".into()),
      session: Some("bar".into()),
      hostname: Some("foo".into()),
      shell: Some("bar".into()),
    };

    let (first, second) = (
      database.insert(&execution).unwrap(),
      database.insert(&execution).unwrap(),
    );

    assert_ne!(first, second);

    assert_eq!(
      database
        .connection()
        .query_row(
          "SELECT
            command,
            timestamp_ns,
            duration_ns,
            exit_code,
            directory,
            session,
            hostname,
            shell
          FROM executions
          WHERE id = ?1",
          [first.to_string()],
          |row| {
            Ok((
              row.get::<_, String>(0)?,
              row.get::<_, i64>(1)?,
              row.get::<_, Option<i64>>(2)?,
              row.get::<_, Option<i32>>(3)?,
              row.get::<_, Option<String>>(4)?,
              row.get::<_, Option<String>>(5)?,
              row.get::<_, Option<String>>(6)?,
              row.get::<_, Option<String>>(7)?,
            ))
          },
        )
        .unwrap(),
      (
        "foo".into(),
        1,
        Some(2),
        Some(0),
        Some("/foo".into()),
        Some("bar".into()),
        Some("foo".into()),
        Some("bar".into()),
      ),
    );

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
  fn negative_duration_is_rejected() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let execution = Execution {
      command: "foo".into(),
      duration_ns: Some(-1),
      ..Default::default()
    };

    let error = database.insert(&execution).unwrap_err();

    let error = error.downcast_ref::<rusqlite::Error>().unwrap();

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
