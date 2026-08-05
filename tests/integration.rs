use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

  let path = test.path("foo/bar/honu.sqlite");

  let history = test.path("history");

  fs::write(&history, "foo\n")?;

  test
    .command()
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("zsh")
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
      "error: backup `[ROOT]/foo/bar/honu.sqlite` already exists; use --force to overwrite it\n",
    )
    .run()?;

  fs::write(&history, "foo\nbar\n")?;

  test
    .command()
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("zsh")
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
fn clear() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(&history, "foo\n")?;

  test
    .command()
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("zsh")
    .expected_stdout("imported 1 executions from [ROOT]/history\n")
    .run()?;

  test.command().argument("clear").run()?;

  assert_eq!(
    test.database()?.query_row(
      "SELECT COUNT(*) FROM executions",
      [],
      |row| { row.get::<_, i64>(0) }
    )?,
    0,
  );

  Ok(())
}

#[test]
fn import_bash() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(
    &history,
    "#1700000000\nfor foo in bar; do\n  echo \"$foo\"\ndone\n#1700000001\ncargo test\n",
  )?;

  test
    .command()
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("bash")
    .expected_stdout("imported 2 executions from [ROOT]/history\n")
    .run()?;

  let rows = test
    .database()?
    .prepare(
      "SELECT command, timestamp_ns, shell
       FROM executions
       ORDER BY timestamp_ns",
    )?
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
      ))
    })?
    .collect::<rusqlite::Result<Vec<_>>>()?;

  assert_eq!(
    rows,
    [
      (
        "for foo in bar; do\n  echo \"$foo\"\ndone".into(),
        1_700_000_000_000_000_000,
        "bash".into(),
      ),
      (
        "cargo test".into(),
        1_700_000_001_000_000_000,
        "bash".into(),
      ),
    ],
  );

  Ok(())
}

#[test]
fn import_defaults_are_shell_specific() -> Result {
  let test = Test::new()?;

  let bash_history = test.path(".bash_history");
  let fish_history = test.path("fish/fish_history");
  let other_history = test.path("history");
  let zsh_history = test.path(".zsh_history");

  fs::create_dir(test.path("fish"))?;

  fs::write(&bash_history, "foo\n")?;
  fs::write(&fish_history, "- cmd: baz\n  when: 2\n")?;
  fs::write(&other_history, "qux\n")?;
  fs::write(&zsh_history, ": 1:0;bar\n")?;

  test
    .command()
    .environment("HISTFILE", &other_history)
    .environment("HOME", test.tempdir.path())
    .arguments(["import", "zsh"])
    .expected_stdout("imported 1 executions from [ROOT]/.zsh_history\n")
    .run()?;

  test
    .command()
    .environment("HISTFILE", &other_history)
    .environment("HOME", test.tempdir.path())
    .arguments(["import", "bash"])
    .expected_stdout("imported 1 executions from [ROOT]/.bash_history\n")
    .run()?;

  test
    .command()
    .environment("HOME", test.tempdir.path())
    .arguments(["import", "fish"])
    .expected_stdout("imported 1 executions from [ROOT]/fish/fish_history\n")
    .run()?;

  let rows = test
    .database()?
    .prepare("SELECT command, shell FROM executions ORDER BY shell")?
    .query_map([], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?
    .collect::<rusqlite::Result<Vec<_>>>()?;

  assert_eq!(
    rows,
    [
      ("foo".into(), "bash".into()),
      ("baz".into(), "fish".into()),
      ("bar".into(), "zsh".into()),
    ],
  );

  Ok(())
}

#[test]
fn import_fish() -> Result {
  let test = Test::new()?;

  let history = test.path("history");

  fs::write(
    &history,
    "- cmd: git status\n  when: 1700000000\n- cmd: for foo in bar\\n    echo $foo\\nend\n  when: 1700000001\n  paths:\n    - /foo\n",
  )?;

  test
    .command()
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("fish")
    .expected_stdout("imported 2 executions from [ROOT]/history\n")
    .run()?;

  let rows = test
    .database()?
    .prepare(
      "SELECT command, timestamp_ns, shell
       FROM executions
       ORDER BY timestamp_ns",
    )?
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
      ))
    })?
    .collect::<rusqlite::Result<Vec<_>>>()?;

  assert_eq!(
    rows,
    [
      (
        "git status".into(),
        1_700_000_000_000_000_000,
        "fish".into(),
      ),
      (
        "for foo in bar\n    echo $foo\nend".into(),
        1_700_000_001_000_000_000,
        "fish".into(),
      ),
    ],
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
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("zsh")
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
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("zsh")
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
fn init_bash() -> Result {
  Test::new()?
    .command()
    .arguments(["init", "bash"])
    .expected_stdout(
      &include_str!("../src/shell/bash/init.bash").replace('\\', "/"),
    )
    .run()?;

  Ok(())
}

#[test]
fn init_fish() -> Result {
  Test::new()?
    .command()
    .arguments(["init", "fish"])
    .expected_stdout(
      &include_str!("../src/shell/fish/init.fish").replace('\\', "/"),
    )
    .run()?;

  Ok(())
}

#[test]
fn init_zsh() -> Result {
  Test::new()?
    .command()
    .arguments(["init", "zsh"])
    .expected_stdout(include_str!("../src/shell/zsh/init.zsh"))
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
    .arguments(["import", "--path"])
    .argument(&history)
    .argument("zsh")
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
fn search_empty() -> Result {
  Test::new()?
    .command()
    .arguments(["search", "--", "foo"])
    .run()?;

  Ok(())
}
