use super::*;

pub(super) trait Parser {
  fn finish(&mut self) -> Result<Option<Record>>;

  fn parse(&mut self, line: Line) -> Result<Option<Record>>;
}

impl<T: Parser + ?Sized> Parser for Box<T> {
  fn finish(&mut self) -> Result<Option<Record>> {
    T::finish(self)
  }

  fn parse(&mut self, line: Line) -> Result<Option<Record>> {
    T::parse(self, line)
  }
}
