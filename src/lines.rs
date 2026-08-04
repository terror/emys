use super::*;

pub(super) struct Lines<R> {
  done: bool,
  number: usize,
  reader: BufReader<R>,
}

impl<R: Read> Lines<R> {
  pub(super) fn new(reader: R) -> Self {
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

  #[test]
  fn preserves_numbers_and_termination() {
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
