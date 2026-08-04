use super::*;

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

  pub(super) fn new(name: &'static str) -> Result<Self> {
    let bar = if io::stderr().is_terminal() {
      let bar = ProgressBar::new_spinner()
        .with_style(
          ProgressStyle::with_template(SPINNER_STYLE)?.tick_chars(TICK_CHARS),
        )
        .with_message(format!("{name}: parsing"));

      bar.enable_steady_tick(Duration::from_millis(50));

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

  pub(super) fn update(&self, entry: ProgressEntry) {
    if !entry.processed.is_multiple_of(Self::UPDATE_INTERVAL) {
      return;
    }

    let Some(bar) = &self.bar else {
      return;
    };

    bar.set_message(format!(
      "{}: {} scanned, {} new",
      self.name, entry.processed, entry.inserted,
    ));
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
