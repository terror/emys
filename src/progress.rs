use super::*;

const PROGRESS_CHARS: &str = "█▉▊▋▌▍▎▏ ";

const PROGRESS_STYLE: &str = "{spinner:.green} ⟪{elapsed_precise}⟫ \
  ⟦{wide_bar:.cyan}⟧ {pos}/{len} ⟨{per_sec}, {eta}⟩ {msg}";

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
    Self::with_bar(
      name,
      io::stderr().is_terminal().then(ProgressBar::new_spinner),
    )
  }

  pub(super) fn reader(&self, reader: impl Read + 'static) -> Box<dyn Read> {
    if let Some(bar) = &self.bar {
      Box::new(bar.wrap_read(reader))
    } else {
      Box::new(reader)
    }
  }

  pub(super) fn update(&self, entry: ProgressEntry) {
    if !entry.processed.is_multiple_of(Self::UPDATE_INTERVAL)
      && entry.processed != entry.total
    {
      return;
    }

    let Some(bar) = &self.bar else {
      return;
    };

    if bar.length().is_none() && entry.total > 0 {
      bar.set_position(0);
      bar.set_length(u64::try_from(entry.total).unwrap_or(u64::MAX));
      bar.set_style(
        ProgressStyle::with_template(PROGRESS_STYLE)
          .unwrap()
          .progress_chars(PROGRESS_CHARS)
          .tick_chars(TICK_CHARS),
      );
      bar.reset_elapsed();
    }

    if bar.length().is_some() {
      bar.set_position(u64::try_from(entry.processed).unwrap_or(u64::MAX));
    }

    bar.set_message(format!(
      "{}: {} scanned, {} new",
      self.name, entry.processed, entry.inserted,
    ));
  }

  fn with_bar(name: &'static str, bar: Option<ProgressBar>) -> Result<Self> {
    if let Some(bar) = &bar {
      bar.set_style(
        ProgressStyle::with_template(SPINNER_STYLE)?.tick_chars(TICK_CHARS),
      );
      bar.set_message(format!("{name}: parsing"));
    }

    Ok(Self { bar, name })
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

  #[test]
  fn parsing_progress_is_reset_for_record_import() {
    let bar = ProgressBar::hidden();
    let progress = Progress::with_bar("foo", Some(bar.clone())).unwrap();
    let mut reader = progress.reader(io::Cursor::new(b"foo"));

    io::copy(&mut reader, &mut io::sink()).unwrap();

    assert_eq!((bar.position(), bar.length()), (3, None));

    progress.update(ProgressEntry {
      inserted: 0,
      processed: 0,
      total: 2,
    });

    assert_eq!((bar.position(), bar.length()), (0, Some(2)));

    progress.update(ProgressEntry {
      inserted: 2,
      processed: 2,
      total: 2,
    });

    assert_eq!((bar.position(), bar.length()), (2, Some(2)));
  }

  #[test]
  fn zero_records_remain_indeterminate() {
    let bar = ProgressBar::hidden();
    let progress = Progress::with_bar("foo", Some(bar.clone())).unwrap();

    progress.update(ProgressEntry {
      inserted: 0,
      processed: 0,
      total: 0,
    });

    assert_eq!(bar.length(), None);
  }
}
