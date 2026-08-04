use super::*;

mod bash;
mod zsh;

pub(crate) use {bash::Bash, zsh::Zsh};

pub(super) trait Importer {
  const DEFAULT_HISTORY_FILE: &'static str;
  const NAME: &'static str;

  type Parser: Parser;

  /// Returns the history path explicitly supplied by the user.
  fn explicit_path(&self) -> Option<&Path>;

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

  /// Resolves the history path from an explicit path or the user's home directory.
  fn path(&self) -> Result<PathBuf> {
    self
      .explicit_path()
      .map(Path::to_owned)
      .or_else(|| {
        env::var_os("HOME")
          .filter(|path| !path.is_empty())
          .map(|path| PathBuf::from(path).join(Self::DEFAULT_HISTORY_FILE))
      })
      .with_context(|| {
        format!(
          "failed to determine {} history path; pass PATH or set HOME",
          <Self::Parser as Parser>::FORMAT,
        )
      })
  }
}
