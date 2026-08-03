use {super::*, std::collections::HashMap};

const NAMESPACE: Uuid = Uuid::from_u128(0x81d6d1f2_748f_5b72_89b6_bf87e06a762d);

mod zsh;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Source {
  Zsh(zsh::Zsh),
}

impl Source {
  pub(crate) fn run(self, database: &Database) -> Result {
    match self {
      Self::Zsh(zsh) => zsh.import(database),
    }
  }
}

trait HistoryImporter {
  const FORMAT: &'static str;
  const NAME: &'static str;

  /// Imports this source's history into the database.
  fn import(&self, database: &Database) -> Result {
    let path = self.path()?;

    let contents = fs::read(&path).with_context(|| {
      format!("failed to read {} history `{}`", Self::NAME, path.display())
    })?;

    let entries = self.parse(&contents).with_context(|| {
      format!(
        "failed to parse {} history `{}`",
        Self::NAME,
        path.display()
      )
    })?;

    let records = records(Self::FORMAT, entries)?;

    let inserted = database.import(&records)?;

    println!("imported {inserted} executions from {}", path.display());

    Ok(())
  }

  /// Parses raw history file contents into imported executions.
  fn parse(&self, contents: &[u8]) -> Result<Vec<ImportedExecution>>;

  /// Determines the history file path from source-specific configuration.
  fn path(&self) -> Result<PathBuf>;
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Identity {
  fields: Vec<Vec<u8>>,
  scheme: Vec<u8>,
}

impl Identity {
  fn field(mut self, field: impl AsRef<[u8]>) -> Self {
    self.fields.push(field.as_ref().to_vec());
    self
  }

  fn new(scheme: &[u8]) -> Self {
    Self {
      fields: Vec::new(),
      scheme: scheme.to_vec(),
    }
  }
}

#[derive(Debug, Eq, PartialEq)]
struct ImportedExecution {
  execution: Execution,
  identity: Identity,
}

impl ImportedExecution {
  fn new(identity: Identity, execution: Execution) -> Self {
    Self {
      execution,
      identity,
    }
  }
}

fn frame(destination: &mut Vec<u8>, value: &[u8]) {
  destination
    .extend_from_slice(&u64::try_from(value.len()).unwrap().to_be_bytes());
  destination.extend_from_slice(value);
}

fn identifier(format: &str, identity: &Identity, occurrence: u64) -> Uuid {
  let capacity = identity
    .fields
    .iter()
    .map(Vec::len)
    .sum::<usize>()
    .saturating_add(identity.scheme.len())
    .saturating_add(format.len())
    .saturating_add(64);

  let mut name = Vec::with_capacity(capacity);

  frame(&mut name, &identity.scheme);
  frame(&mut name, format.as_bytes());

  for field in &identity.fields {
    frame(&mut name, field);
  }

  frame(&mut name, &occurrence.to_be_bytes());

  Uuid::new_v5(&NAMESPACE, &name)
}

fn records(
  format: &str,
  imported: Vec<ImportedExecution>,
) -> Result<Vec<(Uuid, Execution)>> {
  let mut occurrences = HashMap::new();
  let mut records = Vec::with_capacity(imported.len());

  for imported in imported {
    let occurrence = occurrences
      .entry(imported.identity.clone())
      .or_insert(0_u64);

    *occurrence = occurrence
      .checked_add(1)
      .context("history occurrence count overflowed")?;

    let mut execution = imported.execution;
    execution.shell = Some(format.into());

    records.push((
      identifier(format, &imported.identity, *occurrence),
      execution,
    ));
  }

  Ok(records)
}
