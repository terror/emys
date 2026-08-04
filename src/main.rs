use {
  anyhow::{Context, Error, bail},
  arguments::Arguments,
  clap::{Parser, ValueEnum},
  database::{Database, ImportProgress},
  execution::Execution,
  identity::Identity,
  importer::{Importer, Zsh},
  indicatif::{ProgressBar, ProgressStyle},
  parsed_execution::ParsedExecution,
  rusqlite::{Connection, MAIN_DB, params},
  shell::Shell,
  std::{
    collections::HashMap,
    env, fs,
    io::{self, BufRead, BufReader, IsTerminal, Read},
    mem,
    path::{Path, PathBuf},
    process, str,
    time::{Duration, SystemTime, UNIX_EPOCH},
  },
  subcommand::Subcommand,
  uuid::Uuid,
};

#[cfg(unix)]
use {
  skim::{
    Skim, SkimItem, SkimItemSender, options::SkimOptionsBuilder,
    prelude::bounded,
  },
  std::{
    collections::HashSet,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    sync::Arc,
    thread,
  },
  xdg::BaseDirectories,
};

mod arguments;
mod database;
mod execution;
mod identity;
mod importer;
mod parsed_execution;
mod shell;
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
