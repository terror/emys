use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Search {
  #[arg(short, long)]
  interactive: bool,
  #[arg(short, long, default_value_t = 50)]
  limit: usize,
  #[arg(default_value = "")]
  query: String,
}

impl Search {
  pub(crate) fn run(self, database: &Database) -> Result {
    if self.interactive {
      return self.run_interactive(database);
    }

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

  #[cfg(not(unix))]
  fn run_interactive(&self, _database: &Database) -> Result {
    bail!("interactive search is unsupported on this platform")
  }

  #[cfg(unix)]
  fn run_interactive(&self, database: &Database) -> Result {
    let executions = database.search("", 10_000)?;

    if executions.is_empty() {
      return Ok(());
    }

    let options = SkimOptionsBuilder::default()
      .height("40%")
      .multi(false)
      .query(&self.query)
      .build()?;

    let output = Skim::run_items(
      options,
      executions
        .into_iter()
        .map(|(_, execution)| execution.command),
    )
    .map_err(|error| Error::msg(error.to_string()))?;

    if !output.is_abort
      && let Some(item) = output.selected_items.first()
    {
      println!("{}", item.output());
    }

    Ok(())
  }
}
