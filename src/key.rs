use super::*;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct Key {
  pub(super) components: Vec<Vec<u8>>,
  pub(super) variant: Vec<u8>,
}

impl Key {
  const NAMESPACE: Uuid =
    Uuid::from_u128(0x81d6d1f2_748f_5b72_89b6_bf87e06a762d);

  pub(super) fn identifier(&self, format: &str, occurrence: u64) -> Uuid {
    let capacity = self
      .components
      .iter()
      .map(Vec::len)
      .sum::<usize>()
      .saturating_add(self.variant.len())
      .saturating_add(format.len())
      .saturating_add(64);

    let mut name = Vec::with_capacity(capacity);

    name.extend_from_slice(
      &u64::try_from(self.variant.len()).unwrap().to_be_bytes(),
    );

    name.extend_from_slice(&self.variant);

    name.extend_from_slice(&u64::try_from(format.len()).unwrap().to_be_bytes());
    name.extend_from_slice(format.as_bytes());

    for component in &self.components {
      name.extend_from_slice(
        &u64::try_from(component.len()).unwrap().to_be_bytes(),
      );

      name.extend_from_slice(component);
    }

    let occurrence = occurrence.to_be_bytes();

    name.extend_from_slice(
      &u64::try_from(occurrence.len()).unwrap().to_be_bytes(),
    );

    name.extend_from_slice(&occurrence);

    Uuid::new_v5(&Self::NAMESPACE, &name)
  }
}
