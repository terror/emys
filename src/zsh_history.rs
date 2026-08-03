use {super::*, std::collections::HashMap};

const NAMESPACE: Uuid = Uuid::from_u128(0x81d6d1f2_748f_5b72_89b6_bf87e06a762d);
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

fn identifier(
  command: &str,
  occurrence: u64,
  timing: Option<(i64, i64)>,
) -> Uuid {
  let mut name = Vec::with_capacity(command.len() + 64);

  frame(
    &mut name,
    if timing.is_some() {
      b"extended"
    } else {
      b"plain"
    },
  );
  frame(&mut name, b"zsh");
  frame(&mut name, command.as_bytes());

  if let Some((timestamp_ns, duration_ns)) = timing {
    frame(&mut name, &timestamp_ns.to_be_bytes());
    frame(&mut name, &duration_ns.to_be_bytes());
  }

  frame(&mut name, &occurrence.to_be_bytes());

  Uuid::new_v5(&NAMESPACE, &name)
}

fn frame(destination: &mut Vec<u8>, value: &[u8]) {
  destination
    .extend_from_slice(&u64::try_from(value.len()).unwrap().to_be_bytes());
  destination.extend_from_slice(value);
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

pub(crate) fn parse(contents: &str) -> Result<Vec<(Uuid, Execution)>> {
  let mut extended_occurrences = HashMap::new();
  let mut plain_occurrences = HashMap::new();
  let mut plain_timestamp_ns = 0_i64;
  let mut records = Vec::new();

  for (line, command) in logical_entries(contents) {
    if let Some((timestamp_ns, duration_ns, command)) =
      parse_extended(&command, line)?
    {
      if command.is_empty() {
        continue;
      }

      let key = (command.to_owned(), timestamp_ns, duration_ns);
      let occurrence = extended_occurrences.entry(key).or_insert(0_u64);
      *occurrence = occurrence
        .checked_add(1)
        .context("extended history occurrence count overflowed")?;

      records.push((
        identifier(command, *occurrence, Some((timestamp_ns, duration_ns))),
        Execution {
          command: command.into(),
          duration_ns: Some(duration_ns),
          shell: Some("zsh".into()),
          timestamp_ns,
          ..Default::default()
        },
      ));
    } else {
      plain_timestamp_ns = plain_timestamp_ns
        .checked_add(1)
        .context("plain history timestamp exceeds SQLite integer range")?;
      let occurrence =
        plain_occurrences.entry(command.clone()).or_insert(0_u64);
      *occurrence = occurrence
        .checked_add(1)
        .context("plain history occurrence count overflowed")?;

      records.push((
        identifier(&command, *occurrence, None),
        Execution {
          command,
          shell: Some("zsh".into()),
          timestamp_ns: plain_timestamp_ns,
          ..Default::default()
        },
      ));
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
      parse(": foo\n: 1:x;bar\n: 1:2bar;baz").unwrap(),
      vec![
        (
          Uuid::parse_str("bae45eab-db31-54d0-b0fb-56b9470874ea").unwrap(),
          Execution {
            command: ": foo".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 1,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("506dad05-3ee9-514a-aa68-933536d64a00").unwrap(),
          Execution {
            command: ": 1:x;bar".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 2,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("fa333708-6a25-54aa-a223-cf2bbac86771").unwrap(),
          Execution {
            command: ": 1:2bar;baz".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 3,
            ..Default::default()
          },
        ),
      ],
    );
  }

  #[test]
  fn duration_overflow() {
    assert_eq!(
      parse(": 1:9223372037;foo").unwrap_err().to_string(),
      "duration on history line 1 overflows nanoseconds",
    );
  }

  #[test]
  fn empty_input() {
    assert_eq!(parse("").unwrap(), Vec::new());
  }

  #[test]
  fn extended_history() {
    assert_eq!(
      parse(": 1700000000:2;git status").unwrap(),
      vec![(
        Uuid::parse_str("24c87754-e0cb-5e30-9852-52d34462378a").unwrap(),
        Execution {
          command: "git status".into(),
          duration_ns: Some(2_000_000_000),
          shell: Some("zsh".into()),
          timestamp_ns: 1_700_000_000_000_000_000,
          ..Default::default()
        },
      )],
    );
  }

  #[test]
  fn mixed_history() {
    assert_eq!(
      parse("foo\n: 2:3;bar\nbaz").unwrap(),
      vec![
        (
          Uuid::parse_str("5701fc72-edbb-500d-9c84-ff46c43300fc").unwrap(),
          Execution {
            command: "foo".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 1,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("e4c7a59c-dad0-5290-9984-eb62b6289b02").unwrap(),
          Execution {
            command: "bar".into(),
            duration_ns: Some(3_000_000_000),
            shell: Some("zsh".into()),
            timestamp_ns: 2_000_000_000,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("11c39ab0-d0d3-5f8d-9e21-3458b92c6aef").unwrap(),
          Execution {
            command: "baz".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 2,
            ..Default::default()
          },
        ),
      ],
    );
  }

  #[test]
  fn multiline_entries() {
    assert_eq!(
      parse("foo \\\nbar\n: 1:2;baz \\\nqux\n").unwrap(),
      vec![
        (
          Uuid::parse_str("6f8e438c-8923-5c8e-aa92-f5bfb6239198").unwrap(),
          Execution {
            command: "foo \nbar".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 1,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("1a6c3a91-9e46-5cd9-bf26-c55bf76cc0d8").unwrap(),
          Execution {
            command: "baz \nqux".into(),
            duration_ns: Some(2_000_000_000),
            shell: Some("zsh".into()),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
        ),
      ],
    );
  }

  #[test]
  fn parsing_is_deterministic() {
    let contents = "foo\n: 1:2;bar\nfoo";

    assert_eq!(parse(contents).unwrap(), parse(contents).unwrap());
  }

  #[test]
  fn plain_history() {
    assert_eq!(
      parse("git status\ncargo test").unwrap(),
      vec![
        (
          Uuid::parse_str("b7868239-523b-5b1c-a66f-24f2ca04613c").unwrap(),
          Execution {
            command: "git status".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 1,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("674314a8-4e0f-50f5-9a0c-ac0546adfd5b").unwrap(),
          Execution {
            command: "cargo test".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 2,
            ..Default::default()
          },
        ),
      ],
    );
  }

  #[test]
  fn repeated_commands() {
    let records = parse("foo\nfoo\n: 1:2;foo\n: 1:2;foo").unwrap();

    assert_eq!(
      records,
      vec![
        (
          Uuid::parse_str("5701fc72-edbb-500d-9c84-ff46c43300fc").unwrap(),
          Execution {
            command: "foo".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 1,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("ea9febfd-f3f9-5203-a6dc-d827af73228c").unwrap(),
          Execution {
            command: "foo".into(),
            shell: Some("zsh".into()),
            timestamp_ns: 2,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("aebb3d50-be71-56f6-93d1-349e89dcd00b").unwrap(),
          Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            shell: Some("zsh".into()),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
        ),
        (
          Uuid::parse_str("7190a37f-1ada-57c8-9c4d-20427e3170ec").unwrap(),
          Execution {
            command: "foo".into(),
            duration_ns: Some(2_000_000_000),
            shell: Some("zsh".into()),
            timestamp_ns: 1_000_000_000,
            ..Default::default()
          },
        ),
      ],
    );
  }

  #[test]
  fn timestamp_overflow() {
    assert_eq!(
      parse(": 9223372037:1;foo").unwrap_err().to_string(),
      "timestamp on history line 1 overflows nanoseconds",
    );
  }
}
