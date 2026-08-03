use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Execution {
  pub(crate) command: String,
  pub(crate) directory: Option<PathBuf>,
  pub(crate) duration_ns: Option<i64>,
  pub(crate) exit_code: Option<i32>,
  pub(crate) hostname: Option<String>,
  pub(crate) session: Option<String>,
  pub(crate) shell: Option<String>,
  pub(crate) timestamp_ns: i64,
}
