use super::*;
use expectrl::{Eof, Expect, Session};

#[test]
#[ignore = "requires bash and a PTY"]
fn bash_installed_hooks_record_execution_once() {
  let test = Test::new();

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

  let mut command = Command::new(Shell::Bash.name());

  command
    .current_dir(test.tempdir.path())
    .env("HOME", test.tempdir.path())
    .env("HOSTNAME", "foo")
    .env("PATH", path)
    .env("PS1", "honu-test> ")
    .env("PS2", "")
    .env("XDG_DATA_HOME", test.tempdir.path())
    .args(Shell::Bash.arguments());

  let mut session = Session::spawn(command).unwrap();
  session.set_expect_timeout(Some(std::time::Duration::from_secs(10)));
  session.expect("honu-test> ").unwrap();

  for _ in 0..2 {
    session.send_line("eval \"$(honu init bash)\"").unwrap();
    session.expect("honu-test> ").unwrap();
  }

  session.send_line("false").unwrap();
  session.expect("honu-test> ").unwrap();
  session.send_line("exit").unwrap();
  session.expect(Eof).unwrap();

  let executions = test.executions();

  pretty_assert_eq!(executions.len(), 1);

  let execution = &executions[0];

  pretty_assert_eq!(execution.command, "false");
  pretty_assert_eq!(
    execution.directory,
    Some(test.tempdir.path().canonicalize().unwrap()),
  );
  assert!(execution.duration_ns.is_some_and(|duration| duration >= 0));
  pretty_assert_eq!(execution.exit_code, Some(1));
  pretty_assert_eq!(execution.hostname.as_deref(), Some("foo"));
  assert!(
    execution
      .session
      .as_ref()
      .is_some_and(|session| !session.is_empty())
  );
  pretty_assert_eq!(execution.shell.as_deref(), Some("bash"));
  assert!(execution.timestamp_ns > 0);
}

#[test]
#[ignore = "requires bash"]
fn bash_records_execution() {
  let script = include_str!("../src/shell/bash/init.bash");

  Test::new()
    .shell(Shell::Bash)
    .stdin(formatdoc! {
      "
      {script}
      _honu_preexec 'foo'
      false
      _honu_precmd
      "
    })
    .status(1)
    .assert_execution_count(1);
}

#[test]
#[ignore = "requires bash"]
fn bash_search_preserves_capture() {
  let script = include_str!("../src/shell/bash/init.bash");

  Test::new()
    .shell(Shell::Bash)
    .stdin(formatdoc! {
      "
      {script}
      _honu_preexec() {{ printf '%s\\n' \"$1\"; }}
      _honu_search() {{ :; }}
      history -c
      _honu_arm
      trap '_honu_debug \"$?\"' DEBUG
      _honu_search
      true
      "
    })
    .stdout("true\n")
    .success();
}

#[test]
#[ignore = "requires bash"]
fn bash_preserves_scalar_prompt_command() {
  Test::new()
    .shell(Shell::Bash)
    .write("init.bash", include_str!("../src/shell/bash/init.bash"))
    .write(
      "case.bash",
      indoc! {
        r#"
        PROMPT_COMMAND='printf "%s\n" foo;'
        source init.bash
        trap - DEBUG
        _honu_precmd() { printf '%s\n' precmd; }
        _honu_arm() { printf '%s\n' arm; }
        eval "$PROMPT_COMMAND"
        "#
      },
    )
    .stdin("bash --noprofile --norc -ic 'source case.bash' 2>/dev/null")
    .stdout(indoc! {
      "
      precmd
      foo
      arm
      "
    })
    .success();
}

#[test]
#[ignore = "requires bash"]
fn bash_uses_only_new_history() {
  let script = include_str!("../src/shell/bash/init.bash");

  Test::new()
    .shell(Shell::Bash)
    .stdin(formatdoc! {
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
    })
    .stdout(indoc! {
      "
      true
      fresh
      "
    })
    .success();
}

#[test]
#[ignore = "requires fish"]
fn fish_records_execution() {
  let script = include_str!("../src/shell/fish/init.fish");

  Test::new()
    .shell(Shell::Fish)
    .stdin(formatdoc! {
      "
      {script}
      set -e fish_private_mode
      _honu_preexec 'foo'
      false
      _honu_postexec
      "
    })
    .status(1)
    .assert_execution_count(1);
}

#[test]
#[ignore = "requires fish"]
fn fish_private_mode_is_not_recorded() {
  let script = include_str!("../src/shell/fish/init.fish");

  let test = Test::new()
    .shell(Shell::Fish)
    .stdin(formatdoc! {
      "
      {script}
      set -g fish_private_mode 1
      _honu_preexec 'foo'
      _honu_postexec
      "
    })
    .success();

  assert!(!test.path("honu/history.db").try_exists().unwrap());
}

#[test]
#[ignore = "requires bash"]
fn init_bash() {
  Test::new()
    .program("bash")
    .argument("-n")
    .stdin(include_str!("../src/shell/bash/init.bash"))
    .success();
}

#[test]
#[ignore = "requires fish"]
fn init_fish() {
  Test::new()
    .program("fish")
    .argument("-n")
    .stdin(include_str!("../src/shell/fish/init.fish"))
    .success();
}

#[test]
#[ignore = "requires zsh"]
fn init_zsh() {
  Test::new()
    .program("zsh")
    .argument("-n")
    .stdin(include_str!("../src/shell/zsh/init.zsh"))
    .success();
}

#[test]
#[ignore = "requires zsh"]
fn zsh_records_execution() {
  let script = include_str!("../src/shell/zsh/init.zsh");

  Test::new()
    .shell(Shell::Zsh)
    .stdin(formatdoc! {
      "
      {script}
      add-zsh-hook -d preexec _honu_preexec
      add-zsh-hook -d precmd _honu_precmd
      _honu_preexec 'foo'
      false
      _honu_precmd
      _honu_precmd
      "
    })
    .status(1)
    .assert_execution_count(1);
}
