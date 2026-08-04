use super::*;

pub(crate) struct Database {
  connection: Connection,
}

impl Database {
  const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_execution_timestamp_id.sql"),
    include_str!("../migrations/0003_import_sources.sql"),
    include_str!("../migrations/0004_commands.sql"),
  ];

  const SCHEMA_VERSION: usize = Self::MIGRATIONS.len();

  pub(crate) fn backup(&self, path: &Path, force: bool) -> Result {
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

    let temporary = parent.join(format!(".honu-backup-{}.tmp", Uuid::new_v4()));

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

  pub(crate) fn clear(&self) -> Result {
    let transaction = self.connection.unchecked_transaction()?;

    transaction.execute("DELETE FROM commands", [])?;
    transaction.execute("DELETE FROM import_sources", [])?;
    transaction.execute("DELETE FROM executions", [])?;

    transaction.commit()?;

    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn connection(&self) -> &Connection {
    &self.connection
  }

  #[cfg(unix)]
  pub(crate) fn for_each_command(
    &self,
    mut callback: impl FnMut(String) -> bool,
  ) -> Result {
    let mut statement = self.connection.prepare(
      "SELECT command
       FROM commands
       ORDER BY timestamp_ns DESC, execution_id DESC",
    )?;

    let mut rows = statement.query([])?;

    while let Some(row) = rows.next()? {
      let command = row.get::<_, String>(0)?;

      if !callback(command) {
        break;
      }
    }

    Ok(())
  }

  #[cfg(unix)]
  pub(crate) fn has_executions(&self) -> Result<bool> {
    self
      .connection
      .query_row("SELECT EXISTS(SELECT 1 FROM executions)", [], |row| {
        row.get(0)
      })
      .map_err(Into::into)
  }

  pub(crate) fn import(
    &self,
    format: &str,
    path: &Path,
    records: impl IntoIterator<Item = Result<Record>>,
    mut progress: impl FnMut(ProgressEntry),
  ) -> Result<usize> {
    let path = path.as_os_str().as_encoded_bytes();

    let (source_id, generation) = self.reserve_source(format, path)?;

    let records = records.into_iter().collect::<Result<Vec<_>>>()?;

    i32::try_from(records.len())
      .context("history contains too many records")?;

    let transaction = Transaction::new_unchecked(
      &self.connection,
      TransactionBehavior::Immediate,
    )?;

    let current_generation = transaction.query_row(
      "SELECT generation FROM import_sources WHERE id = ?1",
      [&source_id],
      |row| row.get::<_, i64>(0),
    )?;

    if current_generation != generation {
      return Ok(0);
    }

    let previous = {
      let mut statement = transaction.prepare(
        "SELECT fingerprint, execution_id
         FROM source_records
         WHERE source_id = ?1
         ORDER BY position",
      )?;

      statement
        .query_map([&source_id], |row| {
          Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut identifiers = Self::reconcile(&previous, &records)?;

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
        ON CONFLICT (id) DO UPDATE SET
          command = excluded.command,
          timestamp_ns = excluded.timestamp_ns,
          duration_ns = excluded.duration_ns,
          exit_code = excluded.exit_code,
          directory = excluded.directory,
          session = excluded.session,
          hostname = excluded.hostname,
          shell = excluded.shell
        WHERE ?10 = 0",
      )?;

      let mut inserted = 0;

      for (index, (record, identifier)) in
        records.iter().zip(&mut identifiers).enumerate()
      {
        let new = identifier.is_none();

        let id = identifier.get_or_insert_with(|| Uuid::new_v4().to_string());

        let directory = record.execution.directory()?;

        let changed = statement.execute(params![
          id.as_str(),
          record.execution.command,
          record.execution.timestamp_ns,
          record.execution.duration_ns,
          record.execution.exit_code,
          directory,
          record.execution.session,
          record.execution.hostname,
          record.execution.shell,
          new,
        ])?;

        if new && changed == 0 {
          bail!("generated duplicate execution ID `{id}`");
        }

        inserted += usize::from(new);

        progress(ProgressEntry {
          inserted,
          processed: index + 1,
        });
      }

      inserted
    };

    transaction.execute(
      "DELETE FROM source_records WHERE source_id = ?1",
      [&source_id],
    )?;

    {
      let mut statement = transaction.prepare(
        "INSERT INTO source_records (
           source_id,
           position,
           fingerprint,
           execution_id
         ) VALUES (?1, ?2, ?3, ?4)",
      )?;

      for (position, (record, identifier)) in
        records.iter().zip(identifiers).enumerate()
      {
        statement.execute(params![
          source_id,
          i64::try_from(position)?,
          record.fingerprint,
          identifier.unwrap(),
        ])?;
      }
    }

    transaction.execute("DELETE FROM commands", [])?;

    transaction.execute(
      "INSERT OR IGNORE INTO commands (command, timestamp_ns, execution_id)
       SELECT command, timestamp_ns, id
       FROM executions
       ORDER BY timestamp_ns DESC, id DESC",
      [],
    )?;

    transaction.commit()?;

    Ok(inserted)
  }

  pub(crate) fn insert(&self, execution: &Execution) -> Result<Uuid> {
    let id = Uuid::new_v4();

    let directory = execution.directory()?;

    let transaction = Transaction::new_unchecked(
      &self.connection,
      TransactionBehavior::Immediate,
    )?;

    transaction.execute(
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

    transaction.execute(
      "INSERT INTO commands (command, timestamp_ns, execution_id)
       VALUES (?1, ?2, ?3)
       ON CONFLICT (command) DO UPDATE SET
         timestamp_ns = excluded.timestamp_ns,
         execution_id = excluded.execution_id
       WHERE excluded.timestamp_ns > commands.timestamp_ns
         OR (
           excluded.timestamp_ns = commands.timestamp_ns
           AND excluded.execution_id > commands.execution_id
         )",
      params![execution.command, execution.timestamp_ns, id.to_string()],
    )?;

    transaction.commit()?;

    Ok(id)
  }

  pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self> {
    Self::try_from(Connection::open(path)?)
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

  fn reconcile(
    previous: &[(Vec<u8>, String)],
    records: &[Record],
  ) -> Result<Vec<Option<String>>> {
    let previous_len = i32::try_from(previous.len())
      .context("previous history contains too many records")?
      .cast_unsigned();

    let records_len = u32::try_from(records.len())
      .context("current history contains too many records")?;

    let mut input = InternedInput::default();

    input.reserve(previous_len, records_len);

    input.update_before(
      previous.iter().map(|(fingerprint, _)| fingerprint).cloned(),
    );

    input
      .update_after(records.iter().map(|record| &record.fingerprint).cloned());

    let diff = Diff::compute(Algorithm::Myers, &input);

    let identifiers = (0..records_len)
      .scan(0_u32, |before, after| {
        while *before < previous_len && diff.is_removed(*before) {
          *before += 1;
        }

        let identifier = if diff.is_added(after) {
          None
        } else {
          let identifier = previous[*before as usize].1.clone();
          *before += 1;
          Some(identifier)
        };

        Some(identifier)
      })
      .collect();

    Ok(identifiers)
  }

  fn reserve_source(&self, format: &str, path: &[u8]) -> Result<(String, i64)> {
    let transaction = Transaction::new_unchecked(
      &self.connection,
      TransactionBehavior::Immediate,
    )?;

    let source_id = Uuid::new_v4().to_string();

    transaction.execute(
      "INSERT INTO import_sources (id, format, path)
       VALUES (?1, ?2, ?3)
       ON CONFLICT (format, path) DO NOTHING",
      params![source_id, format, path],
    )?;

    transaction.execute(
      "UPDATE import_sources
       SET generation = generation + 1
       WHERE format = ?1 AND path = ?2",
      params![format, path],
    )?;

    let source = transaction.query_row(
      "SELECT id, generation
       FROM import_sources
       WHERE format = ?1 AND path = ?2",
      params![format, path],
      |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )?;

    transaction.commit()?;

    Ok(source)
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

  fn try_from(mut connection: Connection) -> Result<Self> {
    connection.busy_timeout(Duration::from_secs(5))?;

    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", true)?;

    let transaction =
      connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let version: i64 =
      transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    let Ok(version) = usize::try_from(version) else {
      bail!(
        "database schema version {version} is unsupported; expected {}",
        Self::SCHEMA_VERSION,
      );
    };

    if version > Self::SCHEMA_VERSION {
      bail!(
        "database schema version {version} is unsupported; expected \
         {}",
        Self::SCHEMA_VERSION,
      );
    }

    for (version, migration) in
      Self::MIGRATIONS.iter().enumerate().skip(version)
    {
      let version = version + 1;

      transaction.execute_batch(migration).with_context(|| {
        format!("failed to apply database migration {version}")
      })?;

      transaction.pragma_update(
        None,
        "user_version",
        i64::try_from(version)?,
      )?;
    }

    transaction.commit()?;

    Ok(Self { connection })
  }
}

impl TryFrom<&Path> for Database {
  type Error = Error;

  fn try_from(path: &Path) -> Result<Self> {
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
}

#[cfg(test)]
mod tests {
  use {
    super::*,
    std::{collections::HashMap, iter},
  };

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

    database.backup(&destination, false).unwrap();

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

    database.backup(&destination, false).unwrap();

    database
      .insert(&Execution {
        command: "bar".into(),
        ..Default::default()
      })
      .unwrap();

    assert_eq!(
      database
        .backup(&destination, false)
        .unwrap_err()
        .to_string(),
      format!(
        "backup `{}` already exists; use --force to overwrite it",
        destination.display(),
      ),
    );

    database.backup(&destination, true).unwrap();

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
  fn clear_deletes_all_executions() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    database
      .insert(&Execution {
        command: "foo".into(),
        ..Default::default()
      })
      .unwrap();

    database
      .connection()
      .execute(
        "INSERT INTO import_sources (id, format, path) VALUES (?1, ?2, ?3)",
        params![Uuid::new_v4().to_string(), "foo", b"bar"],
      )
      .unwrap();

    database.clear().unwrap();

    assert_eq!(
      (
        database.recent(20).unwrap(),
        database
          .connection()
          .query_row("SELECT COUNT(*) FROM import_sources", [], |row| {
            row.get::<_, i64>(0)
          })
          .unwrap(),
        database
          .connection()
          .query_row("SELECT COUNT(*) FROM commands", [], |row| {
            row.get::<_, i64>(0)
          })
          .unwrap(),
      ),
      (Vec::new(), 0, 0),
    );

    database
      .insert(&Execution {
        command: "bar".into(),
        ..Default::default()
      })
      .unwrap();

    assert_eq!(database.recent(20).unwrap().len(), 1);
  }

  #[cfg(unix)]
  #[test]
  fn for_each_command_visits_every_unique_command_in_recent_order() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    for (command, timestamp_ns) in [("foo", 1), ("bar", 2), ("foo", 3)] {
      database
        .insert(&Execution {
          command: command.into(),
          timestamp_ns,
          ..Default::default()
        })
        .unwrap();
    }

    let mut commands = Vec::new();

    database
      .for_each_command(|command| {
        commands.push(command);
        true
      })
      .unwrap();

    assert_eq!(commands, ["foo", "bar"]);

    database
      .for_each_command(|command| {
        commands.push(command);
        false
      })
      .unwrap();

    assert_eq!(commands, ["foo", "bar", "foo"]);
  }

  #[test]
  fn import_inserts_and_is_idempotent() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let records = [
      Record {
        execution: Execution {
          command: "foo".into(),
          timestamp_ns: 1,
          ..Default::default()
        },
        fingerprint: b"foo".to_vec(),
      },
      Record {
        execution: Execution {
          command: "bar".into(),
          timestamp_ns: 2,
          ..Default::default()
        },
        fingerprint: b"bar".to_vec(),
      },
    ];

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          records.iter().cloned().map(Ok),
          |_| {},
        )
        .unwrap(),
      2,
    );

    let imported = database.recent(20).unwrap();

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          records.into_iter().map(Ok),
          |_| {},
        )
        .unwrap(),
      0,
    );
    assert_eq!(database.recent(20).unwrap(), imported);
  }

  #[test]
  fn import_preserves_repeated_commands_and_metadata() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let first = Execution {
      command: "foo".into(),
      timestamp_ns: 1,
      duration_ns: Some(2),
      exit_code: Some(3),
      directory: Some("/foo".into()),
      session: Some("bar".into()),
      hostname: Some("foo".into()),
      shell: Some("zsh".into()),
    };

    let second = Execution {
      command: "foo".into(),
      timestamp_ns: 4,
      duration_ns: Some(5),
      exit_code: Some(6),
      directory: Some("/bar".into()),
      session: Some("foo".into()),
      hostname: Some("bar".into()),
      shell: Some("zsh".into()),
    };

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          [
            Ok(Record {
              execution: first.clone(),
              fingerprint: b"foo".to_vec(),
            }),
            Ok(Record {
              execution: second.clone(),
              fingerprint: b"bar".to_vec(),
            }),
          ],
          |_| {},
        )
        .unwrap(),
      2,
    );

    assert_eq!(
      (
        database
          .recent(20)
          .unwrap()
          .into_iter()
          .map(|(_, execution)| execution)
          .collect::<Vec<_>>(),
        database
          .search("foo", 20)
          .unwrap()
          .into_iter()
          .map(|(_, execution)| execution)
          .collect::<Vec<_>>(),
      ),
      (vec![second.clone(), first], vec![second]),
    );
  }

  #[test]
  fn import_refreshes_command_recency() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    for records in [
      [("foo", 3, b"foo".as_slice()), ("bar", 2, b"bar".as_slice())],
      [("foo", 1, b"foo".as_slice()), ("bar", 2, b"bar".as_slice())],
    ] {
      database
        .import(
          "test",
          Path::new("foo"),
          records.map(|(command, timestamp_ns, fingerprint)| {
            Ok(Record {
              execution: Execution {
                command: command.into(),
                timestamp_ns,
                ..Default::default()
              },
              fingerprint: fingerprint.to_vec(),
            })
          }),
          |_| {},
        )
        .unwrap();
    }

    let commands = database
      .connection()
      .prepare(
        "SELECT command, timestamp_ns
         FROM commands
         ORDER BY timestamp_ns DESC, execution_id DESC",
      )
      .unwrap()
      .query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
      })
      .unwrap()
      .collect::<rusqlite::Result<Vec<_>>>()
      .unwrap();

    assert_eq!(commands, [("bar".into(), 2), ("foo".into(), 1)]);
  }

  #[test]
  fn import_reconciles_ordered_source_snapshots() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          [
            Ok(Record {
              execution: Execution {
                command: "foo".into(),
                timestamp_ns: 1,
                ..Default::default()
              },
              fingerprint: b"foo".to_vec(),
            }),
            Ok(Record {
              execution: Execution {
                command: "bar".into(),
                timestamp_ns: 2,
                ..Default::default()
              },
              fingerprint: b"bar".to_vec(),
            }),
          ],
          |_| {},
        )
        .unwrap(),
      2,
    );

    let original = database
      .recent(20)
      .unwrap()
      .into_iter()
      .map(|(id, execution)| (execution.command, (id, execution.timestamp_ns)))
      .collect::<HashMap<_, _>>();

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          [
            Ok(Record {
              execution: Execution {
                command: "baz".into(),
                timestamp_ns: 1,
                ..Default::default()
              },
              fingerprint: b"baz".to_vec(),
            }),
            Ok(Record {
              execution: Execution {
                command: "foo".into(),
                timestamp_ns: 2,
                ..Default::default()
              },
              fingerprint: b"foo".to_vec(),
            }),
            Ok(Record {
              execution: Execution {
                command: "bar".into(),
                timestamp_ns: 3,
                ..Default::default()
              },
              fingerprint: b"bar".to_vec(),
            }),
          ],
          |_| {},
        )
        .unwrap(),
      1,
    );

    let reconciled = database
      .recent(20)
      .unwrap()
      .into_iter()
      .map(|(id, execution)| (execution.command, (id, execution.timestamp_ns)))
      .collect::<HashMap<_, _>>();

    assert_eq!(
      (
        reconciled["foo"].0,
        reconciled["bar"].0,
        reconciled["foo"].1,
        reconciled["bar"].1,
      ),
      (original["foo"].0, original["bar"].0, 2, 3),
    );
  }

  #[test]
  fn import_retains_truncated_records_and_preserves_new_duplicates() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          [
            Ok(Record {
              execution: Execution {
                command: "foo".into(),
                timestamp_ns: 1,
                ..Default::default()
              },
              fingerprint: b"foo".to_vec(),
            }),
            Ok(Record {
              execution: Execution {
                command: "bar".into(),
                timestamp_ns: 2,
                ..Default::default()
              },
              fingerprint: b"bar".to_vec(),
            }),
          ],
          |_| {},
        )
        .unwrap(),
      2,
    );

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          [Ok(Record {
            execution: Execution {
              command: "bar".into(),
              timestamp_ns: 1,
              ..Default::default()
            },
            fingerprint: b"bar".to_vec(),
          })],
          |_| {},
        )
        .unwrap(),
      0,
    );

    assert_eq!(database.recent(20).unwrap().len(), 2);

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          [
            Ok(Record {
              execution: Execution {
                command: "bar".into(),
                timestamp_ns: 1,
                ..Default::default()
              },
              fingerprint: b"bar".to_vec(),
            }),
            Ok(Record {
              execution: Execution {
                command: "bar".into(),
                timestamp_ns: 2,
                ..Default::default()
              },
              fingerprint: b"bar".to_vec(),
            }),
          ],
          |_| {},
        )
        .unwrap(),
      1,
    );

    assert_eq!(database.recent(20).unwrap().len(), 3);
  }

  #[test]
  fn import_rolls_back_complete_batch_on_constraint_failure() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let error = database
      .import(
        "test",
        Path::new("foo"),
        [
          Ok(Record {
            execution: Execution {
              command: "foo".into(),
              ..Default::default()
            },
            fingerprint: b"foo".to_vec(),
          }),
          Ok(Record {
            execution: Execution {
              command: "bar".into(),
              duration_ns: Some(-1),
              ..Default::default()
            },
            fingerprint: b"bar".to_vec(),
          }),
        ],
        |_| {},
      )
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
  fn import_rolls_back_complete_batch_on_iterator_failure() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    let mut progress = Vec::new();

    let error = database
      .import(
        "test",
        Path::new("foo"),
        [
          Ok(Record {
            execution: Execution {
              command: "foo".into(),
              ..Default::default()
            },
            fingerprint: b"foo".to_vec(),
          }),
          Err(Error::msg("bar")),
        ],
        |status| progress.push((status.processed, status.inserted)),
      )
      .unwrap_err();

    assert_eq!(
      (error.to_string(), progress, database.recent(20).unwrap(),),
      ("bar".into(), Vec::new(), Vec::new()),
    );
  }

  #[test]
  fn import_superseded_source_generation_is_discarded() {
    let database =
      Database::try_from(Connection::open_in_memory().unwrap()).unwrap();

    assert_eq!(
      database
        .import(
          "test",
          Path::new("foo"),
          iter::once_with(|| {
            database.reserve_source("test", b"foo").unwrap();

            Ok(Record {
              execution: Execution {
                command: "foo".into(),
                ..Default::default()
              },
              fingerprint: b"foo".to_vec(),
            })
          }),
          |_| {},
        )
        .unwrap(),
      0,
    );

    assert_eq!(database.recent(20).unwrap(), Vec::new());
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
      i64::try_from(Database::SCHEMA_VERSION).unwrap(),
    );
  }

  #[test]
  fn open_upgrades_schema() {
    let connection = Connection::open_in_memory().unwrap();

    connection.execute_batch(Database::MIGRATIONS[0]).unwrap();

    for (id, command, timestamp_ns) in
      [("foo", "foo", 1), ("bar", "bar", 2), ("baz", "foo", 3)]
    {
      connection
        .execute(
          "INSERT INTO executions (id, command, timestamp_ns)
           VALUES (?1, ?2, ?3)",
          params![id, command, timestamp_ns],
        )
        .unwrap();
    }

    connection.pragma_update(None, "user_version", 1).unwrap();

    let database = Database::try_from(connection).unwrap();

    let columns = database
      .connection()
      .prepare("SELECT name FROM pragma_index_info('executions_timestamp')")
      .unwrap()
      .query_map([], |row| row.get::<_, String>(0))
      .unwrap()
      .collect::<rusqlite::Result<Vec<_>>>()
      .unwrap();

    let commands = database
      .connection()
      .prepare(
        "SELECT command, timestamp_ns, execution_id
         FROM commands
         ORDER BY timestamp_ns DESC, execution_id DESC",
      )
      .unwrap()
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, i64>(1)?,
          row.get::<_, String>(2)?,
        ))
      })
      .unwrap()
      .collect::<rusqlite::Result<Vec<_>>>()
      .unwrap();

    assert_eq!(
      (
        database
          .connection()
          .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
          .unwrap(),
        columns,
        commands,
      ),
      (
        4,
        vec!["timestamp_ns".into(), "id".into()],
        vec![
          ("foo".into(), 3, "baz".into()),
          ("bar".into(), 2, "bar".into())
        ],
      ),
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
  fn try_from_path_creates_private_database() {
    let root = tempfile::tempdir().unwrap();

    let path = root.path().join("foo/history.db");

    fs::create_dir_all(path.parent().unwrap()).unwrap();

    let database = Database::try_from(path.as_path()).unwrap();

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
  fn unsupported_schema_is_rejected() {
    let connection = Connection::open_in_memory().unwrap();

    connection.execute_batch("PRAGMA user_version = 5").unwrap();

    let Err(error) = Database::try_from(connection) else {
      panic!("expected unsupported schema to fail")
    };

    assert_eq!(
      error.to_string(),
      "database schema version 5 is unsupported; expected 4",
    );
  }
}
