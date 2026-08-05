use super::*;

const SPINNER_STYLE: &str = "{spinner:.green} ⟪{elapsed_precise}⟫ \
  {binary_bytes} ⟨{binary_bytes_per_sec}⟩ {msg}";

pub(super) struct Progress {
  bar: Option<ProgressBar>,
}

impl Progress {
  pub(super) fn finish(self) {
    if let Some(bar) = self.bar {
      bar.finish_and_clear();
    }
  }

  pub(super) fn new(message: impl Into<Cow<'static, str>>) -> Result<Self> {
    let bar = if io::stderr().is_terminal() {
      let bar = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template(SPINNER_STYLE)?)
        .with_message(message);

      bar.enable_steady_tick(Duration::from_millis(50));

      Some(bar)
    } else {
      None
    };

    Ok(Self { bar })
  }

  pub(super) fn reader(&self, reader: impl Read + 'static) -> Box<dyn Read> {
    if let Some(bar) = &self.bar {
      Box::new(bar.wrap_read(reader))
    } else {
      Box::new(reader)
    }
  }

  pub(super) fn set_message(&self, message: impl Into<Cow<'static, str>>) {
    if let Some(bar) = &self.bar {
      bar.set_message(message);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn styles_are_valid() {
    ProgressStyle::with_template(SPINNER_STYLE).unwrap();
  }
}
