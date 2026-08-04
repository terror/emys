use super::*;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Entry {
  pub(super) execution: Execution,
  pub(super) identity: Identity,
}
