use super::*;

const PROGRESS_CHARS: &str = "█▉▊▋▌▍▎▏ ";

const PROGRESS_STYLE: &str = "{spinner:.green} ⟪{elapsed_precise}⟫ \
  ⟦{wide_bar:.cyan}⟧ {binary_bytes}/{binary_total_bytes} \
  ⟨{binary_bytes_per_sec}, {eta}⟩ {msg}";

const SPINNER_STYLE: &str = "{spinner:.green} ⟪{elapsed_precise}⟫ \
  {binary_bytes} ⟨{binary_bytes_per_sec}⟩ {msg}";

const TICK_CHARS: &str = concat!(
  "⠀⠁⠂⠃⠄⠅⠆⠇⡀⡁⡂⡃⡄⡅⡆⡇",
  "⠈⠉⠊⠋⠌⠍⠎⠏⡈⡉⡊⡋⡌⡍⡎⡏",
  "⠐⠑⠒⠓⠔⠕⠖⠗⡐⡑⡒⡓⡔⡕⡖⡗",
  "⠘⠙⠚⠛⠜⠝⠞⠟⡘⡙⡚⡛⡜⡝⡞⡟",
  "⠠⠡⠢⠣⠤⠥⠦⠧⡠⡡⡢⡣⡤⡥⡦⡧",
  "⠨⠩⠪⠫⠬⠭⠮⠯⡨⡩⡪⡫⡬⡭⡮⡯",
  "⠰⠱⠲⠳⠴⠵⠶⠷⡰⡱⡲⡳⡴⡵⡶⡷",
  "⠸⠹⠺⠻⠼⠽⠾⠿⡸⡹⡺⡻⡼⡽⡾⡿",
  "⢀⢁⢂⢃⢄⢅⢆⢇⣀⣁⣂⣃⣄⣅⣆⣇",
  "⢈⢉⢊⢋⢌⢍⢎⢏⣈⣉⣊⣋⣌⣍⣎⣏",
  "⢐⢑⢒⢓⢔⢕⢖⢗⣐⣑⣒⣓⣔⣕⣖⣗",
  "⢘⢙⢚⢛⢜⢝⢞⢟⣘⣙⣚⣛⣜⣝⣞⣟",
  "⢠⢡⢢⢣⢤⢥⢦⢧⣠⣡⣢⣣⣤⣥⣦⣧",
  "⢨⢩⢪⢫⢬⢭⢮⢯⣨⣩⣪⣫⣬⣭⣮⣯",
  "⢰⢱⢲⢳⢴⢵⢶⢷⣰⣱⣲⣳⣴⣵⣶⣷",
  "⢸⢹⢺⢻⢼⢽⢾⢿⣸⣹⣺⣻⣼⣽⣾⣿",
);

pub(super) struct Progress {
  bar: Option<ProgressBar>,
  name: &'static str,
}

impl Progress {
  const UPDATE_INTERVAL: usize = 256;

  pub(super) fn finish(self) {
    if let Some(bar) = self.bar {
      bar.finish_and_clear();
    }
  }

  pub(super) fn new(name: &'static str, length: Option<u64>) -> Result<Self> {
    let bar = if io::stderr().is_terminal() {
      let (bar, style) = if let Some(length) = length {
        (
          ProgressBar::new(length),
          ProgressStyle::with_template(PROGRESS_STYLE)?
            .progress_chars(PROGRESS_CHARS),
        )
      } else {
        (
          ProgressBar::new_spinner(),
          ProgressStyle::with_template(SPINNER_STYLE)?,
        )
      };

      bar.set_style(style.tick_chars(TICK_CHARS));
      bar.set_message(format!("{name}: 0 scanned, 0 new"));

      Some(bar)
    } else {
      None
    };

    Ok(Self { bar, name })
  }

  pub(super) fn reader(&self, reader: impl Read + 'static) -> Box<dyn Read> {
    if let Some(bar) = &self.bar {
      Box::new(bar.wrap_read(reader))
    } else {
      Box::new(reader)
    }
  }

  pub(super) fn update(&self, status: ImportProgress) {
    if !status.processed.is_multiple_of(Self::UPDATE_INTERVAL) {
      return;
    }

    let Some(bar) = &self.bar else {
      return;
    };

    bar.set_message(format!(
      "{}: {} scanned, {} new",
      self.name, status.processed, status.inserted,
    ));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn styles_are_valid() {
    ProgressStyle::with_template(PROGRESS_STYLE).unwrap();
    ProgressStyle::with_template(SPINNER_STYLE).unwrap();
  }
}
