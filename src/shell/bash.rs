use super::*;

pub(super) mod parser;

pub(super) const DEFAULT_HISTORY_FILE: &str = ".bash_history";
pub(super) const FORMAT: &str = "bash";
pub(super) const INIT: &str = include_str!("bash/init.bash");
pub(super) const NAME: &str = "Bash";
