use {super::*, imported_execution::ImportedExecution, importer::Importer};

mod imported_execution;
mod importer;
mod zsh;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Source {
  Zsh(zsh::Zsh),
}

impl Source {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Zsh(zsh) => zsh.import(database),
    }
  }
}
