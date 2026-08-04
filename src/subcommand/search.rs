use super::*;

#[derive(Debug, Clap)]
pub(crate) struct Search {
  #[arg(short, long, default_value_t = 50)]
  limit: usize,
  #[arg(default_value = "")]
  query: String,
}

impl Search {
  #[cfg(unix)]
  const BATCH_SIZE: usize = 256;
  #[cfg(unix)]
  const CHANNEL_CAPACITY: usize = 8;

  #[cfg(unix)]
  fn load_items(
    database: &Database,
    sender: &SkimItemSender,
    limit: usize,
  ) -> Result {
    let mut batch: Vec<Arc<dyn SkimItem>> =
      Vec::with_capacity(Self::BATCH_SIZE);

    let mut flush_threshold = 1;

    database.for_each_command(limit, |command| {
      batch.push(Arc::new(command));

      if batch.len() < flush_threshold {
        return true;
      }

      let next_batch = Vec::with_capacity(Self::BATCH_SIZE);

      if sender.send(mem::replace(&mut batch, next_batch)).is_err() {
        return false;
      }

      flush_threshold = Self::BATCH_SIZE;

      true
    })?;

    if !batch.is_empty() {
      let _ = sender.send(batch);
    }

    Ok(())
  }

  #[cfg(not(unix))]
  pub(crate) fn run(self, _database: Database) -> Result {
    bail!("search is unsupported on this platform")
  }

  #[cfg(unix)]
  pub(crate) fn run(self, database: Database) -> Result {
    if !database.has_entries()? {
      return Ok(());
    }

    let options = SkimOptionsBuilder::default()
      .height("40%")
      .multi(false)
      .query(&self.query)
      .build()?;

    let (sender, receiver) = bounded(Self::CHANNEL_CAPACITY);

    let limit = self.limit;

    let loader =
      thread::spawn(move || Self::load_items(&database, &sender, limit));

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
}
