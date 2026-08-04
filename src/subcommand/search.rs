use super::*;

#[cfg(unix)]
const BATCH_SIZE: usize = 256;

#[cfg(unix)]
const CHANNEL_CAPACITY: usize = 8;

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
  #[cfg(unix)]
  fn load_items(database: &Database, sender: &SkimItemSender) -> Result {
    let mut batch: Vec<Arc<dyn SkimItem>> = Vec::with_capacity(BATCH_SIZE);
    let mut flush_threshold = 1;

    database.for_each_command(|command| {
      batch.push(Arc::new(command));

      if batch.len() < flush_threshold {
        return true;
      }

      let next_batch = Vec::with_capacity(BATCH_SIZE);
      let current_batch = mem::replace(&mut batch, next_batch);

      if sender.send(current_batch).is_err() {
        return false;
      }

      flush_threshold = BATCH_SIZE;
      true
    })?;

    if !batch.is_empty() {
      let _ = sender.send(batch);
    }

    Ok(())
  }

  pub(crate) fn run(self, database: Database) -> Result {
    if self.interactive {
      self.run_interactive(database)
    } else {
      self.run_non_interactive(&database)
    }
  }

  #[cfg(not(unix))]
  fn run_interactive(&self, _database: Database) -> Result {
    bail!("interactive search is unsupported on this platform")
  }

  #[cfg(unix)]
  fn run_interactive(&self, database: Database) -> Result {
    if !database.has_executions()? {
      return Ok(());
    }

    let options = SkimOptionsBuilder::default()
      .height("40%")
      .multi(false)
      .query(&self.query)
      .build()?;

    let (sender, receiver) = bounded(CHANNEL_CAPACITY);

    let loader = thread::spawn(move || Self::load_items(&database, &sender));

    let output = Skim::run_with(options, Some(receiver));

    let load_result = loader
      .join()
      .map_err(|_| Error::msg("interactive search loader panicked"))?;

    load_result?;

    let output = output.map_err(|error| Error::msg(error.to_string()))?;

    if output.is_abort {
      return Ok(());
    }

    if let Some(item) = output.selected_items.first() {
      println!("{}", item.output());
    }

    Ok(())
  }

  fn run_non_interactive(&self, database: &Database) -> Result {
    for (_, execution) in database.search(&self.query, self.limit)? {
      let exit_code = execution
        .exit_code
        .map_or_else(String::new, |code| code.to_string());

      println!(
        "{}\t{}\t{}",
        execution.timestamp_ns, exit_code, execution.command
      );
    }

    Ok(())
  }
}
