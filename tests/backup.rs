use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn backup() {
  let test = Test::new()
    .write("history", "foo\n")
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success()
    .arguments(["backup", "foo/bar/honu.sqlite"])
    .success()
    .assert_database("foo/bar/honu.sqlite", 1);

  #[cfg(unix)]
  assert_eq!(
    test
      .path("foo/bar/honu.sqlite")
      .metadata()
      .unwrap()
      .permissions()
      .mode()
      & 0o777,
    0o600,
  );

  test
    .arguments(["backup", "foo/bar/honu.sqlite"])
    .stderr(
      "error: backup `foo/bar/honu.sqlite` already exists; use --force to overwrite it\n",
    )
    .failure()
    .write(
      "history",
      indoc! {
        "
        foo
        bar
        "
      },
    )
    .arguments(["import", "--path", "history", "zsh"])
    .stdout("imported 1 executions from history\n")
    .success()
    .arguments(["backup", "--force", "foo/bar/honu.sqlite"])
    .success()
    .assert_database("foo/bar/honu.sqlite", 2);
}
