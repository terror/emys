use super::*;

const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Debug, clap::Args)]
pub(crate) struct Zsh {
  #[arg(value_name = "PATH")]
  path: Option<PathBuf>,
}

impl Importer for Zsh {
  const FORMAT: &'static str = "zsh";
  const NAME: &'static str = "Zsh";

  fn parse(contents: &[u8]) -> Result<Vec<ImportedExecution>> {
    parse_history(contents)
  }

  fn path(&self) -> Result<PathBuf> {
    self
      .path
      .clone()
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
      )
  }
}

fn logical_entries(contents: &str) -> Vec<(usize, String)> {
  let mut current = None;
  let mut entries = Vec::new();

  for (index, physical) in contents.split_inclusive('\n').enumerate() {
    let has_newline = physical.ends_with('\n');
    let physical = physical.strip_suffix('\n').unwrap_or(physical);
    let start = index + 1;
    let entry = current.get_or_insert_with(|| (start, String::new()));

    if has_newline && physical.ends_with('\\') {
      entry.1.push_str(&physical[..physical.len() - 1]);
      entry.1.push('\n');
    } else {
      entry.1.push_str(physical);

      let entry = current.take().unwrap();

      if !entry.1.is_empty() {
        entries.push(entry);
      }
    }
  }

  if let Some(entry) = current
    && !entry.1.is_empty()
  {
    entries.push(entry);
  }

  entries
}

fn parse_history(contents: &[u8]) -> Result<Vec<ImportedExecution>> {
  let contents =
    std::str::from_utf8(contents).context("Zsh history is not valid UTF-8")?;

  let mut plain_timestamp_ns = 0_i64;
  let mut records = Vec::new();

  for (line, command) in logical_entries(contents) {
    if let Some((timestamp_ns, duration_ns, command)) =
      parse_extended(&command, line)?
    {
      if command.is_empty() {
        continue;
      }

      records.push(ImportedExecution {
        execution: Execution {
          command: command.into(),
          duration_ns: Some(duration_ns),
          timestamp_ns,
          ..Default::default()
        },
        fields: vec![
          command.as_bytes().to_vec(),
          timestamp_ns.to_be_bytes().to_vec(),
          duration_ns.to_be_bytes().to_vec(),
        ],
        scheme: b"extended".to_vec(),
      });
    } else {
      plain_timestamp_ns = plain_timestamp_ns
        .checked_add(1)
        .context("plain history timestamp exceeds SQLite integer range")?;

      let fields = vec![command.as_bytes().to_vec()];

      records.push(ImportedExecution {
        execution: Execution {
          command,
          timestamp_ns: plain_timestamp_ns,
          ..Default::default()
        },
        fields,
        scheme: b"plain".to_vec(),
      });
    }
  }

  Ok(records)
}

fn parse_extended(
  command: &str,
  line: usize,
) -> Result<Option<(i64, i64, &str)>> {
  let Some(metadata) = command.strip_prefix(": ") else {
    return Ok(None);
  };

  let Some((timestamp, metadata)) = metadata.split_once(':') else {
    return Ok(None);
  };

  let Some((duration, command)) = metadata.split_once(';') else {
    return Ok(None);
  };

  if timestamp.is_empty()
    || duration.is_empty()
    || !timestamp.bytes().all(|byte| byte.is_ascii_digit())
    || !duration.bytes().all(|byte| byte.is_ascii_digit())
  {
    return Ok(None);
  }

  let timestamp_ns = nanoseconds(timestamp, "timestamp", line)?;
  let duration_ns = nanoseconds(duration, "duration", line)?;

  Ok(Some((timestamp_ns, duration_ns, command)))
}

fn nanoseconds(value: &str, field: &str, line: usize) -> Result<i64> {
  value
    .parse::<u64>()
    .ok()
    .and_then(|value| value.checked_mul(NANOSECONDS_PER_SECOND))
    .and_then(|value| i64::try_from(value).ok())
    .with_context(|| {
      format!("{field} on history line {line} overflows nanoseconds")
    })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn commands_beginning_with_colon() {
    assert_eq!(
      Zsh::parse(b": foo\n: 1:x;bar\n: 1:2bar;baz").unwrap(),
      vec![
        ImportedExecution {
          execution: Execution {
            command: ": foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          fields: vec![b": foo".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: ": 1:x;bar".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          fields: vec![b": 1:x;bar".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: ": 1:2bar;baz".into(),
            timestamp_ns: 3,
            ..Default::default()
          },
          fields: vec![b": 1:2bar;baz".to_vec()],
          scheme: b"plain".to_vec(),
        },
      ],
    );
  }

  #[test]
  fn duration_overflow() {
    assert_eq!(
      Zsh::parse(b": 1:9223372037;foo").unwrap_err().to_string(),
      "duration on history line 1 overflows nanoseconds",
    );
  }

  #[test]
  fn empty_input() {
    assert_eq!(Zsh::parse(b"").unwrap(), Vec::new());
  }

  #[test]
  fn extended_history() {
    assert_eq!(
      Zsh::parse(b": 1:2;foo").unwrap(),
      vec![ImportedExecution {
        execution: Execution {
          command: "foo".into(),
          duration_ns: Some(2_000_000_000),
          timestamp_ns: 1_000_000_000,
          ..Default::default()
        },
        fields: vec![
          b"foo".to_vec(),
          1_000_000_000_i64.to_be_bytes().to_vec(),
          2_000_000_000_i64.to_be_bytes().to_vec(),
        ],
        scheme: b"extended".to_vec(),
      }],
    );
  }

  #[test]
  fn invalid_utf8() {
    assert_eq!(
      Zsh::parse(&[0xFF]).unwrap_err().to_string(),
      "Zsh history is not valid UTF-8",
    );
  }

  #[test]
  fn mixed_history() {
    assert_eq!(
      Zsh::parse(b"foo\n: 2:3;bar\nbaz").unwrap(),
      vec![
        ImportedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          fields: vec![b"foo".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "bar".into(),
            duration_ns: Some(3_000_000_000),
            timestamp_ns: 2_000_000_000,
            ..Default::default()
          },
          fields: vec![
            b"bar".to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
            3_000_000_000_i64.to_be_bytes().to_vec(),
          ],
          scheme: b"extended".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "baz".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          fields: vec![b"baz".to_vec()],
          scheme: b"plain".to_vec(),
        },
      ],
    );
  }

  #[test]
  fn multiline_entries() {
    assert_eq!(
      Zsh::parse(b"foo \\\nbar\n: 1:2;baz \\\nqux\n").unwrap(),
      vec![
        ImportedExecution {
          execution: Execution {
            command: "foo \nbar".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          fields: vec![b"foo \nbar".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "baz \nqux".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          fields: vec![
            b"baz \nqux".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
          scheme: b"extended".to_vec(),
        },
      ],
    );
  }

  #[test]
  fn parsing_is_deterministic() {
    let contents = b"foo\n: 1:2;bar\nfoo";

    assert_eq!(Zsh::parse(contents).unwrap(), Zsh::parse(contents).unwrap());
  }

  #[test]
  fn plain_history() {
    assert_eq!(
      Zsh::parse(b"foo\nbar").unwrap(),
      vec![
        ImportedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          fields: vec![b"foo".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "bar".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          fields: vec![b"bar".to_vec()],
          scheme: b"plain".to_vec(),
        },
      ],
    );
  }

  #[test]
  fn repeated_commands() {
    let records = Zsh::parse(b"foo\nfoo\n: 1:2;foo\n: 1:2;foo").unwrap();

    assert_eq!(
      records,
      vec![
        ImportedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          fields: vec![b"foo".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          fields: vec![b"foo".to_vec()],
          scheme: b"plain".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          fields: vec![
            b"foo".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
          scheme: b"extended".to_vec(),
        },
        ImportedExecution {
          execution: Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          fields: vec![
            b"foo".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
          scheme: b"extended".to_vec(),
        },
      ],
    );
  }

  #[test]
  fn timestamp_overflow() {
    assert_eq!(
      Zsh::parse(b": 9223372037:1;foo").unwrap_err().to_string(),
      "timestamp on history line 1 overflows nanoseconds",
    );
  }
}
