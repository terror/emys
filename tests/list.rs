use super::*;

#[test]
fn list() {
  Test::new()
    .args([
      "add",
      "--timestamp-ns",
      "1",
      "--exit-code",
      "1",
      "--",
      "foo",
    ])
    .success()
    .args([
      "add",
      "--timestamp-ns",
      "2",
      "--exit-code",
      "0",
      "--",
      "bar",
    ])
    .success()
    .arg("list")
    .stdout(indoc! {
      "
      2\t0\tbar
      1\t1\tfoo
      "
    })
    .success()
    .args(["list", "--limit", "1"])
    .stdout("2\t0\tbar\n")
    .success()
    .args(["list", "--limit", "0"])
    .success();
}
