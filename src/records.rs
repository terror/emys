use super::*;

pub(super) struct Records<R, P> {
  done: bool,
  lines: Lines<R>,
  parser: P,
}

impl<R: Read, P: Parser> Records<R, P> {
  pub(super) fn new(reader: R, parser: P) -> Self {
    Self {
      done: false,
      lines: Lines::new(reader),
      parser,
    }
  }
}

impl<R: Read, P: Parser> Iterator for Records<R, P> {
  type Item = Result<Record>;

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
        Ok(Some(record)) => return Some(Ok(record)),
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
    fn finish(&mut self) -> Result<Option<Record>> {
      Ok(Some(Record::new(
        Execution {
          command: String::from_utf8(mem::take(&mut self.0)).unwrap(),
          ..Default::default()
        },
        b"",
        Vec::<Vec<u8>>::new(),
      )))
    }

    fn parse(&mut self, line: Line) -> Result<Option<Record>> {
      self.0.extend(line.bytes);

      if line.terminated {
        self.0.push(b'\n');
      }

      Ok(None)
    }
  }

  #[test]
  fn records_flush_parser_at_eof() {
    assert_eq!(
      Records::new(&b"foo\nbar"[..], TestParser(Vec::new()))
        .collect::<Result<Vec<_>>>()
        .unwrap()[0]
        .execution
        .command,
      "foo\nbar",
    );
  }
}
