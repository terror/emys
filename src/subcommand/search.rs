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
  #[cfg(unix)]
  const BATCH_SIZE: usize = 256;

  #[cfg(unix)]
  fn load(database: &Database, sender: &SkimItemSender) -> Result {
    let mut batch = Vec::with_capacity(Self::BATCH_SIZE);
    let mut batch_size = 1;

    database.for_each_command(|command| {
      batch.push(Arc::new(command) as Arc<dyn SkimItem>);

      if batch.len() < batch_size {
        return true;
      }

      let sent = sender
        .send(mem::replace(
          &mut batch,
          Vec::with_capacity(Self::BATCH_SIZE),
        ))
        .is_ok();

      batch_size = Self::BATCH_SIZE;

      sent
    })?;

    if !batch.is_empty() {
      let _ = sender.send(batch);
    }

    Ok(())
  }

  pub(crate) fn run(self, database: Database) -> Result {
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

    let (sender, receiver) = bounded(8);

    let loader = thread::spawn(move || Self::load(&database, &sender));

    let output = Skim::run_with(options, Some(receiver));

    loader
      .join()
      .map_err(|_| Error::msg("interactive search loader panicked"))??;

    let output = output.map_err(|error| Error::msg(error.to_string()))?;

    if !output.is_abort
      && let Some(item) = output.selected_items.first()
    {
      println!("{}", item.output());
    }

    Ok(())
  }
}
