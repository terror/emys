use super::*;

#[test]
fn clear() {
  let test = Test::new()
    .write("history", "foo\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success();

  let id = test.execution_id("foo");

  let test = test
    .argument("clear")
    .success()
    .assert_executions([])
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success()
    .assert_execution_count(1);

  assert_ne!(test.execution_id("foo"), id);
}
