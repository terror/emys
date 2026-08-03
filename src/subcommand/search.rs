use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Search {
  #[arg(short, long, default_value_t = 50)]
  limit: usize,
  #[arg(default_value = "")]
  query: String,
}

impl Search {
  pub(crate) fn run(self, database: &Database) -> Result {
    for (_, execution) in database.search(&self.query, self.limit)? {
      println!(
        "{}\t{}\t{}",
        execution.timestamp_ns,
        execution
          .exit_code
          .map(|exit_code| exit_code.to_string())
          .unwrap_or_default(),
        execution.command,
      );
    }

    Ok(())
  }
}
