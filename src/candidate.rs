use super::*;

pub(crate) struct Candidate {
  command: Command,
  now_ns: i64,
}

impl Candidate {
  const DIRECTORY_WIDTH: usize = 9;
  const EARLIEST_TIMESTAMP_NS: i64 = 1_000_000_000_000_000_000;
  const EXIT_CODE_WIDTH: usize = 5;

  fn command(&self) -> String {
    self
      .command
      .text
      .chars()
      .map(|character| {
        if character.is_control() {
          ' '
        } else {
          character
        }
      })
      .collect()
  }

  fn directory(&self) -> String {
    self
      .command
      .directory
      .as_deref()
      .map(|directory| {
        directory
          .file_name()
          .unwrap_or(directory.as_os_str())
          .to_string_lossy()
          .into_owned()
      })
      .map(|directory| {
        if unicode_display_width::width(&directory)
          <= u64::try_from(Self::DIRECTORY_WIDTH).unwrap()
        {
          return directory;
        }

        let mut truncated = String::new();
        let width = u64::try_from(Self::DIRECTORY_WIDTH).unwrap() - 1;
        let mut used = 0;

        for grapheme in directory.graphemes(true) {
          let grapheme_width = unicode_display_width::width(grapheme);

          if used + grapheme_width > width {
            break;
          }

          truncated.push_str(grapheme);
          used += grapheme_width;
        }

        truncated.push('…');
        truncated
      })
      .unwrap_or_default()
  }

  fn directory_column(&self) -> String {
    let mut directory = self.directory();
    let width =
      usize::try_from(unicode_display_width::width(&directory)).unwrap();

    directory.push_str(&" ".repeat(Self::DIRECTORY_WIDTH - width));
    directory
  }

  fn exit_code(&self) -> String {
    self
      .command
      .exit_code
      .filter(|code| *code != 0)
      .map(|code| format!("[{code}]"))
      .unwrap_or_default()
  }

  pub(crate) fn new(command: Command, now_ns: i64) -> Self {
    Self { command, now_ns }
  }

  fn relative_age(&self) -> String {
    const DAY: i64 = 24 * HOUR;
    const HOUR: i64 = 60 * MINUTE;
    const MINUTE: i64 = 60 * SECOND;
    const MONTH: i64 = 30 * DAY;
    const SECOND: i64 = 1_000_000_000;
    const YEAR: i64 = 365 * DAY;

    if self.command.timestamp_ns < Self::EARLIEST_TIMESTAMP_NS {
      return String::new();
    }

    let age = self.now_ns.saturating_sub(self.command.timestamp_ns).max(0);

    if age < MINUTE {
      format!("{}s", age / SECOND)
    } else if age < HOUR {
      format!("{}m", age / MINUTE)
    } else if age < DAY {
      format!("{}h", age / HOUR)
    } else if age < MONTH {
      format!("{}d", age / DAY)
    } else if age < YEAR {
      format!("{}mo", age / MONTH)
    } else {
      format!("{}y", age / YEAR)
    }
  }

  #[cfg(test)]
  fn row(&self) -> String {
    format!(
      "{:>4}  {}  {:<exit_code_width$}  {}",
      self.relative_age(),
      self.directory_column(),
      self.exit_code(),
      self.command(),
      exit_code_width = Self::EXIT_CODE_WIDTH,
    )
  }
}

impl SkimItem for Candidate {
  fn display(&self, context: DisplayContext) -> ratatui::text::Line<'_> {
    let metadata = context
      .base_style
      .remove_modifier(Modifier::BOLD)
      .add_modifier(Modifier::DIM);

    let mut line = ratatui::text::Line::from(Span::styled(
      format!("{:>4}  {}  ", self.relative_age(), self.directory_column()),
      metadata,
    ));

    line.spans.push(Span::styled(
      format!(
        "{:<width$}  ",
        self.exit_code(),
        width = Self::EXIT_CODE_WIDTH,
      ),
      metadata.fg(Color::Red),
    ));

    line
      .spans
      .extend(context.to_line(Cow::Owned(self.command())).spans);

    line
  }

  fn output(&self) -> Cow<'_, str> {
    Cow::Borrowed(&self.command.text)
  }

  fn text(&self) -> Cow<'_, str> {
    Cow::Borrowed(&self.command.text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const NOW: i64 = 2_000_000_000_000_000_000;

  fn candidate(command: Command) -> Candidate {
    Candidate::new(command, NOW)
  }

  #[test]
  fn display_aligns_age_directory_command_and_nonzero_exit_code() {
    let item = candidate(Command {
      directory: Some("/foo".into()),
      exit_code: Some(1),
      text: "bar".into(),
      timestamp_ns: NOW - 12_000_000_000,
    });

    assert_eq!(item.row(), " 12s  foo        [1]    bar");
    assert_eq!(
      item.display(DisplayContext::default()).to_string(),
      item.row()
    );

    let line = item.display(DisplayContext {
      base_style: ratatui::style::Style::default().add_modifier(Modifier::BOLD),
      ..Default::default()
    });

    assert!(line.spans[0].style.add_modifier.contains(Modifier::DIM));
    assert!(!line.spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert!(line.spans[1].style.add_modifier.contains(Modifier::DIM));
    assert_eq!(line.spans[1].style.fg, Some(Color::Red));
    assert!(!line.spans[2].style.add_modifier.contains(Modifier::DIM));
    assert!(line.spans[2].style.add_modifier.contains(Modifier::BOLD));
  }

  #[test]
  fn display_ellipsizes_directory_by_terminal_width() {
    #[track_caller]
    fn case(directory: &str, expected: &str) {
      assert_eq!(
        candidate(Command {
          directory: Some(directory.into()),
          ..Default::default()
        })
        .directory(),
        expected,
      );
    }

    case("/foobarbazqux", "foobarba…");
    case("/foo界界界界", "foo界界…");
  }

  #[test]
  fn display_hides_zero_exit_code_and_uses_cwd_basename() {
    assert_eq!(
      candidate(Command {
        directory: Some("/foo/baz".into()),
        exit_code: Some(0),
        text: "bar".into(),
        timestamp_ns: NOW - 8 * 60 * 1_000_000_000,
      })
      .row(),
      "  8m  baz               bar",
    );
  }

  #[test]
  fn display_keeps_multiline_commands_on_one_row() {
    assert_eq!(
      candidate(Command {
        text: "foo\n\tbar".into(),
        timestamp_ns: NOW,
        ..Default::default()
      })
      .row(),
      "  0s                    foo  bar",
    );
  }

  #[test]
  fn display_preserves_empty_metadata_columns() {
    let item = candidate(Command {
      text: "foo".into(),
      timestamp_ns: 1,
      ..Default::default()
    });

    assert_eq!(item.row(), "                        foo");
    assert_eq!(
      item.display(DisplayContext::default()).to_string(),
      "                        foo",
    );
  }

  #[test]
  fn relative_age_uses_compact_units() {
    #[track_caller]
    fn case(age_seconds: i64, expected: &str) {
      assert_eq!(
        candidate(Command {
          timestamp_ns: NOW - age_seconds * 1_000_000_000,
          ..Default::default()
        })
        .relative_age(),
        expected,
      );
    }

    case(12, "12s");
    case(8 * 60, "8m");
    case(3 * 60 * 60, "3h");
    case(4 * 24 * 60 * 60, "4d");
    case(5 * 30 * 24 * 60 * 60, "5mo");
    case(2 * 365 * 24 * 60 * 60, "2y");

    assert_eq!(
      candidate(Command {
        timestamp_ns: NOW + 1,
        ..Default::default()
      })
      .relative_age(),
      "0s",
    );

    assert_eq!(
      candidate(Command {
        timestamp_ns: 1,
        ..Default::default()
      })
      .relative_age(),
      "",
    );
  }

  #[test]
  fn skim_item_matches_and_outputs_original_command() {
    let item = candidate(Command {
      directory: Some("/baz".into()),
      exit_code: Some(1),
      text: "foo\nbar".into(),
      ..Default::default()
    });

    assert_eq!(item.text(), "foo\nbar");
    assert_eq!(item.output(), "foo\nbar");
  }
}
