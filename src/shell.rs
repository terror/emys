use super::*;

#[derive(Clone, Debug, ValueEnum)]
pub(super) enum Shell {
  Zsh,
}

impl Shell {
  pub(super) fn init(self) -> String {
    match self {
      Self::Zsh => include_str!("shell/zsh/init.zsh").into(),
    }
  }
}
