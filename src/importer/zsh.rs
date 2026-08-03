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

  fn parse(contents: &[u8]) -> Result<Vec<ParsedExecution>> {
    let contents =
      str::from_utf8(contents).context("zsh history is not valid UTF-8")?;

    let nanoseconds = |value: &str, field: &str, line: usize| -> Result<i64> {
      value
        .parse::<u64>()
        .ok()
        .and_then(|value| value.checked_mul(NANOSECONDS_PER_SECOND))
        .and_then(|value| i64::try_from(value).ok())
        .with_context(|| {
          format!("{field} on history line {line} overflows nanoseconds")
        })
    };

    let (_, _, records) = contents
      .split_inclusive('\n')
      .enumerate()
      .map(Some)
      .chain(once(None))
      .try_fold(
        (
          None::<(usize, String)>,
          0_i64,
          Vec::<ParsedExecution>::new(),
        ),
        |(mut current, mut plain_timestamp_ns, mut records),
         physical|
         -> Result<_> {
          let completed = match physical {
            Some((index, physical)) => {
              let has_newline = physical.ends_with('\n');

              let physical = physical.strip_suffix('\n').unwrap_or(physical);

              let continued = has_newline && physical.ends_with('\\');

              let (_, command) =
                current.get_or_insert_with(|| (index + 1, String::new()));

              if continued {
                command.push_str(
                  physical
                    .strip_suffix('\\')
                    .expect("continued line ends with a backslash"),
                );

                command.push('\n');
                None
              } else {
                command.push_str(physical);

                current.take().filter(|(_, command)| !command.is_empty())
              }
            }

            None => current.take().filter(|(_, command)| !command.is_empty()),
          };

          let Some((line, command)) = completed else {
            return Ok((current, plain_timestamp_ns, records));
          };

          let extended = command
            .strip_prefix(": ")
            .and_then(|metadata| {
              let (timestamp, metadata) = metadata.split_once(':')?;
              let (duration, command) = metadata.split_once(';')?;
              Some((timestamp, duration, command))
            })
            .filter(|(timestamp, duration, _)| {
              !timestamp.is_empty()
                && !duration.is_empty()
                && timestamp.bytes().all(|byte| byte.is_ascii_digit())
                && duration.bytes().all(|byte| byte.is_ascii_digit())
            });

          if let Some((timestamp, duration, command)) = extended {
            if !command.is_empty() {
              let timestamp_ns = nanoseconds(timestamp, "timestamp", line)?;

              let duration_ns = nanoseconds(duration, "duration", line)?;

              records.push(ParsedExecution {
                execution: Execution {
                  command: command.into(),
                  duration_ns: Some(duration_ns),
                  timestamp_ns,
                  ..Default::default()
                },
                identity: Identity {
                  fields: vec![
                    command.as_bytes().to_vec(),
                    timestamp_ns.to_be_bytes().to_vec(),
                    duration_ns.to_be_bytes().to_vec(),
                  ],
                  scheme: b"extended".to_vec(),
                },
              });
            }
          } else {
            plain_timestamp_ns = plain_timestamp_ns.checked_add(1).context(
              "plain history timestamp exceeds SQLite integer range",
            )?;

            let fields = vec![command.as_bytes().to_vec()];

            records.push(ParsedExecution {
              execution: Execution {
                command,
                timestamp_ns: plain_timestamp_ns,
                ..Default::default()
              },
              identity: Identity {
                fields,
                scheme: b"plain".to_vec(),
              },
            });
          }

          Ok((current, plain_timestamp_ns, records))
        },
      )?;

    Ok(records)
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

#[cfg(test)]
mod tests {
  use {super::*, indoc::indoc};

  #[test]
  fn commands_beginning_with_colon() {
    assert_eq!(
      Zsh::parse(indoc! {
        b"
        : foo
        : 1:x;bar
        : 1:2bar;baz
        "
      })
      .unwrap(),
      vec![
        ParsedExecution {
          execution: Execution {
            command: ": foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b": foo".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: ": 1:x;bar".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b": 1:x;bar".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: ": 1:2bar;baz".into(),
            timestamp_ns: 3,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b": 1:2bar;baz".to_vec()],
            scheme: b"plain".to_vec(),
          },
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
      vec![ParsedExecution {
        execution: Execution {
          command: "foo".into(),
          duration_ns: Some(2_000_000_000),
          timestamp_ns: 1_000_000_000,
          ..Default::default()
        },
        identity: Identity {
          fields: vec![
            b"foo".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
          scheme: b"extended".to_vec(),
        },
      }],
    );
  }

  #[test]
  fn invalid_utf8() {
    assert_eq!(
      Zsh::parse(&[0xFF]).unwrap_err().to_string(),
      "zsh history is not valid UTF-8",
    );
  }

  #[test]
  fn mixed_history() {
    assert_eq!(
      Zsh::parse(indoc! {
        b"
        foo
        : 2:3;bar
        baz
        "
      })
      .unwrap(),
      vec![
        ParsedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"foo".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "bar".into(),
            duration_ns: Some(3_000_000_000),
            timestamp_ns: 2_000_000_000,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![
              b"bar".to_vec(),
              2_000_000_000_i64.to_be_bytes().to_vec(),
              3_000_000_000_i64.to_be_bytes().to_vec(),
            ],
            scheme: b"extended".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "baz".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"baz".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
      ],
    );
  }

  #[test]
  fn multiline_entries() {
    assert_eq!(
      Zsh::parse(indoc! {
        b"
        foo \x5C
        bar
        : 1:2;baz \x5C
        qux
        "
      })
      .unwrap(),
      vec![
        ParsedExecution {
          execution: Execution {
            command: "foo \nbar".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"foo \nbar".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "baz \nqux".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![
              b"baz \nqux".to_vec(),
              1_000_000_000_i64.to_be_bytes().to_vec(),
              2_000_000_000_i64.to_be_bytes().to_vec(),
            ],
            scheme: b"extended".to_vec(),
          },
        },
      ],
    );
  }

  #[test]
  fn parsing_is_deterministic() {
    let contents = indoc! {
      b"
      foo
      : 1:2;bar
      foo
      "
    };

    assert_eq!(Zsh::parse(contents).unwrap(), Zsh::parse(contents).unwrap());
  }

  #[test]
  fn plain_history() {
    assert_eq!(
      Zsh::parse(indoc! {b"
          foo
          bar
        "})
      .unwrap(),
      vec![
        ParsedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"foo".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "bar".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"bar".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
      ],
    );
  }

  #[test]
  fn repeated_commands() {
    assert_eq!(
      Zsh::parse(indoc! {
        b"
        foo
        foo
        : 1:2;foo
        : 1:2;foo
        "
      })
      .unwrap(),
      vec![
        ParsedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"foo".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "foo".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![b"foo".to_vec()],
            scheme: b"plain".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![
              b"foo".to_vec(),
              1_000_000_000_i64.to_be_bytes().to_vec(),
              2_000_000_000_i64.to_be_bytes().to_vec(),
            ],
            scheme: b"extended".to_vec(),
          },
        },
        ParsedExecution {
          execution: Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          identity: Identity {
            fields: vec![
              b"foo".to_vec(),
              1_000_000_000_i64.to_be_bytes().to_vec(),
              2_000_000_000_i64.to_be_bytes().to_vec(),
            ],
            scheme: b"extended".to_vec(),
          },
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
