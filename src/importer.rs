use super::*;

mod zsh;

pub(crate) use zsh::Zsh;

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

    let records = entries
      .into_iter()
      .map(|entry| {
        let occurrence = occurrences
          .entry((entry.scheme.clone(), entry.fields.clone()))
          .or_insert(0_u64);

        *occurrence = occurrence
          .checked_add(1)
          .context("history occurrence count overflowed")?;

        let id = entry.identifier(Self::FORMAT, *occurrence);

        let mut execution = entry.execution;

        execution.shell = Some(Self::FORMAT.into());

        Ok((id, execution))
      })
      .collect::<anyhow::Result<Vec<_>>>()?;

    let inserted = database.import(&records)?;

    println!("imported {inserted} executions from {}", path.display());

    Ok(())
  }

  /// Parses raw history file contents into imported executions.
  fn parse(&self, contents: &[u8]) -> Result<Vec<ImportedExecution>>;

  /// Determines the history file path from source-specific configuration.
  fn path(&self) -> Result<PathBuf>;
}
