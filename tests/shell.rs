use super::*;

#[test]
#[ignore = "requires bash"]
fn bash_records_execution() -> Result {
  let test = Test::new()?;

  let script = include_str!("../src/shell/bash/init.bash");

  let path = env::join_paths(
    once(
      executable_path(env!("CARGO_PKG_NAME"))
        .parent()
        .unwrap()
        .to_path_buf(),
    )
    .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
  )?;

  let mut bash = Command::new("bash")
    .args(["--noprofile", "--norc"])
    .env("PATH", path)
    .env("XDG_DATA_HOME", test.path(""))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run bash")?;

  bash.stdin.take().unwrap().write_all(
    formatdoc! {
      "
        {script}
        _honu_preexec 'foo'
        false
        _honu_precmd
        "
    }
    .as_bytes(),
  )?;

  let output = bash.wait_with_output()?;

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
        shell,
        timestamp_ns > 0,
        duration_ns IS NULL OR duration_ns >= 0
      FROM executions",
      [],
      |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, i32>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, bool>(4)?,
          row.get::<_, String>(5)?,
          row.get::<_, bool>(6)?,
          row.get::<_, bool>(7)?,
        ))
      },
    )?,
    (
      1,
      "foo".into(),
      1,
      env::current_dir()?.to_string_lossy().replace('\\', "/"),
      true,
      "bash".into(),
      true,
      true,
    ),
  );

  Ok(())
}

#[test]
#[ignore = "requires bash"]
fn bash_uses_only_new_history() -> Result {
  let script = include_str!("../src/shell/bash/init.bash");

  let mut bash = Command::new("bash")
    .args(["--noprofile", "--norc"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run bash")?;

  bash.stdin.take().unwrap().write_all(
    formatdoc! {
      "
        {script}
        _honu_preexec() {{ printf '%s\\n' \"$1\"; }}
        history -c
        history -s stale
        _honu_arm
        trap '_honu_debug \"$?\"' DEBUG
        true
        trap - DEBUG
        history -s fresh
        __honu_ready=1
        trap '_honu_debug \"$?\"' DEBUG
        true
        "
    }
    .as_bytes(),
  )?;

  let output = bash.wait_with_output()?;

  assert_eq!(
    (
      output.status.code(),
      String::from_utf8(output.stdout)?,
      String::from_utf8(output.stderr)?,
    ),
    (Some(0), "true\nfresh\n".into(), String::new()),
  );

  Ok(())
}

#[test]
#[ignore = "requires fish"]
fn fish_records_execution() -> Result {
  let test = Test::new()?;

  let script = include_str!("../src/shell/fish/init.fish");

  let path = env::join_paths(
    once(
      executable_path(env!("CARGO_PKG_NAME"))
        .parent()
        .unwrap()
        .to_path_buf(),
    )
    .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
  )?;

  let mut fish = Command::new("fish")
    .arg("--no-config")
    .env("PATH", path)
    .env("XDG_DATA_HOME", test.path(""))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run fish")?;

  fish.stdin.take().unwrap().write_all(
    formatdoc! {
      "
        {script}
        _honu_preexec 'foo'
        false
        _honu_postexec
        "
    }
    .as_bytes(),
  )?;

  let output = fish.wait_with_output()?;

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
        duration_ns IS NULL OR duration_ns >= 0
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
      "fish".into(),
      true,
      true,
    ),
  );

  Ok(())
}

#[test]
#[ignore = "requires bash"]
fn init_bash() -> Result {
  let script = include_str!("../src/shell/bash/init.bash");

  let mut bash = Command::new("bash")
    .arg("-n")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run bash")?;

  bash.stdin.take().unwrap().write_all(script.as_bytes())?;

  let output = bash.wait_with_output()?;

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

#[test]
#[ignore = "requires fish"]
fn init_fish() -> Result {
  let script = include_str!("../src/shell/fish/init.fish");

  let mut fish = Command::new("fish")
    .arg("-n")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run fish")?;

  fish.stdin.take().unwrap().write_all(script.as_bytes())?;

  let output = fish.wait_with_output()?;

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

#[test]
#[ignore = "requires zsh"]
fn init_zsh() -> Result {
  let script = include_str!("../src/shell/zsh/init.zsh");

  let mut zsh = Command::new("zsh")
    .arg("-n")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run zsh")?;

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

#[test]
#[ignore = "requires zsh"]
fn zsh_records_execution() -> Result {
  let test = Test::new()?;

  let script = include_str!("../src/shell/zsh/init.zsh");

  let path = env::join_paths(
    once(
      executable_path(env!("CARGO_PKG_NAME"))
        .parent()
        .unwrap()
        .to_path_buf(),
    )
    .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
  )?;

  let mut zsh = Command::new("zsh")
    .arg("-f")
    .env("PATH", path)
    .env("XDG_DATA_HOME", test.path(""))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .context("failed to run zsh")?;

  zsh.stdin.take().unwrap().write_all(
    formatdoc! {
      "
        {script}
        add-zsh-hook -d preexec _honu_preexec
        add-zsh-hook -d precmd _honu_precmd
        _honu_preexec 'foo'
        false
        _honu_precmd
        _honu_precmd
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
