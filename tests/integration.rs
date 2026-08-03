use {std::process::Command, tempfile::tempdir};

#[test]
fn add_and_list() {
  let directory = tempdir().unwrap();

  let binary = env!("CARGO_BIN_EXE_emys");

  let add = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args([
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
    .output()
    .unwrap();

  assert_eq!(
    (
      add.status.success(),
      String::from_utf8(add.stdout).unwrap(),
      String::from_utf8(add.stderr).unwrap(),
    ),
    (true, String::new(), String::new()),
  );

  let list = Command::new(binary)
    .env("XDG_DATA_HOME", directory.path())
    .args(["list", "--limit", "1"])
    .output()
    .unwrap();

  assert_eq!(
    (
      list.status.success(),
      String::from_utf8(list.stdout).unwrap(),
      String::from_utf8(list.stderr).unwrap(),
    ),
    (true, "1\t0\tfoo\n".into(), String::new()),
  );
}
