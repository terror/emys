use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Entry {
  pub(crate) command: String,
  pub(crate) directory: Option<PathBuf>,
  pub(crate) duration_ns: Option<i64>,
  pub(crate) exit_code: Option<i32>,
  pub(crate) hostname: Option<String>,
  pub(crate) session: Option<String>,
  pub(crate) shell: Option<String>,
  pub(crate) timestamp_ns: i64,
}

impl Entry {
  pub(crate) fn directory(&self) -> Result<Option<&str>> {
    self
      .directory
      .as_deref()
      .map(|directory| {
        directory
          .to_str()
          .context("entry directory is not valid UTF-8")
      })
      .transpose()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn directory() {
    assert_eq!(Entry::default().directory().unwrap(), None);

    assert_eq!(
      Entry {
        directory: Some("foo".into()),
        ..Default::default()
      }
      .directory()
      .unwrap(),
      Some("foo"),
    );
  }
}
