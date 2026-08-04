#[derive(Debug, Eq, PartialEq)]
pub(super) struct Line {
  pub(super) bytes: Vec<u8>,
  pub(super) number: usize,
  pub(super) terminated: bool,
}
