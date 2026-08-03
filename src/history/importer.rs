use {super::*, std::collections::HashMap};

pub(super) trait Importer {
  const FORMAT: &'static str;
  const NAME: &'static str;

  /// Imports this source's history into the database.
  fn import(&self, database: &Database) -> Result {
    let path = self.path()?;

    let contents = fs::read(&path).with_context(|| {
      format!("failed to read {} history `{}`", Self::NAME, path.display())
    })?;

    let entries = self.parse(&contents).with_context(|| {
      format!(
        "failed to parse {} history `{}`",
        Self::NAME,
        path.display()
      )
    })?;

    let mut occurrences = HashMap::new();
    let mut records = Vec::with_capacity(entries.len());

    for entry in entries {
      let occurrence =
        occurrences.entry(entry.identity.clone()).or_insert(0_u64);

      *occurrence = occurrence
        .checked_add(1)
        .context("history occurrence count overflowed")?;

      let mut execution = entry.execution;

      execution.shell = Some(Self::FORMAT.into());

      records.push((
        identifier(Self::FORMAT, &entry.identity, *occurrence),
        execution,
      ));
    }

    let inserted = database.import(&records)?;

    println!("imported {inserted} executions from {}", path.display());

    Ok(())
  }

  /// Parses raw history file contents into imported executions.
  fn parse(&self, contents: &[u8]) -> Result<Vec<ImportedExecution>>;

  /// Determines the history file path from source-specific configuration.
  fn path(&self) -> Result<PathBuf>;
}
