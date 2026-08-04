use super::*;

mod zsh;

pub(crate) use zsh::Zsh;

pub(super) trait Importer {
  const NAME: &'static str;

  type Parser: Parser;

  /// Imports this source's history into the database.
  fn import(&self, database: &Database) -> Result {
    let path = self.path()?;

    let source = fs::canonicalize(&path).with_context(|| {
      format!(
        "failed to resolve {} history `{}`",
        Self::NAME,
        path.display()
      )
    })?;

    let file = fs::File::open(&source).with_context(|| {
      format!("failed to read {} history `{}`", Self::NAME, path.display())
    })?;

    let metadata = file.metadata()?;

    let progress = Progress::new(Self::NAME)?;

    let reader: Box<dyn Read> = if metadata.is_file() {
      Box::new(file.take(metadata.len()))
    } else {
      Box::new(file)
    };

    let reader = progress.reader(reader);

    let records = Self::Parser::records(reader).map(|record| {
      let mut record = record.with_context(|| {
        format!(
          "failed to parse {} history `{}`",
          Self::NAME,
          path.display()
        )
      })?;

      record.execution.shell = Some(Self::Parser::FORMAT.into());

      Ok(record)
    });

    let result =
      database.import(Self::Parser::FORMAT, &source, records, |status| {
        progress.update(status);
      });

    progress.finish();

    let inserted = result?;

    println!("imported {inserted} executions from {}", path.display());

    Ok(())
  }

  /// Determines the history file path from source-specific configuration.
  fn path(&self) -> Result<PathBuf>;
}
