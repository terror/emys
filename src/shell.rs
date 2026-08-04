use super::*;

#[derive(Clone, Debug, ValueEnum)]
pub(super) enum Shell {
  Bash,
  Fish,
  Zsh,
}

impl Shell {
  pub(super) fn init(self) -> String {
    match self {
      Self::Bash => include_str!("shell/bash/init.bash").into(),
      Self::Fish => include_str!("shell/fish/init.fish").into(),
      Self::Zsh => include_str!("shell/zsh/init.zsh").into(),
    }
  }
}
