use super::*;

#[derive(Debug, Parser)]
pub(crate) enum Import {
  Zsh(Zsh),
}

impl Import {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Zsh(zsh) => zsh.import(database),
    }
  }
}
