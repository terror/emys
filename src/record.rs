use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Record {
  pub(super) entry: Entry,
  pub(super) fingerprint: Vec<u8>,
}

impl Record {
  pub(super) fn new(
    entry: Entry,
    variant: &[u8],
    components: impl IntoIterator<Item = impl AsRef<[u8]>>,
  ) -> Self {
    let mut fingerprint = Vec::new();

    fingerprint
      .extend_from_slice(&u64::try_from(variant.len()).unwrap().to_be_bytes());

    fingerprint.extend_from_slice(variant);

    for component in components {
      let component = component.as_ref();

      fingerprint.extend_from_slice(
        &u64::try_from(component.len()).unwrap().to_be_bytes(),
      );

      fingerprint.extend_from_slice(component);
    }

    Self { entry, fingerprint }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fingerprints_encode_boundaries() {
    let fingerprint =
      Record::new(Entry::default(), b"foo", [b"bar".as_slice()]).fingerprint;

    assert_eq!(
      fingerprint,
      [
        0, 0, 0, 0, 0, 0, 0, 3, b'f', b'o', b'o', 0, 0, 0, 0, 0, 0, 0, 3, b'b',
        b'a', b'r',
      ],
    );

    assert_ne!(
      Record::new(
        Entry::default(),
        b"foo",
        [b"a".as_slice(), b"bc".as_slice()],
      )
      .fingerprint,
      Record::new(
        Entry::default(),
        b"foo",
        [b"ab".as_slice(), b"c".as_slice()],
      )
      .fingerprint,
    );
  }
}
