use super::*;

const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;

#[derive(Default)]
pub(crate) struct Parser {
  command: Option<Vec<u8>>,
  plain_timestamp_ns: i64,
  timestamp: Option<(Vec<u8>, usize)>,
}

impl Parser {
  fn complete(&mut self) -> Result<Option<Record>> {
    let Some(command) = self.command.take() else {
      return Ok(None);
    };

    if command.is_empty() {
      self.timestamp = None;
      return Ok(None);
    }

    let command = String::from_utf8_lossy(&command).into_owned();

    let timestamp = self.timestamp.take().and_then(|(timestamp, line)| {
      str::from_utf8(&timestamp)
        .ok()
        .and_then(|timestamp| timestamp.parse::<i64>().ok())
        .map(|timestamp| (timestamp, line))
    });

    if let Some((timestamp, line)) = timestamp {
      let timestamp_ns = timestamp
        .checked_mul(NANOSECONDS_PER_SECOND)
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

  fn unescape(bytes: &[u8]) -> Vec<u8> {
    let (mut bytes, mut unescaped) = (bytes.iter().copied(), Vec::new());

    while let Some(byte) = bytes.next() {
      if byte != b'\\' {
        unescaped.push(byte);
        continue;
      }

      match bytes.next() {
        Some(b'\\') => unescaped.push(b'\\'),
        Some(b'n') => unescaped.push(b'\n'),
        _ => break,
      }
    }

    unescaped
  }
}

impl crate::parser::Parser for Parser {
  fn finish(&mut self) -> Result<Option<Record>> {
    self.complete()
  }

  fn parse(&mut self, line: Line) -> Result<Option<Record>> {
    if let Some(command) = line.bytes.strip_prefix(b"- cmd:") {
      let completed = self.complete()?;

      self.command = Some(Self::unescape(command.trim_ascii_start()));

      return Ok(completed);
    }

    if self.command.is_none() {
      return Ok(None);
    }

    if let Some(timestamp) = line.bytes.strip_prefix(b"  when:") {
      self.timestamp = Some((timestamp.trim_ascii_start().into(), line.number));
    }

    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use {super::*, indoc::indoc};

  #[test]
  fn history() {
    assert_eq!(
      Shell::Fish
        .records(
          indoc! {
            b"
          - cmd: foo\\nbar\\\\baz
            when: 2
            paths:
              - /foo
          - cmd: qux
            added_when: 1
          "
          }
          .as_slice()
        )
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      vec![
        Record::new(
          Execution {
            command: "foo\nbar\\baz".into(),
            timestamp_ns: 2_000_000_000,
            ..Default::default()
          },
          b"timestamped",
          [
            b"foo\nbar\\baz".as_slice(),
            2_000_000_000_i64.to_be_bytes().as_slice(),
          ],
        ),
        Record::new(
          Execution {
            command: "qux".into(),
            timestamp_ns: 1,
            ..Default::default()
          },
          b"plain",
          [b"qux".as_slice()],
        ),
      ],
    );
  }

  #[test]
  fn invalid_records_are_ignored() {
    assert_eq!(
      Shell::Fish
        .records(&b"foo\n  when: 1\n- cmd:\n  when: 2"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      Vec::new(),
    );
  }

  #[test]
  fn invalid_timestamp_is_plain() {
    assert_eq!(
      Shell::Fish
        .records(&b"- cmd: foo\n  when: bar"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap()[0]
        .execution,
      Execution {
        command: "foo".into(),
        timestamp_ns: 1,
        ..Default::default()
      },
    );
  }

  #[test]
  fn invalid_utf8_is_lossy() {
    assert_eq!(
      Shell::Fish
        .records(&b"- cmd: \xFF"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap()[0]
        .execution
        .command,
      "\u{FFFD}",
    );
  }

  #[test]
  fn timestamp_overflow() {
    assert_eq!(
      Shell::Fish
        .records(&b"- cmd: foo\n  when: 9223372037"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap_err()
        .to_string(),
      "timestamp on history line 2 overflows nanoseconds",
    );
  }
}
