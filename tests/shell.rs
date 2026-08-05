use super::*;

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
    .assert_executions(1);
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
      _honu_preexec 'foo'
      false
      _honu_postexec
      "
    })
    .status(1)
    .assert_executions(1);
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
    .assert_executions(1);
}
