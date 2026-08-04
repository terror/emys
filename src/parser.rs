use super::*;

mod zsh;

pub(super) use zsh::Zsh;

pub(super) trait Parser: Default {
  const FORMAT: &'static str;

  fn decode(reader: impl Read) -> impl Read {
    reader
  }

  fn entries(
    reader: impl Read,
  ) -> impl Iterator<Item = Result<ParsedExecution>> {
    Entries::new(Self::decode(reader), Self::default())
  }

  fn finish(&mut self) -> Result<Option<ParsedExecution>>;

  fn parse(&mut self, line: Line) -> Result<Option<ParsedExecution>>;
}
