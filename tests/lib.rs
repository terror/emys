use {
  anyhow::{Context, Error},
  executable_path::executable_path,
  indoc::formatdoc,
  rusqlite::Connection,
  std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Write,
    iter::once,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    str,
  },
  tempfile::TempDir,
};

mod integration;
mod shell;

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
    Ok(Connection::open(self.path("honu/history.db"))?)
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
      tempdir: Rc::new(TempDir::with_prefix("honu-test")?),
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
