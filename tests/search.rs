use super::*;

#[test]
fn empty() {
  Test::new().args(["search", "--", "foo"]).success();
}
