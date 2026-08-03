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

pub(crate) struct Database {
  connection: Connection,
}

impl Database {
  pub(crate) fn backup(&self, path: impl AsRef<Path>) -> Result {
    self.backup_inner(path.as_ref(), false)
  }

  fn backup_inner(&self, path: &Path, force: bool) -> Result {
    let parent = path
      .parent()
      .filter(|parent| !parent.as_os_str().is_empty())
      .unwrap_or_else(|| Path::new("."));

    fs::create_dir_all(parent)?;

    if !force && path.try_exists()? {
      bail!(
        "backup `{}` already exists; use --force to overwrite it",
        path.display(),
      );
    }

    let temporary = parent.join(format!(".emys-backup-{}.tmp", Uuid::new_v4()));

    let result = (|| {
      let mut options = fs::OpenOptions::new();
      options.write(true).create_new(true);

      #[cfg(unix)]
      options.mode(0o600);

      drop(options.open(&temporary)?);

      self
        .connection
        .backup(MAIN_DB, &temporary, None)
        .with_context(|| {
          format!("failed to back up database to `{}`", path.display())
        })?;

      #[cfg(unix)]
      fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;

      if !force && path.try_exists()? {
        bail!(
          "backup `{}` already exists; use --force to overwrite it",
          path.display(),
        );
      }

      #[cfg(windows)]
      if force && path.try_exists()? {
        fs::remove_file(path)?;
      }

      fs::rename(&temporary, path)?;

      Ok(())
    })();

    if result.is_err() {
      let _ = fs::remove_file(temporary);
    }

    result
  }

  #[cfg(test)]
  pub(crate) fn connection(&self) -> &Connection {
    &self.connection
  }

  pub(crate) fn force_backup(&self, path: impl AsRef<Path>) -> Result {
    self.backup_inner(path.as_ref(), true)
  }

  pub(crate) fn import(&self, records: &[(Uuid, Execution)]) -> Result<usize> {
    let transaction = self.connection.unchecked_transaction()?;

    let inserted = {
      let mut statement = transaction.prepare(
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
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(id) DO NOTHING",
      )?;

      let mut inserted = 0;

      for (id, execution) in records {
        let directory = execution
          .directory
          .as_ref()
          .map(|directory| {
            directory
              .to_str()
              .context("execution directory is not valid UTF-8")
          })
          .transpose()?;

        inserted += statement.execute(params![
          id.to_string(),
          execution.command,
          execution.timestamp_ns,
          execution.duration_ns,
          execution.exit_code,
          directory,
          execution.session,
          execution.hostname,
          execution.shell,
        ])?;
      }

      inserted
    };

    transaction.commit()?;

    Ok(inserted)
  }

  pub(crate) fn insert(&self, execution: &Execution) -> Result<Uuid> {
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

  pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self> {
    Self::try_from(Connection::open(path)?)
  }

  pub(crate) fn open_default() -> Result<Self> {
    #[cfg(unix)]
    let path =
      BaseDirectories::with_prefix("emys").place_data_file("history.db")?;

    #[cfg(windows)]
    let path = {
      let directory = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .context("failed to determine local data directory")?
        .join("emys");

      fs::create_dir_all(&directory)?;

      directory.join("history.db")
    };

    Self::open_file(&path)
  }

  fn open_file(path: &Path) -> Result<Self> {
    #[cfg(unix)]
    let directory = path
      .parent()
      .context("database path has no parent directory")?;

    #[cfg(unix)]
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;

    let database = Self::open(path).with_context(|| {
      format!("failed to open database `{}`", path.display())
    })?;

    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;

    Ok(database)
  }

  pub(crate) fn recent(&self, limit: usize) -> Result<Vec<(Uuid, Execution)>> {
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

  pub(crate) fn search(
    &self,
    query: &str,
    limit: usize,
  ) -> Result<Vec<(Uuid, Execution)>> {
    let limit = i64::try_from(limit)
      .context("execution limit exceeds SQLite integer range")?;

    let mut statement = self.connection.prepare(
      "WITH matches AS (
        SELECT
          id,
          command,
          timestamp_ns,
          duration_ns,
          exit_code,
          directory,
          session,
          hostname,
          shell,
          ROW_NUMBER() OVER (
            PARTITION BY command
            ORDER BY timestamp_ns DESC, id DESC
          ) AS command_rank
        FROM executions
        WHERE INSTR(LOWER(command), LOWER(?1)) > 0
      )
      SELECT
        id,
        command,
        timestamp_ns,
        duration_ns,
        exit_code,
        directory,
        session,
        hostname,
        shell
      FROM matches
      WHERE command_rank = 1
      ORDER BY timestamp_ns DESC, id DESC
      LIMIT ?2",
    )?;

    let rows = statement.query_map(params![query, limit], |row| {
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
}

impl TryFrom<Connection> for Database {
  type Error = Error;

  fn try_from(connection: Connection) -> Result<Self> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "journal_mode", "WAL")?;

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
  fn backup_copies_executions_while_source_remains_open() {
    let root = tempfile::tempdir().unwrap();

    let (source, destination) = (
      root.path().join("foo/source.db"),
      root.path().join("bar/backup.db"),
    );

    fs::create_dir_all(source.parent().unwrap()).unwrap();

    let database = Database::open(&source).unwrap();

    let first = database
      .insert(&Execution {
        command: "foo".into(),
        timestamp_ns: 1,
        ..Default::default()
      })
      .unwrap();

    let second = database
      .insert(&Execution {
        command: "bar".into(),
        timestamp_ns: 2,
        ..Default::default()
      })
      .unwrap();

    database.backup(&destination).unwrap();

    database
      .insert(&Execution {
        command: "baz".into(),
        timestamp_ns: 3,
        ..Default::default()
      })
      .unwrap();

    let backup = Database::open(&destination).unwrap();

    assert_eq!(
      (
        backup
          .connection()
          .query_row("PRAGMA integrity_check", [], |row| row
            .get::<_, String>(0))
          .unwrap(),
        backup.recent(20).unwrap(),
      ),
      (
        "ok".into(),
        vec![
          (
            second,
            Execution {
              command: "bar".into(),
              timestamp_ns: 2,
              ..Default::default()
            },
          ),
          (
            first,
            Execution {
              command: "foo".into(),
              timestamp_ns: 1,
              ..Default::default()
            },
          ),
        ],
      ),
    );

    #[cfg(unix)]
    assert_eq!(
      fs::metadata(destination).unwrap().permissions().mode() & 0o777,
      0o600,
    );
  }

  #[test]
  fn backup_refuses_existing_destination_unless_forced() {
    let root = tempfile::tempdir().unwrap();

    let (source, destination) =
      (root.path().join("source.db"), root.path().join("backup.db"));

    let database = Database::open(source).unwrap();

    database
      .insert(&Execution {
        command: "foo".into(),
        ..Default::default()
      })
      .unwrap();

    database.backup(&destination).unwrap();

    database
      .insert(&Execution {
        command: "bar".into(),
        ..Default::default()
      })
      .unwrap();

    assert_eq!(
      database.backup(&destination).unwrap_err().to_string(),
      format!(
        "backup `{}` already exists; use --force to overwrite it",
        destination.display(),
      ),
    );

    database.force_backup(&destination).unwrap();

    assert_eq!(
      Database::open(destination)
        .unwrap()
        .recent(20)
        .unwrap()
        .len(),
      2
    );
  }

  #[test]
  fn import_inserts_and_is_idempotent() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let records = vec![
      (
        Uuid::from_u128(1),
        Execution {
          command: "foo".into(),
          timestamp_ns: 1,
          ..Default::default()
        },
      ),
      (
        Uuid::from_u128(2),
        Execution {
          command: "bar".into(),
          timestamp_ns: 2,
          ..Default::default()
        },
      ),
    ];

    assert_eq!(
      (
        database.import(&records).unwrap(),
        database.import(&records).unwrap(),
        database.recent(20).unwrap(),
      ),
      (2, 0, records.into_iter().rev().collect()),
    );
  }

  #[test]
  fn import_preserves_repeated_commands_and_metadata() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let first = (
      Uuid::from_u128(1),
      Execution {
        command: "foo".into(),
        timestamp_ns: 1,
        duration_ns: Some(2),
        exit_code: Some(3),
        directory: Some("/foo".into()),
        session: Some("bar".into()),
        hostname: Some("foo".into()),
        shell: Some("zsh".into()),
      },
    );

    let second = (
      Uuid::from_u128(2),
      Execution {
        command: "foo".into(),
        timestamp_ns: 4,
        duration_ns: Some(5),
        exit_code: Some(6),
        directory: Some("/bar".into()),
        session: Some("foo".into()),
        hostname: Some("bar".into()),
        shell: Some("zsh".into()),
      },
    );

    assert_eq!(
      database.import(&[first.clone(), second.clone()]).unwrap(),
      2
    );

    assert_eq!(
      (
        database.recent(20).unwrap(),
        database.search("foo", 20).unwrap()
      ),
      (vec![second.clone(), first], vec![second]),
    );
  }

  #[test]
  fn import_rolls_back_complete_batch_on_constraint_failure() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let error = database
      .import(&[
        (
          Uuid::from_u128(1),
          Execution {
            command: "foo".into(),
            ..Default::default()
          },
        ),
        (
          Uuid::from_u128(2),
          Execution {
            command: "bar".into(),
            duration_ns: Some(-1),
            ..Default::default()
          },
        ),
      ])
      .unwrap_err();

    let error = error.downcast_ref::<rusqlite::Error>().unwrap();

    assert_eq!(
      (
        error.sqlite_error_code(),
        error.to_string(),
        database.recent(20).unwrap(),
      ),
      (
        Some(rusqlite::ffi::ErrorCode::ConstraintViolation),
        "CHECK constraint failed: duration_ns IS NULL OR duration_ns >= 0"
          .into(),
        Vec::new(),
      ),
    );
  }

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
  fn open_file_creates_private_database() {
    let root = tempfile::tempdir().unwrap();

    let path = root.path().join("foo/history.db");

    fs::create_dir_all(path.parent().unwrap()).unwrap();

    let database = Database::open_file(&path).unwrap();

    assert_eq!(
      (
        path.is_file(),
        database
          .connection()
          .pragma_query_value(None, "journal_mode", |row| {
            row.get::<_, String>(0)
          })
          .unwrap(),
        database
          .connection()
          .pragma_query_value(None, "busy_timeout", |row| {
            row.get::<_, i64>(0)
          })
          .unwrap(),
      ),
      (true, "wal".into(), 5000),
    );

    #[cfg(unix)]
    assert_eq!(
      (
        fs::metadata(path.parent().unwrap())
          .unwrap()
          .permissions()
          .mode()
          & 0o777,
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
      ),
      (0o700, 0o600),
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

    let Err(error) = database.recent(usize::MAX) else {
      panic!("expected large limit to fail")
    };

    assert_eq!(
      error.to_string(),
      "execution limit exceeds SQLite integer range",
    );
  }

  #[test]
  fn search_filters_orders_and_collapses_executions() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    database
      .insert(&Execution {
        command: "git status".into(),
        timestamp_ns: 1,
        exit_code: Some(1),
        ..Default::default()
      })
      .unwrap();

    let uppercase = database
      .insert(&Execution {
        command: "GIT log".into(),
        timestamp_ns: 2,
        ..Default::default()
      })
      .unwrap();

    let newest = database
      .insert(&Execution {
        command: "git status".into(),
        timestamp_ns: 3,
        exit_code: Some(0),
        directory: Some("/foo".into()),
        ..Default::default()
      })
      .unwrap();

    database
      .insert(&Execution {
        command: "cargo test".into(),
        timestamp_ns: 4,
        ..Default::default()
      })
      .unwrap();

    let literal = database
      .insert(&Execution {
        command: "echo %".into(),
        timestamp_ns: 5,
        ..Default::default()
      })
      .unwrap();

    assert_eq!(
      database.search("gIt", 20).unwrap(),
      vec![
        (
          newest,
          Execution {
            command: "git status".into(),
            timestamp_ns: 3,
            exit_code: Some(0),
            directory: Some("/foo".into()),
            ..Default::default()
          },
        ),
        (
          uppercase,
          Execution {
            command: "GIT log".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
        ),
      ],
    );

    assert_eq!(database.search("git", 0).unwrap(), Vec::new());

    assert_eq!(
      database.search("%", 20).unwrap(),
      vec![(
        literal,
        Execution {
          command: "echo %".into(),
          timestamp_ns: 5,
          ..Default::default()
        },
      )],
    );

    assert_eq!(
      database
        .search("", 1)
        .unwrap()
        .into_iter()
        .map(|(_, execution)| execution.command)
        .collect::<Vec<_>>(),
      vec!["echo %"],
    );
  }

  #[cfg(target_pointer_width = "64")]
  #[test]
  fn search_rejects_large_limit() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let Err(error) = database.search("foo", usize::MAX) else {
      panic!("expected large limit to fail")
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

    let Err(error) = Database::try_from(connection) else {
      panic!("expected unsupported schema to fail")
    };

    assert_eq!(
      error.to_string(),
      "database schema version 2 is unsupported; expected 1",
    );
  }
}
