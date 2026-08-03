use {
  rusqlite::Connection,
  std::{
    env, fs,
    io::{self, Write},
    iter::once,
    path::Path,
    process::{Command, Output, Stdio},
  },
  tempfile::tempdir,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn init(binary: &str, directory: &Path) -> Output {
  Command::new(binary)
    .env("XDG_DATA_HOME", directory)
    .args(["init", "zsh"])
    .output()
    .unwrap()
}

fn record(binary: &str, directory: &Path) -> Output {
  Command::new(binary)
    .env("XDG_DATA_HOME", directory)
    .args([
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
    .output()
    .unwrap()
}

#[test]
fn add() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  let add = record(binary, directory.path());

  assert_eq!(
    (
      add.status.success(),
      String::from_utf8(add.stdout).unwrap(),
      String::from_utf8(add.stderr).unwrap(),
    ),
    (true, String::new(), String::new()),
  );
}

#[test]
fn backup() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");
  let path = directory.path().join("foo/bar/emys.sqlite");

  assert!(record(binary, directory.path()).status.success());

  let backup = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .arg("backup")
    .arg(&path)
    .output()
    .unwrap();

  assert_eq!(
    (
      backup.status.success(),
      String::from_utf8(backup.stdout).unwrap(),
      String::from_utf8(backup.stderr).unwrap(),
    ),
    (true, String::new(), String::new()),
  );

  let connection = Connection::open(&path).unwrap();

  assert_eq!(
    (
      connection
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .unwrap(),
      connection
        .query_row("SELECT COUNT(*) FROM executions", [], |row| {
          row.get::<_, i64>(0)
        })
        .unwrap(),
    ),
    ("ok".into(), 1),
  );

  drop(connection);

  #[cfg(unix)]
  assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600,);

  let existing = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .arg("backup")
    .arg(&path)
    .output()
    .unwrap();

  assert_eq!(
    (
      existing.status.success(),
      String::from_utf8(existing.stdout).unwrap(),
      String::from_utf8(existing.stderr).unwrap(),
    ),
    (
      false,
      String::new(),
      format!(
        "error: backup `{}` already exists; use --force to overwrite it\n",
        path.display(),
      ),
    ),
  );

  assert!(record(binary, directory.path()).status.success());

  let forced = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args(["backup", "--force"])
    .arg(&path)
    .output()
    .unwrap();

  assert_eq!(
    (
      forced.status.success(),
      String::from_utf8(forced.stdout).unwrap(),
      String::from_utf8(forced.stderr).unwrap(),
      Connection::open(path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM executions", [], |row| {
          row.get::<_, i64>(0)
        })
        .unwrap(),
    ),
    (true, String::new(), String::new(), 2),
  );
}

#[test]
fn import_zsh() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");
  let history = directory.path().join("history");

  fs::write(&history, "git status\n: 1700000000:2;cargo test\n").unwrap();

  let import = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args(["import", "zsh"])
    .arg(&history)
    .output()
    .unwrap();

  assert_eq!(
    (
      import.status.success(),
      String::from_utf8(import.stdout).unwrap(),
      String::from_utf8(import.stderr).unwrap(),
    ),
    (
      true,
      format!("imported 2 executions from {}\n", history.display()),
      String::new(),
    ),
  );

  let database = directory.path().join("emys/history.db");
  let connection = Connection::open(&database).unwrap();
  let rows = connection
    .prepare(
      "SELECT command, timestamp_ns, duration_ns, shell
       FROM executions
       ORDER BY timestamp_ns DESC",
    )
    .unwrap()
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, Option<i64>>(2)?,
        row.get::<_, Option<String>>(3)?,
      ))
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap();

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

  let reimport = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args(["import", "zsh"])
    .arg(&history)
    .output()
    .unwrap();

  assert_eq!(
    (
      reimport.status.success(),
      String::from_utf8(reimport.stdout).unwrap(),
      String::from_utf8(reimport.stderr).unwrap(),
      Connection::open(database)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM executions", [], |row| {
          row.get::<_, i64>(0)
        })
        .unwrap(),
    ),
    (
      true,
      format!("imported 0 executions from {}\n", history.display()),
      String::new(),
      2,
    ),
  );
}

#[test]
fn import_zsh_histfile() {
  let directory = tempdir().unwrap();

  let history = directory.path().join("history");

  fs::write(&history, "foo\n").unwrap();

  let import = Command::new(env!("CARGO_BIN_EXE_emys"))
    .env("HISTFILE", &history)
    .env("XDG_DATA_HOME", directory.path())
    .args(["import", "zsh"])
    .output()
    .unwrap();

  assert_eq!(
    (
      import.status.success(),
      String::from_utf8(import.stdout).unwrap(),
      String::from_utf8(import.stderr).unwrap(),
      Connection::open(directory.path().join("emys/history.db"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM executions", [], |row| {
          row.get::<_, i64>(0)
        })
        .unwrap(),
    ),
    (
      true,
      format!("imported 1 executions from {}\n", history.display()),
      String::new(),
      1,
    ),
  );
}

#[test]
fn init_zsh() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  let init = init(binary, directory.path());

  let script = String::from_utf8(init.stdout).unwrap();

  assert_eq!(
    (
      init.status.success(),
      script.contains("command emys search --interactive -- \"$BUFFER\""),
      script.contains("zle -N emys-search _emys_search"),
      script.contains("bindkey '^R' emys-search"),
      script.as_str(),
      String::from_utf8(init.stderr).unwrap(),
    ),
    (
      true,
      true,
      true,
      true,
      include_str!("../src/subcommand/init.zsh"),
      String::new(),
    ),
  );

  let mut zsh = match Command::new("zsh")
    .arg("-n")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
  {
    Ok(zsh) => zsh,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return,
    Err(error) => panic!("failed to run zsh: {error}"),
  };

  zsh
    .stdin
    .take()
    .unwrap()
    .write_all(script.as_bytes())
    .unwrap();

  let output = zsh.wait_with_output().unwrap();

  assert_eq!(
    (
      output.status.success(),
      String::from_utf8(output.stdout).unwrap(),
      String::from_utf8(output.stderr).unwrap(),
    ),
    (true, String::new(), String::new()),
  );
}

#[cfg(unix)]
#[test]
fn interactive_search_empty() {
  let directory = tempdir().unwrap();

  let search = Command::new(env!("CARGO_BIN_EXE_emys"))
    .env("XDG_DATA_HOME", directory.path())
    .args(["search", "--interactive", "--", "foo"])
    .output()
    .unwrap();

  assert_eq!(
    (
      search.status.success(),
      String::from_utf8(search.stdout).unwrap(),
      String::from_utf8(search.stderr).unwrap(),
    ),
    (true, String::new(), String::new()),
  );
}

#[cfg(not(unix))]
#[test]
fn interactive_search_unsupported() {
  let directory = tempdir().unwrap();

  let search = Command::new(env!("CARGO_BIN_EXE_emys"))
    .env("XDG_DATA_HOME", directory.path())
    .args(["search", "--interactive", "--", "foo"])
    .output()
    .unwrap();

  assert_eq!(
    (
      search.status.success(),
      String::from_utf8(search.stdout).unwrap(),
      String::from_utf8(search.stderr).unwrap(),
    ),
    (
      false,
      String::new(),
      "error: interactive search is unsupported on this platform\n".into(),
    ),
  );
}

#[test]
fn list() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  assert!(record(binary, directory.path()).status.success());

  let list = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args(["list", "--limit", "1"])
    .output()
    .unwrap();

  assert_eq!(
    (
      list.status.success(),
      String::from_utf8(list.stdout).unwrap(),
      String::from_utf8(list.stderr).unwrap(),
    ),
    (true, "1\t0\tfoo\n".into(), String::new()),
  );
}

#[test]
fn search() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  assert!(record(binary, directory.path()).status.success());

  let search = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args(["search", "--limit", "20", "FO"])
    .output()
    .unwrap();

  assert_eq!(
    (
      search.status.success(),
      String::from_utf8(search.stdout).unwrap(),
      String::from_utf8(search.stderr).unwrap(),
    ),
    (true, "1\t0\tfoo\n".into(), String::new()),
  );
}

#[test]
fn zsh_records_execution() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  let script =
    String::from_utf8(init(binary, directory.path()).stdout).unwrap();

  let path = env::var_os("PATH").unwrap_or_default();

  let path = env::join_paths(
    once(Path::new(binary).parent().unwrap().to_path_buf())
      .chain(env::split_paths(&path)),
  )
  .unwrap();

  let mut zsh = match Command::new("zsh")
    .arg("-f")
    .env("PATH", path)
    .env("XDG_DATA_HOME", directory.path())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
  {
    Ok(zsh) => zsh,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return,
    Err(error) => panic!("failed to run zsh: {error}"),
  };

  zsh
    .stdin
    .take()
    .unwrap()
    .write_all(
      format!(
        "{script}\nadd-zsh-hook -d preexec _emys_preexec\nadd-zsh-hook -d precmd _emys_precmd\n_emys_preexec 'foo'\nfalse\n_emys_precmd\n_emys_precmd\n",
      )
      .as_bytes(),
    )
    .unwrap();

  let output = zsh.wait_with_output().unwrap();

  assert_eq!(
    (
      output.status.code(),
      String::from_utf8(output.stdout).unwrap(),
      String::from_utf8(output.stderr).unwrap(),
    ),
    (Some(1), String::new(), String::new()),
  );

  let connection =
    Connection::open(directory.path().join("emys/history.db")).unwrap();

  assert_eq!(
    connection
      .query_row(
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
      )
      .unwrap(),
    (
      1,
      "foo".into(),
      1,
      env::current_dir().unwrap().to_string_lossy().into_owned(),
      true,
      true,
      "zsh".into(),
      true,
      true,
    ),
  );
}
