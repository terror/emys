use super::*;

#[test]
fn clear() {
  Test::new()
    .write("history", "foo\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success()
    .argument("clear")
    .success()
    .assert_execution_count(0);
}
