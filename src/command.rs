use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Command {
  pub(crate) directory: Option<PathBuf>,
  pub(crate) exit_code: Option<i32>,
  pub(crate) text: String,
  pub(crate) timestamp_ns: i64,
}
