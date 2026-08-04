use super::*;

#[derive(Debug, Clap)]
pub(crate) struct Add {
  #[arg(last = true, value_name = "COMMAND")]
  command: String,
  #[arg(long, value_name = "PATH")]
  directory: Option<PathBuf>,
  #[arg(long, value_name = "NANOSECONDS")]
  duration_ns: Option<i64>,
  #[arg(long, value_name = "CODE")]
  exit_code: Option<i32>,
  #[arg(long, value_name = "HOSTNAME")]
  hostname: Option<String>,
  #[arg(long, value_name = "SESSION")]
  session: Option<String>,
  #[arg(long, value_name = "SHELL")]
  shell: Option<String>,
  #[arg(long, value_name = "NANOSECONDS")]
  timestamp_ns: Option<i64>,
}

impl Add {
  pub(crate) fn run(self, database: &Database) -> Result {
    let timestamp_ns = if let Some(timestamp_ns) = self.timestamp_ns {
      timestamp_ns
    } else {
      let timestamp_ns =
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

      i64::try_from(timestamp_ns)
        .context("timestamp exceeds SQLite integer range")?
    };

    let directory = if let Some(directory) = self.directory {
      directory
    } else {
      env::current_dir()?
    };

    database.insert(&Entry {
      command: self.command,
      timestamp_ns,
      duration_ns: self.duration_ns,
      exit_code: self.exit_code,
      directory: Some(directory),
      session: self.session,
      hostname: self.hostname,
      shell: self.shell,
    })?;

    Ok(())
  }
}
