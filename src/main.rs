use {
  anyhow::{Context, Error, bail},
  arguments::Arguments,
  candidate::Candidate,
  clap::{Parser as Clap, ValueEnum},
  command::Command,
  database::Database,
  entry::Entry,
  imara_diff::{Algorithm, Diff, InternedInput},
  indicatif::{ProgressBar, ProgressStyle},
  line::Line,
  lines::Lines,
  parser::Parser,
  progress::Progress,
  ratatui::{
    style::{Color, Modifier},
    text::Span,
  },
  record::Record,
  records::Records,
  rusqlite::{Connection, MAIN_DB, Transaction, TransactionBehavior, params},
  scan::Scan,
  shell::Shell,
  skim::{
    DisplayContext, Skim, SkimItem, SkimItemSender,
    options::SkimOptionsBuilder, prelude::bounded,
  },
  std::{
    borrow::Cow,
    env, fs,
    io::{self, BufRead, BufReader, IsTerminal, Read},
    mem,
    path::{Path, PathBuf},
    process, str,
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
  },
  subcommand::Subcommand,
  unicode_segmentation::UnicodeSegmentation,
  uuid::Uuid,
};

#[cfg(unix)]
use {
  std::os::unix::fs::{OpenOptionsExt, PermissionsExt},
  xdg::BaseDirectories,
};

mod arguments;
mod candidate;
mod command;
mod database;
mod entry;
mod line;
mod lines;
mod parser;
mod progress;
mod record;
mod records;
mod scan;
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
