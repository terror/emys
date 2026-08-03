use {super::*, importer::Importer};

const NAMESPACE: Uuid = Uuid::from_u128(0x81d6d1f2_748f_5b72_89b6_bf87e06a762d);

mod importer;
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
