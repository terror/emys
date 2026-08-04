use super::*;

mod bash;
mod zsh;

pub(super) use {bash::Bash, zsh::Zsh};

pub(super) trait Parser: Default {
  const FORMAT: &'static str;

  fn decode(reader: impl Read) -> impl Read {
    reader
  }

  fn finish(&mut self) -> Result<Option<Record>>;

  fn parse(&mut self, line: Line) -> Result<Option<Record>>;

  fn records(reader: impl Read) -> impl Iterator<Item = Result<Record>> {
    Records::new(Self::decode(reader), Self::default())
  }
}
