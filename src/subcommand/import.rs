use super::*;

#[derive(Debug, Clap)]
pub(crate) enum Import {
  Bash(Bash),
  Zsh(Zsh),
}

impl Import {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Bash(bash) => bash.import(database),
      Self::Zsh(zsh) => zsh.import(database),
    }
  }
}
