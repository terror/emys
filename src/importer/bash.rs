use super::*;

#[derive(Debug, clap::Args)]
pub(crate) struct Bash {
  #[arg(value_name = "PATH")]
  path: Option<PathBuf>,
}

impl Importer for Bash {
  const NAME: &'static str = "Bash";

  type Parser = parser::Bash;

  fn path(&self) -> Result<PathBuf> {
    self
      .path
      .clone()
      .or_else(|| {
        env::var_os("HISTFILE")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
      })
      .or_else(|| {
        env::var_os("HOME")
          .filter(|path| !path.is_empty())
          .map(|path| PathBuf::from(path).join(".bash_history"))
      })
      .context(
        "failed to determine bash history path; pass PATH or set HISTFILE or HOME",
      )
  }
}
