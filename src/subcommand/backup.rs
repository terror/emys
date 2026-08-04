use super::*;

#[derive(Debug, Clap)]
pub(crate) struct Backup {
  #[arg(long)]
  force: bool,
  #[arg(value_name = "PATH")]
  path: PathBuf,
}

impl Backup {
  pub(crate) fn run(self, database: &Database) -> Result {
    database.backup(&self.path, self.force)
  }
}
