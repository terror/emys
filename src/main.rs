use {
  anyhow::{self, Context, Result, bail},
  execution::Execution,
  rusqlite::{Connection, params},
  std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
  },
  uuid::Uuid,
  xdg::BaseDirectories,
};

pub mod database;
pub mod execution;

fn main() {
  println!("Hello, world!");
}
