use {
  rusqlite::Connection,
  std::{
    env,
    io::{self, Write},
    iter::once,
    path::Path,
    process::{Command, Output, Stdio},
  },
  tempfile::tempdir,
};

fn init(binary: &str, directory: &Path) -> Output {
  Command::new(binary)
    .env("XDG_DATA_HOME", directory)
    .args(["init", "zsh"])
    .output()
    .unwrap()
}

#[test]
fn add_and_list() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  let add = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
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
    .unwrap();

  assert_eq!(
    (
      add.status.success(),
      String::from_utf8(add.stdout).unwrap(),
      String::from_utf8(add.stderr).unwrap(),
    ),
    (true, String::new(), String::new()),
  );

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
fn init_zsh() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  let init = init(binary, directory.path());

  let script = String::from_utf8(init.stdout).unwrap();

  assert_eq!(
    (
      init.status.success(),
      script.as_str(),
      String::from_utf8(init.stderr).unwrap(),
    ),
    (
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
