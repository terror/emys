use super::*;

impl Test {
  fn assert_shell_execution(self, shell: &str, hostname: Option<&str>) -> Self {
    let directory = self.path("").canonicalize().unwrap();
    let executions = self.executions();

    pretty_assert_eq!(executions.len(), 1);

    let execution = &executions[0];

    assert!(execution.timestamp_ns > 0);
    assert!(execution.duration_ns.is_some_and(|duration| duration >= 0));

    let hostname = hostname.map(String::from).or_else(|| {
      execution
        .hostname
        .as_ref()
        .filter(|hostname| !hostname.is_empty())
        .cloned()
    });

    assert!(hostname.is_some());

    pretty_assert_eq!(
      executions,
      [Execution {
        command: "foo".into(),
        directory: Some(directory),
        duration_ns: execution.duration_ns,
        exit_code: Some(1),
        hostname,
        session: Some("bar".into()),
        shell: Some(shell.into()),
        timestamp_ns: execution.timestamp_ns,
      }],
    );

    self
  }
}

#[test]
#[ignore = "requires bash"]
fn bash_records_execution() {
  let script = include_str!("../src/shell/bash/init.bash");

  Test::new()
    .shell(Shell::Bash)
    .stdin(formatdoc! {
      "
      HOSTNAME=baz
      export HONU_SESSION=bar HONU_SHLVL=$SHLVL
      {script}
      _honu_preexec 'foo'
      false
      _honu_precmd
      "
    })
    .status(1)
    .assert_shell_execution("bash", Some("baz"));
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
      set -gx HONU_SESSION bar
      set -gx HONU_SHLVL $SHLVL
      {script}
      _honu_preexec 'foo'
      false
      _honu_postexec
      "
    })
    .status(1)
    .assert_shell_execution("fish", None);
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
      HOST=baz
      typeset -gx HONU_SESSION=bar HONU_SHLVL=$SHLVL
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
    .assert_shell_execution("zsh", Some("baz"));
}
