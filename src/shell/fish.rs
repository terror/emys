use super::*;

pub(super) mod parser;

pub(super) const DEFAULT_HISTORY_FILE: &str = ".local/share/fish/fish_history";
pub(super) const FORMAT: &str = "fish";
pub(super) const INIT: &str = include_str!("fish/init.fish");
pub(super) const NAME: &str = "Fish";
