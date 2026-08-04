use super::*;

#[derive(Debug, Clap)]
pub(crate) struct Search {
  #[arg(short, long)]
  limit: Option<usize>,
  #[arg(default_value = "")]
  query: String,
}

impl Search {
  const BATCH_SIZE: usize = 256;
  const CHANNEL_CAPACITY: usize = 8;

  fn load_items(self, database: &Database, sender: &SkimItemSender) -> Result {
    let mut batch: Vec<Arc<dyn SkimItem>> =
      Vec::with_capacity(Self::BATCH_SIZE);

    let mut flush_threshold = 1;

    let now_ns = i64::try_from(
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock precedes the Unix epoch")?
        .as_nanos(),
    )
    .context("system time exceeds SQLite integer range")?;

    database.for_each_latest_entry(self.limit, |entry| {
      batch.push(Arc::new(SearchItem::new(entry, now_ns)));

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

  pub(crate) fn run(self, database: Database) -> Result {
    if !database.has_entries()? {
      return Ok(());
    }

    let options = SkimOptionsBuilder::default()
      .color(
        "none,current:6:bold,matched:-1:underlined,current_match:6:bold:underlined,info:-1:dim,spinner:-1:dim",
      )
      .height("40%")
      .info("left")
      .multi(false)
      .multi_select_icon("")
      .query(&self.query)
      .selector_icon("")
      .build()?;

    let (sender, receiver) = bounded(Self::CHANNEL_CAPACITY);

    let loader = thread::spawn(move || self.load_items(&database, &sender));

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
