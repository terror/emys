use super::*;

#[derive(Debug, Clap)]
pub(crate) enum Import {
  Bash(Bash),
  Fish(Fish),
  Zsh(Zsh),
}

impl Import {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Bash(bash) => bash.import(database),
      Self::Fish(fish) => fish.import(database),
      Self::Zsh(zsh) => zsh.import(database),
    }
  }
}
