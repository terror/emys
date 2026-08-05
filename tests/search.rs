use super::*;

#[test]
fn empty() {
  Test::new().arguments(["search", "--", "foo"]).success();
}

#[cfg(unix)]
#[test]
fn populated() {
  use {
    expectrl::{Eof, Expect, Session},
    std::time::Duration,
  };

  let test = Test::new()
    .arguments(["add", "--timestamp-ns", "1", "--", "foo bar"])
    .success()
    .arguments(["add", "--timestamp-ns", "2", "--", "baz qux"])
    .success();

  let mut command = Command::new(env!("CARGO_BIN_EXE_honu"));

  command
    .current_dir(test.tempdir.path())
    .env("TERM", "xterm-256color")
    .env("XDG_DATA_HOME", test.tempdir.path())
    .args(["search", "--", "bar"]);

  let mut session = Session::spawn(command).unwrap();

  session.set_expect_timeout(Some(Duration::from_secs(10)));
  session.expect("\x1b[6n").unwrap();
  session.send("\x1b[24;1R").unwrap();
  session.expect("foo").unwrap();
  session.send("\r").unwrap();

  let output = session.expect(Eof).unwrap();
  let output = output.get(0).unwrap();
  let expected = b"foo bar\r\n";
  let selected = output
    .get(output.len().saturating_sub(expected.len())..)
    .unwrap();

  pretty_assert_eq!(selected, expected);
}
