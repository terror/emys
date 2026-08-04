use super::*;

#[derive(Debug, clap::Args)]
pub(crate) struct Zsh {
  #[arg(value_name = "PATH")]
  path: Option<PathBuf>,
}

impl Importer for Zsh {
  const DEFAULT_HISTORY_FILE: &'static str = ".zsh_history";
  const NAME: &'static str = "Zsh";

  type Parser = parser::Zsh;

  fn explicit_path(&self) -> Option<&Path> {
    self.path.as_deref()
  }
}
