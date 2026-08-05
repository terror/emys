use super::*;

#[test]
fn defaults() {
  Test::new()
    .arguments(["add", "--", "foo"])
    .success()
    .assert_executions(1);
}

#[test]
fn record() {
  Test::new()
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
    .success()
    .assert_executions([Execution {
      directory: Some("/foo".into()),
      duration_ns: Some(2),
      exit_code: Some(0),
      hostname: Some("foo".into()),
      session: Some("bar".into()),
      shell: Some("zsh".into()),
      ..Execution::new("foo", 1)
    }]);
}
