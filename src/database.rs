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

  pub fn recent(&self, limit: usize) -> Result<Vec<(Uuid, Execution)>> {
    let limit = i64::try_from(limit)
      .context("execution limit exceeds SQLite integer range")?;

    let mut statement = self.connection.prepare(
      "SELECT
        id,
        command,
        timestamp_ns,
        duration_ns,
        exit_code,
        directory,
        session,
        hostname,
        shell
      FROM executions
      ORDER BY timestamp_ns DESC, id DESC
      LIMIT ?1",
    )?;

    let rows = statement.query_map([limit], |row| {
      Ok((
        row.get::<_, String>(0)?,
        Execution {
          command: row.get(1)?,
          timestamp_ns: row.get(2)?,
          duration_ns: row.get(3)?,
          exit_code: row.get(4)?,
          directory: row.get::<_, Option<String>>(5)?.map(PathBuf::from),
          session: row.get(6)?,
          hostname: row.get(7)?,
          shell: row.get(8)?,
        },
      ))
    })?;

    rows
      .map(|row| {
        let (id, execution) = row?;

        let id = Uuid::parse_str(&id)
          .with_context(|| format!("invalid execution ID `{id}`"))?;

        Ok((id, execution))
      })
      .collect()
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

    let mut expected = vec![(first, execution.clone()), (second, execution)];

    expected.sort_by(|(left, _), (right, _)| right.cmp(left));

    assert_eq!(database.recent(2).unwrap(), expected);
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
      (error.sqlite_error_code(), error.to_string()),
      (
        Some(rusqlite::ffi::ErrorCode::ConstraintViolation),
        "CHECK constraint failed: duration_ns IS NULL OR duration_ns >= 0"
          .into(),
      ),
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
  fn recent_orders_and_limits_executions() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    database
      .insert(&Execution {
        command: "foo".into(),
        timestamp_ns: 1,
        ..Default::default()
      })
      .unwrap();

    let id = database
      .insert(&Execution {
        command: "bar".into(),
        timestamp_ns: 2,
        ..Default::default()
      })
      .unwrap();

    assert_eq!(
      database.recent(1).unwrap(),
      vec![(
        id,
        Execution {
          command: "bar".into(),
          timestamp_ns: 2,
          ..Default::default()
        },
      )],
    );

    assert_eq!(database.recent(0).unwrap(), Vec::new());
  }

  #[cfg(target_pointer_width = "64")]
  #[test]
  fn recent_rejects_large_limit() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let error = match database.recent(usize::MAX) {
      Ok(_) => panic!("expected large limit to fail"),
      Err(error) => error,
    };

    assert_eq!(
      error.to_string(),
      "execution limit exceeds SQLite integer range",
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
