use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Import {
  #[command(subcommand)]
  source: crate::history::Source,
}

impl Import {
  pub(crate) fn run(self, database: &Database) -> Result {
    self.source.run(database)
  }
}
