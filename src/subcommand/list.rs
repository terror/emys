use super::*;

#[derive(Debug, Clap)]
pub(crate) struct List {
  #[arg(short, long, default_value_t = 50)]
  limit: usize,
}

impl List {
  pub(crate) fn run(self, database: &Database) -> Result {
    for (_, entry) in database.recent(self.limit)? {
      println!(
        "{}\t{}\t{}",
        entry.timestamp_ns,
        entry
          .exit_code
          .map(|exit_code| exit_code.to_string())
          .unwrap_or_default(),
        entry.command,
      );
    }

    Ok(())
  }
}
