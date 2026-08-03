use super::*;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ParsedExecution {
  pub(super) execution: Execution,
  pub(super) identity: Identity,
}
