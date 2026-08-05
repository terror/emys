use super::*;

#[test]
fn bash() {
  Test::new()
    .write(
      "history",
      indoc! {
        r#"
        #1700000000
        for foo in bar; do
          echo "$foo"
        done
        #1700000001
        cargo test
        "#
      },
    )
    .args(["import", "--path", "history", "bash"])
    .stdout("imported 2 executions from history\n")
    .success()
    .assert_executions([
      Execution {
        shell: Some("bash".into()),
        ..Execution::new(
          indoc! {
            r#"
            for foo in bar; do
              echo "$foo"
            done
            "#
          }
          .trim_end(),
          1_700_000_000_000_000_000,
        )
      },
      Execution {
        shell: Some("bash".into()),
        ..Execution::new("cargo test", 1_700_000_001_000_000_000)
      },
    ]);
}

#[test]
fn defaults_are_shell_specific() {
  let test = Test::new()
    .write(".bash_history", "foo\n")
    .write(
      "fish/fish_history",
      indoc! {
        "
        - cmd: baz
          when: 2
        "
      },
    )
    .write("history", "qux\n")
    .write(".zsh_history", ": 1:0;bar\n");

  let home = test.path("");

  let history = test.path("history");

  test
    .env("HISTFILE", &history)
    .env("HOME", &home)
    .args(["import", "zsh"])
    .stdout("imported 1 executions from [ROOT]/.zsh_history\n")
    .success()
    .env("HISTFILE", &history)
    .env("HOME", &home)
    .args(["import", "bash"])
    .stdout("imported 1 executions from [ROOT]/.bash_history\n")
    .success()
    .env("HOME", &home)
    .args(["import", "fish"])
    .stdout("imported 1 executions from [ROOT]/fish/fish_history\n")
    .success()
    .assert_executions([
      Execution {
        shell: Some("bash".into()),
        ..Execution::new("foo", 1)
      },
      Execution {
        duration_ns: Some(0),
        shell: Some("zsh".into()),
        ..Execution::new("bar", 1_000_000_000)
      },
      Execution {
        shell: Some("fish".into()),
        ..Execution::new("baz", 2_000_000_000)
      },
    ]);
}

#[test]
fn fish() {
  Test::new()
    .write(
      "history",
      indoc! {
        r"
        - cmd: git status
          when: 1700000000
        - cmd: for foo in bar\n    echo $foo\nend
          when: 1700000001
          paths:
            - /foo
        "
      },
    )
    .args(["import", "--path", "history", "fish"])
    .stdout("imported 2 executions from history\n")
    .success()
    .assert_executions([
      Execution {
        shell: Some("fish".into()),
        ..Execution::new("git status", 1_700_000_000_000_000_000)
      },
      Execution {
        shell: Some("fish".into()),
        ..Execution::new(
          indoc! {
            "
            for foo in bar
                echo $foo
            end
            "
          }
          .trim_end(),
          1_700_000_001_000_000_000,
        )
      },
    ]);
}

#[test]
fn zsh() {
  Test::new()
    .write(
      "history",
      indoc! {
        "
        git status
        : 1700000000:2;cargo test
        "
      },
    )
    .args(["import", "--path", "history", "zsh"])
    .stdout("imported 2 executions from history\n")
    .success()
    .assert_executions([
      Execution {
        shell: Some("zsh".into()),
        ..Execution::new("git status", 1)
      },
      Execution {
        duration_ns: Some(2_000_000_000),
        shell: Some("zsh".into()),
        ..Execution::new("cargo test", 1_700_000_000_000_000_000)
      },
    ])
    .args(["import", "--path", "history", "zsh"])
    .stdout("imported 0 executions from history\n")
    .success()
    .assert_execution_count(2);
}
