use super::*;

#[derive(Debug, clap::Args)]
pub(crate) struct Fish {
  #[arg(value_name = "PATH")]
  path: Option<PathBuf>,
}

impl Importer for Fish {
  const DEFAULT_HISTORY_FILE: &'static str = ".local/share/fish/fish_history";
  const NAME: &'static str = "Fish";

  type Parser = parser::Fish;

  fn explicit_path(&self) -> Option<&Path> {
    self.path.as_deref()
  }

  fn path(&self) -> Result<PathBuf> {
    self
      .explicit_path()
      .map(Path::to_owned)
      .or_else(|| {
        env::var_os("XDG_DATA_HOME")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
          .map(|path| path.join("fish/fish_history"))
      })
      .or_else(|| {
        env::var_os("HOME")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
          .map(|path| path.join(Self::DEFAULT_HISTORY_FILE))
      })
      .context(
        "failed to determine fish history path; pass PATH or set XDG_DATA_HOME or HOME",
      )
  }
}
