use super::*;

#[test]
fn shells() {
  #[track_caller]
  fn case(shell: &str, expected: &str) {
    Test::new()
      .args(["init", shell])
      .stdout(&expected.replace('\\', "/"))
      .success();
  }

  case("bash", include_str!("../src/shell/bash/init.bash"));
  case("fish", include_str!("../src/shell/fish/init.fish"));
  case("zsh", include_str!("../src/shell/zsh/init.zsh"));
}
