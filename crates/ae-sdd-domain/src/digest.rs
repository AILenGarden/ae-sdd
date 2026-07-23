use std::{fmt, str::FromStr};

use sha2::{Digest, Sha256};

use crate::DigestError;

fn parse_digest(kind: &'static str, value: &str) -> Result<[u8; 32], DigestError> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(DigestError::InvalidEncoding { kind });
    }

    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| DigestError::InvalidEncoding { kind })?;
    Ok(bytes)
}

macro_rules! semantic_digest {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_array(value: [u8; 32]) -> Self {
                Self(value)
            }

            pub fn digest(bytes: impl AsRef<[u8]>) -> Self {
                Self(Sha256::digest(bytes.as_ref()).into())
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub const fn into_array(self) -> [u8; 32] {
                self.0
            }

            pub fn to_hex(self) -> String {
                hex::encode(self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&hex::encode(self.0))
            }
        }

        impl FromStr for $name {
            type Err = DigestError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                parse_digest(stringify!($name), value).map(Self)
            }
        }
    };
}

semantic_digest!(ArtifactDigest);
semantic_digest!(EvidenceDigest);
semantic_digest!(PolicyDigest);
semantic_digest!(GateImplementationDigest);
semantic_digest!(InputFingerprint);
semantic_digest!(ToolchainDigest);
semantic_digest!(ConfigDigest);
semantic_digest!(GateKeyDigest);
semantic_digest!(ResultDigest);
semantic_digest!(DecisionDigest);
semantic_digest!(ContextDigest);

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn semantic_sha256_digest_round_trips_canonical_lower_hex(bytes in any::<[u8; 32]>()) {
            let digest = ArtifactDigest::from_array(bytes);
            let encoded = digest.to_string();
            let decoded: ArtifactDigest = encoded.parse().expect("canonical digest parses");

            prop_assert_eq!(decoded, digest);
            prop_assert_eq!(encoded.len(), 64);
        }
    }

    #[test]
    fn semantic_sha256_digest_rejects_uppercase_and_wrong_length() {
        assert!(PolicyDigest::from_str(&"A".repeat(64)).is_err());
        assert!(PolicyDigest::from_str(&"a".repeat(63)).is_err());
    }

    #[test]
    fn semantic_sha256_digest_hashes_input_deterministically() {
        let first = InputFingerprint::digest(b"same input");
        let second = InputFingerprint::digest(b"same input");
        let changed = InputFingerprint::digest(b"changed input");

        assert_eq!(first, second);
        assert_ne!(first, changed);
    }
}
