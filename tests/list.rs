use super::*;

#[test]
fn list() {
  Test::new()
    .arguments([
      "add",
      "--timestamp-ns",
      "1",
      "--exit-code",
      "1",
      "--",
      "foo",
    ])
    .success()
    .arguments([
      "add",
      "--timestamp-ns",
      "2",
      "--exit-code",
      "0",
      "--",
      "bar",
    ])
    .success()
    .argument("list")
    .stdout(indoc! {
      "
      2\t0\tbar
      1\t1\tfoo
      "
    })
    .success()
    .arguments(["list", "--limit", "1"])
    .stdout("2\t0\tbar\n")
    .success()
    .arguments(["list", "--limit", "0"])
    .success();
}
