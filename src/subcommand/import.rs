use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Import {
  #[arg(index = 2, value_name = "PATH")]
  path: Option<PathBuf>,
  #[arg(index = 1)]
  shell: Shell,
}

impl Import {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self.shell {
      Shell::Zsh => self.run_zsh(database),
    }
  }

  fn run_zsh(self, database: &Database) -> Result {
    let path = self
      .path
      .or_else(|| {
        env::var_os("HISTFILE")
          .filter(|path| !path.is_empty())
          .map(PathBuf::from)
      })
      .or_else(|| {
        env::var_os("HOME")
          .filter(|path| !path.is_empty())
          .map(|path| PathBuf::from(path).join(".zsh_history"))
      })
      .context(
        "failed to determine Zsh history path; pass PATH or set HISTFILE or HOME",
      )?;

    let contents = fs::read(&path).with_context(|| {
      format!("failed to read Zsh history `{}`", path.display())
    })?;

    let contents = String::from_utf8(contents).with_context(|| {
      format!("Zsh history `{}` is not valid UTF-8", path.display())
    })?;

    let records = crate::zsh_history::parse(&contents).with_context(|| {
      format!("failed to parse Zsh history `{}`", path.display())
    })?;

    let imported = database.import(&records)?;

    println!("imported {imported} executions from {}", path.display());

    Ok(())
  }
}
