use {
  anyhow::{Context, Error, bail},
  arguments::Arguments,
  clap::{Parser as Clap, ValueEnum},
  database::Database,
  entry::Entry,
  imara_diff::{Algorithm, Diff, InternedInput},
  indicatif::{ProgressBar, ProgressStyle},
  line::Line,
  lines::Lines,
  parser::Parser,
  progress::Progress,
  progress_entry::ProgressEntry,
  record::Record,
  records::Records,
  rusqlite::{Connection, MAIN_DB, Transaction, TransactionBehavior, params},
  shell::Shell,
  std::{
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
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    sync::Arc,
    thread,
  },
  xdg::BaseDirectories,
};

mod arguments;
mod database;
mod entry;
mod line;
mod lines;
mod parser;
mod progress;
mod progress_entry;
mod record;
mod records;
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
