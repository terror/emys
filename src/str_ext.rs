use super::*;

pub(crate) trait StrExt {
  fn truncate(&self, width: usize) -> Cow<'_, str>;
}

impl StrExt for str {
  fn truncate(&self, width: usize) -> Cow<'_, str> {
    let width = u64::try_from(width).unwrap();

    if unicode_display_width::width(self) <= width {
      return Cow::Borrowed(self);
    }

    let Some(width) = width.checked_sub(1) else {
      return Cow::Borrowed("");
    };

    let mut truncated = String::new();
    let mut used = 0;

    for grapheme in self.graphemes(true) {
      let grapheme_width = unicode_display_width::width(grapheme);

      if used + grapheme_width > width {
        break;
      }

      truncated.push_str(grapheme);
      used += grapheme_width;
    }

    truncated.push('…');

    Cow::Owned(truncated)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn truncate() {
    #[track_caller]
    fn case(input: &str, width: usize, expected: &str) {
      assert_eq!(input.truncate(width), expected);
    }

    case("foo", 3, "foo");
    case("foobar", 4, "foo…");
    case("foo界界", 6, "foo界…");
    case("foo界界", 5, "foo…");
    case("foo", 0, "");
  }
}
