use {
  anyhow::{Context, Error, bail},
  arguments::Arguments,
  clap::{Parser, ValueEnum},
  database::Database,
  execution::Execution,
  rusqlite::{Connection, params},
  std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{Duration, SystemTime, UNIX_EPOCH},
  },
  subcommand::Subcommand,
  uuid::Uuid,
};

#[cfg(unix)]
use {std::os::unix::fs::PermissionsExt, xdg::BaseDirectories};

mod arguments;
mod database;
mod execution;
mod subcommand;

type Result<T = (), E = Error> = std::result::Result<T, E>;

fn main() {
  if let Err(error) = Arguments::parse().run() {
    eprintln!("error: {error}");

    for (i, error) in error.chain().skip(1).enumerate() {
      if i == 0 {
        eprintln!();
        eprintln!("because:");
      }

      eprintln!("- {error}");
    }

    process::exit(1);
  }
}
