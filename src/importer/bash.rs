use super::*;

#[derive(Debug, clap::Args)]
pub(crate) struct Bash {
  #[arg(value_name = "PATH")]
  path: Option<PathBuf>,
}

impl Importer for Bash {
  const DEFAULT_HISTORY_FILE: &'static str = ".bash_history";
  const NAME: &'static str = "Bash";

  type Parser = parser::Bash;

  fn explicit_path(&self) -> Option<&Path> {
    self.path.as_deref()
  }
}
