use super::*;

pub(super) trait EntryParser {
  type Entry;

  fn finish(&mut self) -> Result<Option<Self::Entry>>;

  fn parse(&mut self, line: Line) -> Result<Option<Self::Entry>>;
}

pub(super) struct Entries<R, P> {
  done: bool,
  lines: Lines<R>,
  parser: P,
}

impl<R: Read, P: EntryParser> Entries<R, P> {
  pub(super) fn new(reader: R, parser: P) -> Self {
    Self {
      done: false,
      lines: Lines::new(reader),
      parser,
    }
  }
}

impl<R: Read, P: EntryParser> Iterator for Entries<R, P> {
  type Item = Result<P::Entry>;

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

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Line {
  pub(super) bytes: Vec<u8>,
  pub(super) number: usize,
  pub(super) terminated: bool,
}

struct Lines<R> {
  done: bool,
  number: usize,
  reader: BufReader<R>,
}

impl<R: Read> Lines<R> {
  fn new(reader: R) -> Self {
    Self {
      done: false,
      number: 0,
      reader: BufReader::new(reader),
    }
  }
}

impl<R: Read> Iterator for Lines<R> {
  type Item = Result<Line>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.done {
      return None;
    }

    let mut bytes = Vec::new();

    let read = match self.reader.read_until(b'\n', &mut bytes) {
      Ok(read) => read,
      Err(error) => {
        self.done = true;
        return Some(Err(error.into()));
      }
    };

    if read == 0 {
      self.done = true;
      return None;
    }

    self.number += 1;

    let terminated = bytes.ends_with(b"\n");

    if terminated {
      bytes.pop();
    }

    Some(Ok(Line {
      bytes,
      number: self.number,
      terminated,
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Parser(Vec<u8>);

  impl EntryParser for Parser {
    type Entry = Vec<u8>;

    fn finish(&mut self) -> Result<Option<Self::Entry>> {
      Ok(Some(mem::take(&mut self.0)))
    }

    fn parse(&mut self, line: Line) -> Result<Option<Self::Entry>> {
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
      Entries::new(&b"foo\nbar"[..], Parser(Vec::new()))
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      [b"foo\nbar".to_vec()],
    );
  }

  #[test]
  fn lines_preserve_numbers_and_termination() {
    assert_eq!(
      Lines::new(&b"foo\n\nbar"[..])
        .collect::<Result<Vec<_>>>()
        .unwrap(),
      [
        Line {
          bytes: b"foo".to_vec(),
          number: 1,
          terminated: true,
        },
        Line {
          bytes: Vec::new(),
          number: 2,
          terminated: true,
        },
        Line {
          bytes: b"bar".to_vec(),
          number: 3,
          terminated: false,
        },
      ],
    );
  }
}
