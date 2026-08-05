use {
  anyhow::Context,
  indoc::{formatdoc, indoc},
  pretty_assertions::assert_eq as pretty_assert_eq,
  rusqlite::Connection,
  std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Write,
    iter::once,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str,
  },
  tempfile::TempDir,
};

mod add;
mod backup;
mod clear;
mod import;
mod init;
mod list;
mod search;
mod shell;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Execution {
  command: String,
  directory: Option<PathBuf>,
  duration_ns: Option<i64>,
  exit_code: Option<i32>,
  hostname: Option<String>,
  session: Option<String>,
  shell: Option<String>,
  timestamp_ns: i64,
}

impl Execution {
  fn new(command: &str, timestamp_ns: i64) -> Self {
    Self {
      command: command.into(),
      timestamp_ns,
      ..Default::default()
    }
  }
}

#[derive(Clone, Copy)]
enum Shell {
  Bash,
  Fish,
  Zsh,
}

impl Shell {
  fn arguments(self) -> &'static [&'static str] {
    match self {
      Self::Bash => &["--noprofile", "--norc"],
      Self::Fish => &["--no-config"],
      Self::Zsh => &["-f"],
    }
  }

  fn name(self) -> &'static str {
    match self {
      Self::Bash => "bash",
      Self::Fish => "fish",
      Self::Zsh => "zsh",
    }
  }
}

#[derive(Debug)]
struct Test {
  arguments: Vec<OsString>,
  environments: Vec<(OsString, OsString)>,
  executable: OsString,
  expected_stderr: String,
  expected_stdout: String,
  stdin: Option<Vec<u8>>,
  tempdir: TempDir,
}

impl Test {
  fn arg(self, argument: impl AsRef<OsStr>) -> Self {
    self.args([argument])
  }

  fn args<I, S>(mut self, arguments: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    assert!(self.arguments.is_empty());

    self.arguments.extend(
      arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_owned()),
    );

    self
  }

  fn assert_database(self, path: impl AsRef<Path>, executions: i64) -> Self {
    let connection = Connection::open(self.path(path)).unwrap();

    pretty_assert_eq!(
      (
        connection
          .query_row("PRAGMA integrity_check", [], |row| row
            .get::<_, String>(0))
          .unwrap(),
        connection
          .query_row("SELECT COUNT(*) FROM executions", [], |row| {
            row.get::<_, i64>(0)
          })
          .unwrap(),
      ),
      ("ok".into(), executions),
    );

    self
  }

  fn assert_execution_count(self, expected: i64) -> Self {
    pretty_assert_eq!(
      self
        .database()
        .query_row("SELECT COUNT(*) FROM executions", [], |row| {
          row.get::<_, i64>(0)
        })
        .unwrap(),
      expected,
    );

    self
  }

  fn assert_executions(
    self,
    expected: impl IntoIterator<Item = Execution>,
  ) -> Self {
    pretty_assert_eq!(
      self.executions(),
      expected.into_iter().collect::<Vec<_>>(),
    );

    self
  }

  #[track_caller]
  fn assert_recorded(self) {
    pretty_assert_eq!(self.executions().len(), 1);
  }

  fn database(&self) -> Connection {
    Connection::open(self.path("honu/history.db")).unwrap()
  }

  fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
    self
      .environments
      .push((key.as_ref().to_owned(), value.as_ref().to_owned()));

    self
  }

  fn executions(&self) -> Vec<Execution> {
    let connection = self.database();

    let mut statement = connection
      .prepare(
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
         ORDER BY timestamp_ns, id",
      )
      .unwrap();

    statement
      .query_map([], |row| {
        Ok(Execution {
          command: row.get(0)?,
          timestamp_ns: row.get(1)?,
          duration_ns: row.get(2)?,
          exit_code: row.get(3)?,
          directory: row.get::<_, Option<String>>(4)?.map(PathBuf::from),
          session: row.get(5)?,
          hostname: row.get(6)?,
          shell: row.get(7)?,
        })
      })
      .unwrap()
      .collect::<rusqlite::Result<Vec<_>>>()
      .unwrap()
  }

  #[track_caller]
  fn failure(self) -> Self {
    self.status(1)
  }

  fn new() -> Self {
    Self::with_tempdir(TempDir::with_prefix("honu-test").unwrap())
  }

  fn path(&self, path: impl AsRef<Path>) -> PathBuf {
    self.tempdir.path().join(path)
  }

  fn program(mut self, executable: impl AsRef<OsStr>) -> Self {
    executable.as_ref().clone_into(&mut self.executable);
    self
  }

  fn shell(self, shell: Shell) -> Self {
    let path = env::join_paths(
      once(
        Path::new(env!("CARGO_BIN_EXE_honu"))
          .parent()
          .unwrap()
          .to_path_buf(),
      )
      .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap();

    self
      .program(shell.name())
      .args(shell.arguments())
      .env("PATH", path)
  }

  #[track_caller]
  fn status(self, expected_status: i32) -> Self {
    let mut command = Command::new(&self.executable);

    command
      .current_dir(self.tempdir.path())
      .env("XDG_DATA_HOME", self.tempdir.path())
      .args(&self.arguments)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());

    for (key, value) in &self.environments {
      command.env(key, value);
    }

    let mut child = command
      .spawn()
      .with_context(|| {
        format!("failed to run `{}`", Path::new(&self.executable).display())
      })
      .unwrap();

    if let Some(stdin) = &self.stdin {
      child.stdin.take().unwrap().write_all(stdin).unwrap();
    }

    let output = child.wait_with_output().unwrap();

    let normalize = |text: &str| {
      text
        .replace(&self.tempdir.path().display().to_string(), "[ROOT]")
        .replace('\\', "/")
    };

    let stderr = normalize(str::from_utf8(&output.stderr).unwrap());

    pretty_assert_eq!(
      output.status.code(),
      Some(expected_status),
      "unexpected exit status\nstderr: {stderr}",
    );

    pretty_assert_eq!(stderr, self.expected_stderr);

    let stdout = normalize(str::from_utf8(&output.stdout).unwrap());

    pretty_assert_eq!(stdout, self.expected_stdout);

    Self::with_tempdir(self.tempdir)
  }

  fn stderr(mut self, expected_stderr: &str) -> Self {
    assert!(self.expected_stderr.is_empty());
    self.expected_stderr = expected_stderr.into();
    self
  }

  fn stdin(mut self, stdin: impl AsRef<[u8]>) -> Self {
    assert!(self.stdin.is_none());
    self.stdin = Some(stdin.as_ref().into());
    self
  }

  fn stdout(mut self, expected_stdout: &str) -> Self {
    assert!(self.expected_stdout.is_empty());
    self.expected_stdout = expected_stdout.into();
    self
  }

  #[track_caller]
  fn success(self) -> Self {
    self.status(0)
  }

  fn with_tempdir(tempdir: TempDir) -> Self {
    Self {
      arguments: Vec::new(),
      environments: Vec::new(),
      executable: env!("CARGO_BIN_EXE_honu").into(),
      expected_stderr: String::new(),
      expected_stdout: String::new(),
      stdin: None,
      tempdir,
    }
  }

  fn write(self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Self {
    let path = self.path(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
    self
  }
}
