use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SearchEntry {
  pub(crate) command: String,
  pub(crate) directory: Option<PathBuf>,
  pub(crate) exit_code: Option<i32>,
  pub(crate) timestamp_ns: i64,
}
