use {
  anyhow::{self, Context, Result, bail},
  execution::Execution,
  rusqlite::{Connection, params},
  std::path::{Path, PathBuf},
  uuid::Uuid,
};

pub mod database;
pub mod execution;

fn main() {
  println!("Hello, world!");
}
