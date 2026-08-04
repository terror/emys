use super::*;

const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const META: u8 = 0x83;

struct Metafied<R> {
  escaped: bool,
  reader: R,
}

impl<R: BufRead> Read for Metafied<R> {
  fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
    if output.is_empty() {
      return Ok(0);
    }

    let mut written = 0;

    while written < output.len() {
      let (consumed, exhausted) = {
        let input = self.reader.fill_buf()?;

        if input.is_empty() {
          (0, true)
        } else {
          let mut consumed = 0;

          for byte in input.iter().copied() {
            consumed += 1;

            if self.escaped {
              output[written] = byte ^ 0x20;
              written += 1;
              self.escaped = false;
            } else if byte == META {
              self.escaped = true;
            } else {
              output[written] = byte;
              written += 1;
            }

            if written == output.len() {
              break;
            }
          }

          (consumed, false)
        }
      };

      self.reader.consume(consumed);

      if exhausted {
        if self.escaped {
          if written > 0 {
            return Ok(written);
          }

          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zsh history ends with an incomplete metafied byte",
          ));
        }

        break;
      }
    }

    Ok(written)
  }
}

#[derive(Default)]
pub(crate) struct Zsh {
  command: Vec<u8>,
  command_line: Option<usize>,
  plain_timestamp_ns: i64,
}

impl Zsh {
  fn complete(&mut self) -> Result<Option<Record>> {
    let Some(line) = self.command_line.take() else {
      return Ok(None);
    };

    if self.command.is_empty() {
      return Ok(None);
    }

    let command =
      String::from_utf8_lossy(&mem::take(&mut self.command)).into_owned();

    let nanoseconds = |value: &str, field: &str| -> Result<i64> {
      value
        .parse::<u64>()
        .ok()
        .and_then(|value| value.checked_mul(NANOSECONDS_PER_SECOND))
        .and_then(|value| i64::try_from(value).ok())
        .with_context(|| {
          format!("{field} on history line {line} overflows nanoseconds")
        })
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
      if command.is_empty() {
        return Ok(None);
      }

      let timestamp_ns = nanoseconds(timestamp, "timestamp")?;

      let duration_ns = nanoseconds(duration, "duration")?;

      Ok(Some(Record::new(
        Execution {
          command: command.into(),
          duration_ns: Some(duration_ns),
          timestamp_ns,
          ..Default::default()
        },
        b"extended",
        [
          command.as_bytes().to_vec(),
          timestamp_ns.to_be_bytes().to_vec(),
          duration_ns.to_be_bytes().to_vec(),
        ],
      )))
    } else {
      self.plain_timestamp_ns = self
        .plain_timestamp_ns
        .checked_add(1)
        .context("plain history timestamp exceeds SQLite integer range")?;

      let components = vec![command.as_bytes().to_vec()];

      Ok(Some(Record::new(
        Execution {
          command,
          timestamp_ns: self.plain_timestamp_ns,
          ..Default::default()
        },
        b"plain",
        components,
      )))
    }
  }
}

impl Parser for Zsh {
  const FORMAT: &'static str = "zsh";

  fn decode(reader: impl Read) -> impl Read {
    Metafied {
      escaped: false,
      reader: BufReader::new(reader),
    }
  }

  fn finish(&mut self) -> Result<Option<Record>> {
    self.complete()
  }

  fn parse(&mut self, mut line: Line) -> Result<Option<Record>> {
    let continued = line.terminated && line.bytes.ends_with(b"\\");

    if continued {
      line.bytes.pop();
    }

    self.command_line.get_or_insert(line.number);

    self.command.extend(line.bytes);

    if continued {
      self.command.push(b'\n');
      Ok(None)
    } else {
      self.complete()
    }
  }
}

#[cfg(test)]
mod tests {
  use {super::*, indoc::indoc};

  struct OneByte(io::Cursor<Vec<u8>>);

  impl Read for OneByte {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
      let length = output.len().min(1);
      self.0.read(&mut output[..length])
    }
  }

  #[test]
  fn commands_beginning_with_colon() {
    assert_eq!(
      Zsh::records(
        indoc! {
          b"
          : foo
          : 1:x;bar
          : 1:2bar;baz
          "
        }
        .as_slice()
      )
      .collect::<Result<Vec<_>>>()
      .unwrap(),
      vec![
        Record::new(
          Execution {
            command: ": foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b": foo".to_vec()],
        ),
        Record::new(
          Execution {
            command: ": 1:x;bar".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          b"plain",
          [b": 1:x;bar".to_vec()],
        ),
        Record::new(
          Execution {
            command: ": 1:2bar;baz".into(),
            timestamp_ns: 3,
            ..Default::default()
          },
          b"plain",
          [b": 1:2bar;baz".to_vec()],
        ),
      ],
    );
  }

  #[test]
  fn duration_overflow() {
    assert_eq!(
      Zsh::records(&b": 1:9223372037;foo"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap_err()
        .to_string(),
      "duration on history line 1 overflows nanoseconds",
    );
  }

  #[test]
  fn empty_extended_command() {
    assert_eq!(
      Zsh::records(&b": 1:2;"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      Vec::new(),
    );
  }

  #[test]
  fn empty_input() {
    assert_eq!(
      Zsh::records(&b""[..]).collect::<Result<Vec<_>>>().unwrap(),
      Vec::new(),
    );
  }

  #[test]
  fn extended_history() {
    assert_eq!(
      Zsh::records(&b": 1:2;foo"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      vec![Record::new(
        Execution {
          command: "foo".into(),
          duration_ns: Some(2_000_000_000),
          timestamp_ns: 1_000_000_000,
          ..Default::default()
        },
        b"extended",
        [
          b"foo".to_vec(),
          1_000_000_000_i64.to_be_bytes().to_vec(),
          2_000_000_000_i64.to_be_bytes().to_vec(),
        ],
      )],
    );
  }

  #[test]
  fn incomplete_metafied_byte() {
    assert_eq!(
      Zsh::records(&[META][..])
        .collect::<Result<Vec<_>>>()
        .unwrap_err()
        .to_string(),
      "zsh history ends with an incomplete metafied byte",
    );
  }

  #[test]
  fn invalid_utf8_is_lossy() {
    assert_eq!(
      Zsh::records(&[0xFF][..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      vec![Record::new(
        Execution {
          command: "\u{FFFD}".into(),
          timestamp_ns: 1,
          ..Default::default()
        },
        b"plain",
        ["\u{FFFD}".as_bytes().to_vec()],
      )],
    );
  }

  #[test]
  fn metafied_bytes_cross_read_boundaries() {
    let contents = b"foo \xF0\x83\xBF\x83\xB8\x80";

    assert_eq!(
      Zsh::records(OneByte(io::Cursor::new(contents.to_vec())))
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      Zsh::records(&contents[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
    );
  }

  #[test]
  fn metafied_unicode() {
    assert_eq!(
      Zsh::records(&b"foo \xF0\x83\xBF\x83\xB8\x80"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      vec![Record::new(
        Execution {
          command: "foo \u{1F600}".into(),
          timestamp_ns: 1,
          ..Default::default()
        },
        b"plain",
        ["foo \u{1F600}".as_bytes().to_vec()],
      )],
    );
  }

  #[test]
  fn mixed_history() {
    assert_eq!(
      Zsh::records(
        indoc! {
          b"
          foo
          : 2:3;bar
          baz
          "
        }
        .as_slice()
      )
      .collect::<Result<Vec<_>>>()
      .unwrap(),
      vec![
        Record::new(
          Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b"foo".to_vec()],
        ),
        Record::new(
          Execution {
            command: "bar".into(),
            duration_ns: Some(3_000_000_000),
            timestamp_ns: 2_000_000_000,
            ..Default::default()
          },
          b"extended",
          [
            b"bar".to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
            3_000_000_000_i64.to_be_bytes().to_vec(),
          ],
        ),
        Record::new(
          Execution {
            command: "baz".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          b"plain",
          [b"baz".to_vec()],
        ),
      ],
    );
  }

  #[test]
  fn multiline_records() {
    assert_eq!(
      Zsh::records(
        indoc! {
          b"
          foo \x5C
          bar
          : 1:2;baz \x5C
          qux
          "
        }
        .as_slice()
      )
      .collect::<Result<Vec<_>>>()
      .unwrap(),
      vec![
        Record::new(
          Execution {
            command: "foo \nbar".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b"foo \nbar".to_vec()],
        ),
        Record::new(
          Execution {
            command: "baz \nqux".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          b"extended",
          [
            b"baz \nqux".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
        ),
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

    assert_eq!(
      Zsh::records(&contents[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      Zsh::records(&contents[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
    );
  }

  #[test]
  fn plain_history() {
    assert_eq!(
      Zsh::records(
        indoc! {
          b"
          foo
          bar
          "
        }
        .as_slice()
      )
      .collect::<Result<Vec<_>>>()
      .unwrap(),
      vec![
        Record::new(
          Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b"foo".to_vec()],
        ),
        Record::new(
          Execution {
            command: "bar".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          b"plain",
          [b"bar".to_vec()],
        ),
      ],
    );
  }

  #[test]
  fn repeated_commands() {
    assert_eq!(
      Zsh::records(
        indoc! {
          b"
          foo
          foo
          : 1:2;foo
          : 1:2;foo
          "
        }
        .as_slice()
      )
      .collect::<Result<Vec<_>>>()
      .unwrap(),
      vec![
        Record::new(
          Execution {
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b"foo".to_vec()],
        ),
        Record::new(
          Execution {
            command: "foo".into(),
            timestamp_ns: 2,
            ..Default::default()
          },
          b"plain",
          [b"foo".to_vec()],
        ),
        Record::new(
          Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          b"extended",
          [
            b"foo".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
        ),
        Record::new(
          Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          b"extended",
          [
            b"foo".to_vec(),
            1_000_000_000_i64.to_be_bytes().to_vec(),
            2_000_000_000_i64.to_be_bytes().to_vec(),
          ],
        ),
      ],
    );
  }

  #[test]
  fn timestamp_overflow() {
    assert_eq!(
      Zsh::records(&b": 9223372037:1;foo"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap_err()
        .to_string(),
      "timestamp on history line 1 overflows nanoseconds",
    );
  }
}
