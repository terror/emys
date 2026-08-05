use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Command {
  pub(crate) directory: Option<PathBuf>,
  pub(crate) exit_code: Option<i32>,
  pub(crate) text: String,
  pub(crate) timestamp_ns: i64,
}

impl Command {
  pub(crate) fn directory_name(&self) -> Option<Cow<'_, str>> {
    self.directory.as_deref().map(|directory| {
      directory
        .file_name()
        .unwrap_or(directory.as_os_str())
        .to_string_lossy()
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn directory_name() {
    #[track_caller]
    fn case(directory: Option<&str>, expected: Option<&str>) {
      assert_eq!(
        Command {
          directory: directory.map(PathBuf::from),
          ..Default::default()
        }
        .directory_name()
        .as_deref(),
        expected,
      );
    }

    case(None, None);
    case(Some("foo/bar"), Some("bar"));
    case(Some("/"), Some("/"));
  }
}
