use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Clear {}

impl Clear {
  pub(crate) fn run(database: &Database) -> Result {
    database.clear()
  }
}
