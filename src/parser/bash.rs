use super::*;

const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Default)]
pub(crate) struct Bash {
  command: Vec<u8>,
  command_line: Option<usize>,
  plain_timestamp_ns: i64,
  timestamp: Option<(Vec<u8>, usize)>,
}

impl Bash {
  fn complete(&mut self) -> Result<Option<Record>> {
    self.command_line = None;

    if self.command.is_empty() {
      return Ok(None);
    }

    let command =
      String::from_utf8_lossy(&mem::take(&mut self.command)).into_owned();

    if let Some((timestamp, line)) = &self.timestamp {
      let timestamp_ns = str::from_utf8(timestamp)
        .unwrap()
        .parse::<u64>()
        .ok()
        .and_then(|timestamp| {
          timestamp
            .checked_mul(NANOSECONDS_PER_SECOND)
            .and_then(|timestamp| i64::try_from(timestamp).ok())
        })
        .with_context(|| {
          format!("timestamp on history line {line} overflows nanoseconds")
        })?;

      Ok(Some(Record::new(
        Execution {
          command: command.clone(),
          timestamp_ns,
          ..Default::default()
        },
        b"timestamped",
        [
          command.as_bytes().to_vec(),
          timestamp_ns.to_be_bytes().to_vec(),
        ],
      )))
    } else {
      self.plain_timestamp_ns = self
        .plain_timestamp_ns
        .checked_add(1)
        .context("plain history timestamp exceeds SQLite integer range")?;

      Ok(Some(Record::new(
        Execution {
          command: command.clone(),
          timestamp_ns: self.plain_timestamp_ns,
          ..Default::default()
        },
        b"plain",
        [command.as_bytes()],
      )))
    }
  }

  fn timestamp(line: &[u8]) -> Option<&[u8]> {
    let timestamp = line.strip_prefix(b"#")?;

    let length = timestamp
      .iter()
      .take_while(|byte| byte.is_ascii_digit())
      .count();

    if length == 0 {
      return None;
    }

    Some(&timestamp[..length])
  }
}

impl Parser for Bash {
  const FORMAT: &'static str = "bash";

  fn finish(&mut self) -> Result<Option<Record>> {
    self.complete()
  }

  fn parse(&mut self, line: Line) -> Result<Option<Record>> {
    if let Some(timestamp) = Self::timestamp(&line.bytes) {
      let record = self.complete()?;

      self.timestamp = Some((timestamp.into(), line.number));

      return Ok(record);
    }

    if self.timestamp.is_some() && self.command_line.is_some() {
      self.command.push(b'\n');
    }

    self.command_line.get_or_insert(line.number);

    self.command.extend(line.bytes);

    if self.timestamp.is_some() {
      Ok(None)
    } else {
      self.complete()
    }
  }
}

#[cfg(test)]
mod tests {
  use {super::*, indoc::indoc};

  #[test]
  fn empty_input_and_entries_are_ignored() {
    #[track_caller]
    fn case(history: &[u8]) {
      assert_eq!(
        Bash::records(history).collect::<Result<Vec<_>>>().unwrap(),
        Vec::new(),
      );
    }

    case(b"");
    case(b"\n");
    case(b"#1");
    case(b"#1\n#2");
  }

  #[test]
  fn history() {
    assert_eq!(
      Bash::records(
        indoc! {
          b"
          foo
          #1
          bar
          baz
          #2
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
            command: "foo".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b"foo".as_slice()],
        ),
        Record::new(
          Execution {
            command: "bar\nbaz".into(),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
          b"timestamped",
          [
            b"bar\nbaz".as_slice(),
            1_000_000_000_i64.to_be_bytes().as_slice(),
          ],
        ),
        Record::new(
          Execution {
            command: "qux".into(),
            timestamp_ns: 2_000_000_000,
            ..Default::default()
          },
          b"timestamped",
          [
            b"qux".as_slice(),
            2_000_000_000_i64.to_be_bytes().as_slice(),
          ],
        ),
      ],
    );
  }

  #[test]
  fn invalid_utf8_is_lossy() {
    assert_eq!(
      Bash::records(&[0xFF][..])
        .collect::<Result<Vec<_>>>()
        .unwrap()[0]
        .execution
        .command,
      "\u{FFFD}",
    );
  }

  #[test]
  fn non_timestamps_are_commands() {
    assert_eq!(
      Bash::records(&b"#\n#foo"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap()
        .into_iter()
        .map(|record| record.execution.command)
        .collect::<Vec<_>>(),
      ["#", "#foo"],
    );
  }

  #[test]
  fn timestamp_allows_trailing_text() {
    assert_eq!(
      Bash::records(&b"#1foo\nbar"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap()[0]
        .execution,
      Execution {
        command: "bar".into(),
        timestamp_ns: 1_000_000_000,
        ..Default::default()
      },
    );
  }

  #[test]
  fn timestamp_integer_overflow() {
    assert_eq!(
      Bash::records(&b"#18446744073709551616\nfoo"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap_err()
        .to_string(),
      "timestamp on history line 1 overflows nanoseconds",
    );
  }

  #[test]
  fn timestamp_overflow() {
    assert_eq!(
      Bash::records(&b"#9223372037\nfoo"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap_err()
        .to_string(),
      "timestamp on history line 1 overflows nanoseconds",
    );
  }
}
