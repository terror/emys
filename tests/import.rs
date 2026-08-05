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
    .arguments(["import", "--path", "history", "bash"])
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
    .arguments(["import", "zsh"])
    .stdout("imported 1 executions from [ROOT]/.zsh_history\n")
    .success()
    .env("HISTFILE", &history)
    .env("HOME", &home)
    .arguments(["import", "bash"])
    .stdout("imported 1 executions from [ROOT]/.bash_history\n")
    .success()
    .env("HOME", &home)
    .arguments(["import", "fish"])
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
fn distinct_sources_are_tracked_independently() {
  Test::new()
    .write("foo", "bar\n")
    .write("baz", "bar\n")
    .arguments(["import", "--path", "foo", "zsh"])
    .stdout("imported 1 executions from foo\n")
    .success()
    .arguments(["import", "--path", "baz", "zsh"])
    .stdout("imported 1 executions from baz\n")
    .success()
    .arguments(["import", "--path", "foo", "zsh"])
    .stdout("imported 0 executions from foo\n")
    .success()
    .arguments(["import", "--path", "baz", "zsh"])
    .stdout("imported 0 executions from baz\n")
    .success()
    .assert_execution_count(2);
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
    .arguments(["import", "--path", "history", "fish"])
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
fn idempotent() {
  #[track_caller]
  fn case(shell: &str, history: &str) {
    Test::new()
      .write("history", history)
      .arguments(["import", "--path", "history", shell])
      .stdout("imported 2 executions from history\n")
      .success()
      .arguments(["import", "--path", "history", shell])
      .stdout("imported 0 executions from history\n")
      .success()
      .assert_execution_count(2);
  }

  case("bash", "#1\nfoo\n#2\nbar\n");
  case("fish", "- cmd: foo\n  when: 1\n- cmd: bar\n  when: 2\n");
  case("zsh", ": 1:0;foo\n: 2:0;bar\n");
}

#[test]
fn parse_failure_does_not_partially_import() {
  Test::new()
    .write("history", "#1\nfoo\n#9223372037\nbar\n")
    .arguments(["import", "--path", "history", "bash"])
    .stderr(
      "error: failed to parse Bash history `history`\n\nbecause:\n- timestamp on history line 3 overflows nanoseconds\n",
    )
    .failure()
    .assert_execution_count(0)
    .write("history", "#1\nfoo\n#2\nbar\n")
    .arguments(["import", "--path", "history", "bash"])
    .stdout("imported 2 executions from history\n")
    .success()
    .assert_execution_count(2);
}

#[test]
fn reconciles_insertions_with_existing_executions() {
  let test = Test::new()
    .write("history", "foo\nbar\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 2 executions from history\n")
    .success();

  let foo = test.execution_id("foo");
  let bar = test.execution_id("bar");

  let test = test
    .write("history", "baz\nfoo\nqux\nbar\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 2 executions from history\n")
    .success()
    .assert_executions([
      Execution {
        shell: Some("zsh".into()),
        ..Execution::new("baz", 1)
      },
      Execution {
        shell: Some("zsh".into()),
        ..Execution::new("foo", 2)
      },
      Execution {
        shell: Some("zsh".into()),
        ..Execution::new("qux", 3)
      },
      Execution {
        shell: Some("zsh".into()),
        ..Execution::new("bar", 4)
      },
    ]);

  assert_eq!(test.execution_id("foo"), foo);
  assert_eq!(test.execution_id("bar"), bar);
}

#[test]
fn repeated_commands_are_reconciled_by_occurrence() {
  Test::new()
    .write("history", "foo\nfoo\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 2 executions from history\n")
    .success()
    .write("history", "foo\nfoo\nfoo\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success()
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 0 executions from history\n")
    .success()
    .assert_execution_count(3);
}

#[test]
fn truncated_records_are_retained() {
  let test = Test::new()
    .write("history", "foo\nbar\nbaz\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 3 executions from history\n")
    .success();

  let bar = test.execution_id("bar");
  let baz = test.execution_id("baz");

  let test = test
    .write("history", "bar\nbaz\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 0 executions from history\n")
    .success()
    .assert_execution_count(3);

  assert_eq!(test.execution_id("bar"), bar);
  assert_eq!(test.execution_id("baz"), baz);
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
    .arguments(["import", "--path", "history", "zsh"])
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
    ]);
}
