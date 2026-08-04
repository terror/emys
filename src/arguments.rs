use super::*;

#[derive(Debug, Parser)]
#[command(version, about)]
pub(crate) struct Arguments {
  #[clap(subcommand)]
  subcommand: Subcommand,
}

impl Arguments {
  pub(crate) fn run(self) -> Result {
    #[cfg(unix)]
    let path =
      BaseDirectories::with_prefix("emys").place_data_file("history.db")?;

    #[cfg(windows)]
    let path = {
      let directory = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .context("failed to determine local data directory")?
        .join("emys");

      fs::create_dir_all(&directory)?;

      directory.join("history.db")
    };

    self.subcommand.run(Database::open(path)?)
  }
}
