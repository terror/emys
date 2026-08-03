use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Backup {
  #[arg(long)]
  force: bool,
  #[arg(value_name = "PATH")]
  path: PathBuf,
}

impl Backup {
  pub(crate) fn run(self, database: &Database) -> Result {
    if self.force {
      database.force_backup(self.path)
    } else {
      database.backup(self.path)
    }
  }
}
