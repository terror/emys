use super::*;

#[derive(Debug, Clap)]
pub(crate) struct Import {
  #[arg(long, value_name = "PATH")]
  path: Option<PathBuf>,
  shell: Shell,
}

impl Import {
  pub(crate) fn run(self, database: &Database) -> Result {
    self.shell.import(database, self.path.as_deref())
  }
}
