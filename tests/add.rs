use super::*;

#[test]
fn defaults() {
  let test = Test::new().arguments(["add", "--", "foo"]).success();

  let executions = test.executions();

  let [execution] = executions.as_slice() else {
    panic!("expected one execution, got {executions:?}");
  };

  assert!(execution.timestamp_ns > 0);

  let directory = execution.directory.as_ref().unwrap();

  assert_eq!(
    directory.canonicalize().unwrap(),
    test.path("").canonicalize().unwrap(),
  );

  assert_eq!(
    execution,
    &Execution {
      directory: Some(directory.clone()),
      ..Execution::new("foo", execution.timestamp_ns)
    },
  );
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
