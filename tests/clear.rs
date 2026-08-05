use super::*;

#[test]
fn clear() {
  Test::new()
    .write("history", "foo\n")
    .args(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success()
    .arg("clear")
    .success()
    .assert_execution_count(0);
}
