#[derive(Clone, Copy)]
pub(crate) struct ProgressEntry {
  pub(crate) inserted: usize,
  pub(crate) processed: usize,
  pub(crate) total: usize,
}
