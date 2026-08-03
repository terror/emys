use {
  anyhow::Error,
  executable_path::executable_path,
  indoc::formatdoc,
  rusqlite::Connection,
  std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write},
    iter::once,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    str,
  },
  tempfile::TempDir,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

type Result<T = (), E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
struct Test {
  arguments: Vec<OsString>,
  environments: Vec<(OsString, OsString)>,
  expected_status: i32,
  expected_stderr: String,
  expected_stdout: String,
  tempdir: Rc<TempDir>,
}

impl Test {
  fn argument(mut self, argument: impl AsRef<OsStr>) -> Self {
    self.arguments.push(argument.as_ref().to_owned());
    self
  }

  fn arguments<I, S>(mut self, arguments: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    self.arguments.extend(
      arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_owned()),
    );

    self
  }

  fn command(&self) -> Self {
    Self {
      arguments: Vec::new(),
      environments: Vec::new(),
      expected_status: 0,
      expected_stderr: String::new(),
      expected_stdout: String::new(),
      tempdir: Rc::clone(&self.tempdir),
    }
  }

  fn database(&self) -> Result<Connection> {
    Ok(Connection::open(self.path("emys/history.db"))?)
  }

  fn environment(
    mut self,
    key: impl AsRef<OsStr>,
    value: impl AsRef<OsStr>,
  ) -> Self {
    self
      .environments
      .push((key.as_ref().to_owned(), value.as_ref().to_owned()));

    self
  }

  fn expected_status(self, expected_status: i32) -> Self {
    Self {
      expected_status,
      ..self
    }
  }

  fn expected_stderr(self, expected_stderr: &str) -> Self {
    Self {
      expected_stderr: expected_stderr.into(),
      ..self
    }
  }

  fn expected_stdout(self, expected_stdout: &str) -> Self {
    Self {
      expected_stdout: expected_stdout.into(),
      ..self
    }
  }

  fn new() -> Result<Self> {
    Ok(Self {
      arguments: Vec::new(),
      environments: Vec::new(),
      expected_status: 0,
      expected_stderr: String::new(),
      expected_stdout: String::new(),
      tempdir: Rc::new(TempDir::with_prefix("emys-test")?),
    })
  }

  fn path(&self, path: impl AsRef<Path>) -> PathBuf {
    self.tempdir.path().join(path)
  }

  fn run(self) -> Result<String> {
    let output = Command::new(executable_path(env!("CARGO_PKG_NAME")))
      .env("XDG_DATA_HOME", self.tempdir.path())
      .envs(self.environments)
      .args(self.arguments)
      .output()?;

    let normalize = |text: &str| {
      text
        .replace(&self.tempdir.path().display().to_string(), "[ROOT]")
        .replace('\\', "/")
    };

    let stderr = normalize(str::from_utf8(&output.stderr)?);

    assert_eq!(
      output.status.code(),
      Some(self.expected_status),
      "unexpected exit status\nstderr: {stderr}",
    );

    assert_eq!(stderr, self.expected_stderr);

    let stdout = normalize(str::from_utf8(&output.stdout)?);

    assert_eq!(stdout, self.expected_stdout);

    Ok(stdout)
  }
}

#[test]
fn add_record() -> Result {
  Test::new()?
    .arguments([
      "add",
      "--directory",
      "/foo",
      "--duration-ns",
      "2",
      "--exit-code",
      "0",
      "--hostname",
      "foo",
      "--session",
      "bar",
      "--shell",
      "zsh",
      "--timestamp-ns",
      "1",
      "--",
      "foo",
    ])
    .run()?;

  Ok(())
}

#[test]
fn backup() -> Result {
  let test = Test::new()?;

  let path = test.path("foo/bar/emys.sqlite");

  let history = test.path("history");

  fs::write(&history, "foo\n")?;

  test
    .command()
    .arguments(["import", "zsh"])
    .argument(&history)
    .expected_stdout("imported 1 executions from [ROOT]/history\n")
    .run()?;

  test.command().argument("backup").argument(&path).run()?;

  let connection = Connection::open(&path)?;

  assert_eq!(
    (
      connection.query_row("PRAGMA integrity_check", [], |row| row
        .get::<_, String>(0))?,
      connection.query_row("SELECT COUNT(*) FROM executions", [], |row| {
        row.get::<_, i64>(0)
      })?,
    ),
    ("ok".into(), 1),
  );

  drop(connection);

  #[cfg(unix)]
  assert_eq!(path.metadata()?.permissions().mode() & 0o777, 0o600);

  test
    .command()
    .argument("backup")
    .argument(&path)
    .expected_status(1)
    .expected_stderr(
      "error: backup `[ROOT]/foo/bar/emys.sqlite` already exists; use --force to overwrite it\n",
    )
    .run()?;

  fs::write(&history, "foo\nbar\n")?;

  test
    .command()
    .arguments(["import", "zsh"])
    .argument(&history)
    .expected_stdout("imported 1 executions from [ROOT]/history\n")
    .run()?;

  test
    .command()
    .arguments(["backup", "--force"])
    .argument(&path)
    .run()?;

  assert_eq!(
    Connection::open(path)?.query_row(
      "SELECT COUNT(*) FROM executions",
      [],
      |row| { row.get::<_, i64>(0) }
    )?,
    2,
  );

  Ok(())
}

#[test]
fn import_zsh() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(&history, "git status\n: 1700000000:2;cargo test\n")?;

  test
    .command()
    .arguments(["import", "zsh"])
    .argument(&history)
    .expected_stdout("imported 2 executions from [ROOT]/history\n")
    .run()?;

  let connection = test.database()?;

  let rows = connection
    .prepare(
      "SELECT command, timestamp_ns, duration_ns, shell
       FROM executions
       ORDER BY timestamp_ns DESC",
    )?
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, Option<i64>>(2)?,
        row.get::<_, Option<String>>(3)?,
      ))
    })?
    .collect::<rusqlite::Result<Vec<_>>>()?;

  assert_eq!(
    rows,
    vec![
      (
        "cargo test".into(),
        1_700_000_000_000_000_000,
        Some(2_000_000_000),
        Some("zsh".into()),
      ),
      ("git status".into(), 1, None, Some("zsh".into())),
    ],
  );

  drop(connection);

  test
    .command()
    .arguments(["import", "zsh"])
    .argument(&history)
    .expected_stdout("imported 0 executions from [ROOT]/history\n")
    .run()?;

  assert_eq!(
    test.database()?.query_row(
      "SELECT COUNT(*) FROM executions",
      [],
      |row| { row.get::<_, i64>(0) }
    )?,
    2,
  );

  Ok(())
}

#[test]
fn import_zsh_histfile() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(&history, "foo\n")?;

  test
    .command()
    .environment("HISTFILE", &history)
    .arguments(["import", "zsh"])
    .expected_stdout("imported 1 executions from [ROOT]/history\n")
    .run()?;

  assert_eq!(
    test.database()?.query_row(
      "SELECT COUNT(*) FROM executions",
      [],
      |row| { row.get::<_, i64>(0) }
    )?,
    1,
  );

  Ok(())
}

#[test]
fn init_zsh() -> Result {
  let test = Test::new()?;

  let script = test
    .command()
    .arguments(["init", "zsh"])
    .expected_stdout(include_str!("../src/subcommand/init.zsh"))
    .run()?;

  let mut zsh = match Command::new("zsh")
    .arg("-n")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
  {
    Ok(zsh) => zsh,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(error) => panic!("failed to run zsh: {error}"),
  };

  zsh.stdin.take().unwrap().write_all(script.as_bytes())?;

  let output = zsh.wait_with_output()?;

  assert_eq!(
    (
      output.status.success(),
      String::from_utf8(output.stdout)?,
      String::from_utf8(output.stderr)?,
    ),
    (true, String::new(), String::new()),
  );

  Ok(())
}

#[cfg(unix)]
#[test]
fn interactive_search_empty() -> Result {
  Test::new()?
    .command()
    .arguments(["search", "--interactive", "--", "foo"])
    .run()?;

  Ok(())
}

#[cfg(not(unix))]
#[test]
fn interactive_search_unsupported() -> Result {
  Test::new()?
    .command()
    .arguments(["search", "--interactive", "--", "foo"])
    .expected_status(1)
    .expected_stderr(
      "error: interactive search is unsupported on this platform\n",
    )
    .run()?;

  Ok(())
}

#[test]
fn list() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(&history, "foo\n")?;

  test
    .command()
    .arguments(["import", "zsh"])
    .argument(&history)
    .expected_stdout("imported 1 executions from [ROOT]/history\n")
    .run()?;

  test
    .command()
    .arguments(["list", "--limit", "1"])
    .expected_stdout("1\t\tfoo\n")
    .run()?;

  Ok(())
}

#[test]
fn search() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(&history, "foo\n")?;

  test
    .command()
    .arguments(["import", "zsh"])
    .argument(&history)
    .expected_stdout("imported 1 executions from [ROOT]/history\n")
    .run()?;

  test
    .command()
    .arguments(["search", "--limit", "20", "FO"])
    .expected_stdout("1\t\tfoo\n")
    .run()?;

  Ok(())
}

#[test]
fn zsh_records_execution() -> Result {
  let test = Test::new()?;

  let script = test
    .command()
    .arguments(["init", "zsh"])
    .expected_stdout(include_str!("../src/subcommand/init.zsh"))
    .run()?;

  let path = env::join_paths(
    once(
      executable_path(env!("CARGO_PKG_NAME"))
        .parent()
        .unwrap()
        .to_path_buf(),
    )
    .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
  )?;

  let mut zsh = match Command::new("zsh")
    .arg("-f")
    .env("PATH", path)
    .env("XDG_DATA_HOME", test.tempdir.path())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
  {
    Ok(zsh) => zsh,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(error) => panic!("failed to run zsh: {error}"),
  };

  zsh.stdin.take().unwrap().write_all(
    formatdoc! {
      "
        {script}
        add-zsh-hook -d preexec _emys_preexec
        add-zsh-hook -d precmd _emys_precmd
        _emys_preexec 'foo'
        false
        _emys_precmd
        _emys_precmd
        "
    }
    .as_bytes(),
  )?;

  let output = zsh.wait_with_output()?;

  assert_eq!(
    (
      output.status.code(),
      String::from_utf8(output.stdout)?,
      String::from_utf8(output.stderr)?,
    ),
    (Some(1), String::new(), String::new()),
  );

  assert_eq!(
    test.database()?.query_row(
      "SELECT
        COUNT(*),
        command,
        exit_code,
        directory,
        session <> '',
        hostname <> '',
        shell,
        timestamp_ns > 0,
        duration_ns >= 0
      FROM executions",
      [],
      |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, i32>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, bool>(4)?,
          row.get::<_, bool>(5)?,
          row.get::<_, String>(6)?,
          row.get::<_, bool>(7)?,
          row.get::<_, bool>(8)?,
        ))
      },
    )?,
    (
      1,
      "foo".into(),
      1,
      env::current_dir()?.to_string_lossy().into_owned(),
      true,
      true,
      "zsh".into(),
      true,
      true,
    ),
  );

  Ok(())
}
