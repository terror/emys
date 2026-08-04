use super::*;

pub(super) struct Entries<R, P> {
  done: bool,
  lines: Lines<R>,
  parser: P,
}

impl<R: Read, P: Parser> Entries<R, P> {
  pub(super) fn new(reader: R, parser: P) -> Self {
    Self {
      done: false,
      lines: Lines::new(reader),
      parser,
    }
  }
}

impl<R: Read, P: Parser> Iterator for Entries<R, P> {
  type Item = Result<Entry>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.done {
      return None;
    }

    loop {
      let result = match self.lines.next() {
        Some(Ok(line)) => self.parser.parse(line),
        Some(Err(error)) => Err(error),
        None => {
          self.done = true;
          self.parser.finish()
        }
      };

      match result {
        Ok(Some(entry)) => return Some(Ok(entry)),
        Ok(None) if self.done => return None,
        Ok(None) => {}
        Err(error) => {
          self.done = true;
          return Some(Err(error));
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Default)]
  struct TestParser(Vec<u8>);

  impl Parser for TestParser {
    const FORMAT: &'static str = "test";

    fn finish(&mut self) -> Result<Option<Entry>> {
      Ok(Some(Entry {
        execution: Execution {
          command: String::from_utf8(mem::take(&mut self.0)).unwrap(),
          ..Default::default()
        },
        identity: Identity {
          fields: Vec::new(),
          scheme: Vec::new(),
        },
      }))
    }

    fn parse(&mut self, line: Line) -> Result<Option<Entry>> {
      self.0.extend(line.bytes);

      if line.terminated {
        self.0.push(b'\n');
      }

      Ok(None)
    }
  }

  #[test]
  fn entries_flush_parser_at_eof() {
    assert_eq!(
      Entries::new(&b"foo\nbar"[..], TestParser(Vec::new()))
        .collect::<Result<Vec<_>>>()
        .unwrap()[0]
        .execution
        .command,
      "foo\nbar",
    );
  }
}
