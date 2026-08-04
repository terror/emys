use super::*;

mod bash;
mod fish;
mod zsh;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum Shell {
  Bash,
  Fish,
  Zsh,
}

impl Shell {
  pub(super) fn format(self) -> &'static str {
    match self {
      Self::Bash => bash::FORMAT,
      Self::Fish => fish::FORMAT,
      Self::Zsh => zsh::FORMAT,
    }
  }

  fn history_file(self) -> &'static str {
    match self {
      Self::Bash => bash::DEFAULT_HISTORY_FILE,
      Self::Fish => fish::DEFAULT_HISTORY_FILE,
      Self::Zsh => zsh::DEFAULT_HISTORY_FILE,
    }
  }

  fn history_path(self, path: Option<&Path>) -> Result<PathBuf> {
    path
      .map(Path::to_owned)
      .or_else(|| match self {
        Self::Fish => env::var_os("XDG_DATA_HOME")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
          .map(|path| path.join("fish/fish_history")),
        Self::Bash | Self::Zsh => None,
      })
      .or_else(|| {
        env::var_os("HOME")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
          .map(|path| path.join(self.history_file()))
      })
      .with_context(|| match self {
        Self::Fish => "failed to determine fish history path; pass --path or set XDG_DATA_HOME or HOME".into(),
        Self::Bash | Self::Zsh => format!(
          "failed to determine {} history path; pass --path or set HOME",
          self.format(),
        ),
      })
  }

  pub(super) fn import(
    self,
    database: &Database,
    path: Option<&Path>,
  ) -> Result {
    let path = self.history_path(path)?;

    let source = fs::canonicalize(&path).with_context(|| {
      format!(
        "failed to resolve {} history `{}`",
        self.name(),
        path.display()
      )
    })?;

    let file = fs::File::open(&source).with_context(|| {
      format!(
        "failed to read {} history `{}`",
        self.name(),
        path.display()
      )
    })?;

    let metadata = file.metadata()?;

    let progress = Progress::new(self.name())?;

    let reader: Box<dyn Read> = if metadata.is_file() {
      Box::new(file.take(metadata.len()))
    } else {
      Box::new(file)
    };

    let reader = progress.reader(reader);

    let records = self.records(reader).map(|record| {
      let mut record = record.with_context(|| {
        format!(
          "failed to parse {} history `{}`",
          self.name(),
          path.display()
        )
      })?;

      record.entry.shell = Some(self.format().into());

      Ok(record)
    });

    let result = database.import(self.format(), &source, records, |status| {
      progress.update(status);
    });

    progress.finish();

    let inserted = result?;

    println!("imported {inserted} entries from {}", path.display());

    Ok(())
  }

  pub(super) fn init(self) -> &'static str {
    match self {
      Self::Bash => bash::INIT,
      Self::Fish => fish::INIT,
      Self::Zsh => zsh::INIT,
    }
  }

  fn name(self) -> &'static str {
    match self {
      Self::Bash => bash::NAME,
      Self::Fish => fish::NAME,
      Self::Zsh => zsh::NAME,
    }
  }

  pub(super) fn parser(self) -> Box<dyn Parser> {
    match self {
      Self::Bash => Box::new(bash::parser::Parser::default()),
      Self::Fish => Box::new(fish::parser::Parser::default()),
      Self::Zsh => Box::new(zsh::parser::Parser::default()),
    }
  }

  fn reader<'a>(self, reader: impl Read + 'a) -> Box<dyn Read + 'a> {
    match self {
      Self::Bash | Self::Fish => Box::new(reader),
      Self::Zsh => Box::new(zsh::decode(reader)),
    }
  }

  fn records<'a>(
    self,
    reader: impl Read + 'a,
  ) -> Records<Box<dyn Read + 'a>, Box<dyn Parser>> {
    Records::new(self.reader(reader), self.parser())
  }
}
