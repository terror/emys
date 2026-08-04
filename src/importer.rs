use super::*;

mod zsh;

pub(crate) use zsh::Zsh;

pub(super) trait Importer {
  const NAME: &'static str;

  type Parser: Parser;

  /// Imports this source's history into the database.
  fn import(&self, database: &Database) -> Result {
    let path = self.path()?;

    let file = fs::File::open(&path).with_context(|| {
      format!("failed to read {} history `{}`", Self::NAME, path.display())
    })?;

    let metadata = file.metadata()?;

    let progress =
      Progress::new(Self::NAME, metadata.is_file().then_some(metadata.len()))?;

    let reader: Box<dyn Read> = if metadata.is_file() {
      Box::new(file.take(metadata.len()))
    } else {
      Box::new(file)
    };

    let reader = progress.reader(reader);

    let entries = Self::Parser::entries(reader);

    let mut occurrences = HashMap::new();

    let records = entries.map(|entry| {
      let entry = entry.with_context(|| {
        format!(
          "failed to parse {} history `{}`",
          Self::NAME,
          path.display()
        )
      })?;

      let key = entry.identity.identifier(Self::Parser::FORMAT, 0);

      let occurrence = occurrences.entry(key).or_insert(0_u64);

      *occurrence = occurrence
        .checked_add(1)
        .context("history occurrence count overflowed")?;

      let id = entry.identity.identifier(Self::Parser::FORMAT, *occurrence);

      let mut execution = entry.execution;

      execution.shell = Some(Self::Parser::FORMAT.into());

      Ok((id, execution))
    });

    let result = database.import(records, |status| progress.update(status));

    progress.finish();

    let inserted = result?;

    println!("imported {inserted} executions from {}", path.display());

    Ok(())
  }

  /// Determines the history file path from source-specific configuration.
  fn path(&self) -> Result<PathBuf>;
}
