use super::*;

#[derive(Default)]
pub struct Execution {
  pub command: String,
  pub timestamp_ns: i64,
  pub duration_ns: Option<i64>,
  pub exit_code: Option<i32>,
  pub directory: Option<PathBuf>,
  pub session: Option<String>,
  pub hostname: Option<String>,
  pub shell: Option<String>,
}
