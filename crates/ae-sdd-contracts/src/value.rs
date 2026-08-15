use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current schema version of the published control-plane contracts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum SchemaVersion {
    /// First stable contract schema.
    #[serde(rename = "v1")]
    V1,
    /// RA-derived route binding and frozen engineering route contract.
    #[serde(rename = "v2")]
    V2,
}

/// Error returned when a published string value is not canonical.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ContractValueError {
    /// The value was empty.
    #[error("{kind} cannot be empty")]
    Empty {
        /// Contract value kind.
        kind: &'static str,
    },
    /// The value exceeded its byte limit.
    #[error("{kind} exceeds {max_bytes} bytes (actual: {actual_bytes})")]
    TooLong {
        /// Contract value kind.
        kind: &'static str,
        /// Maximum accepted byte length.
        max_bytes: usize,
        /// Observed byte length.
        actual_bytes: usize,
    },
    /// The value did not begin with an ASCII alphanumeric byte.
    #[error("{kind} must start with an ASCII alphanumeric character")]
    InvalidStart {
        /// Contract value kind.
        kind: &'static str,
    },
    /// The value contained a character outside its portable identifier alphabet.
    #[error("{kind} contains invalid character {character:?} at byte {byte_index}")]
    InvalidCharacter {
        /// Contract value kind.
        kind: &'static str,
        /// Zero-based byte offset.
        byte_index: usize,
        /// Rejected character.
        character: char,
    },
    /// Free-form text exceeded its type-level byte budget.
    #[error("bounded text exceeds {max_bytes} bytes (actual: {actual_bytes})")]
    TextTooLong {
        /// Maximum accepted byte length.
        max_bytes: usize,
        /// Observed byte length.
        actual_bytes: usize,
    },
}

fn validate_identifier(
    kind: &'static str,
    max_bytes: usize,
    value: &str,
) -> Result<(), ContractValueError> {
    if value.is_empty() {
        return Err(ContractValueError::Empty { kind });
    }
    if value.len() > max_bytes {
        return Err(ContractValueError::TooLong {
            kind,
            max_bytes,
            actual_bytes: value.len(),
        });
    }
    if !value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
    {
        return Err(ContractValueError::InvalidStart { kind });
    }
    if let Some((byte_index, character)) = value
        .char_indices()
        .find(|(_, character)| !character.is_ascii_alphanumeric() && !"._:-".contains(*character))
    {
        return Err(ContractValueError::InvalidCharacter {
            kind,
            byte_index,
            character,
        });
    }
    Ok(())
}

macro_rules! portable_identifier {
    ($name:ident, $kind:literal, $max:literal) => {
        #[doc = concat!("Validated portable ", $kind, " value.")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            /// Maximum encoded byte length.
            pub const MAX_BYTES: usize = $max;

            /// Validates and constructs the value.
            pub fn new(value: impl Into<Box<str>>) -> Result<Self, ContractValueError> {
                let value = value.into();
                validate_identifier($kind, Self::MAX_BYTES, &value)?;
                Ok(Self(value))
            }

            /// Returns the canonical string representation.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

portable_identifier!(SkillId, "skill id", 128);
portable_identifier!(SeriesKind, "series kind", 64);
portable_identifier!(MethodologyVariant, "methodology variant", 64);
portable_identifier!(RouteDecisionId, "route decision id", 128);
portable_identifier!(ReasonCode, "reason code", 128);
portable_identifier!(SeriesId, "series id", 128);
portable_identifier!(IdempotencyKey, "idempotency key", 128);
portable_identifier!(AdapterId, "host adapter id", 128);
portable_identifier!(HostTaskId, "host task id", 128);
portable_identifier!(ExternalSessionKey, "external session key", 128);
portable_identifier!(ContextBundleId, "context bundle id", 128);
portable_identifier!(DocumentTxnId, "document transaction id", 128);
// `ae-sdd-daemon-design.md` §4.1: the stable *logical* identity of a Spec,
// minted or resolved by the daemon's document registry. It survives a path
// change, which is why a path can never serve as a document identity.
portable_identifier!(DocumentId, "document id", 128);
// The identity of a Spec graph. This comes from §8.3 and §8.4, not §4.1: §4.1's
// table freezes nine identities and a graph is not one of them. §8.3 makes Spec
// relations a directed graph rather than a forced single-parent tree, and §8.4
// rule 3 creates a new graph when a resolved `DocumentId` belongs to none — so
// the graph is an addressable thing that outlives any one document, and needs an
// identity separate from the documents it contains.
//
// Frozen ahead of the registry that will own it, so graph edges are addressable
// before the storage exists.
portable_identifier!(SpecGraphId, "spec graph id", 128);
portable_identifier!(PrdId, "PRD id", 128);
portable_identifier!(MutationIntentId, "mutation intent id", 128);
portable_identifier!(LogicalNamespace, "logical namespace", 64);
portable_identifier!(LogicalKey, "logical key", 128);
portable_identifier!(ReviewId, "review id", 128);
portable_identifier!(ReviewerRole, "reviewer role", 64);
portable_identifier!(ExecutionId, "execution id", 128);
portable_identifier!(ExecutionStepId, "execution step id", 128);
portable_identifier!(WorkerId, "worker id", 128);
portable_identifier!(VerificationContractId, "verification contract id", 128);
portable_identifier!(MessageKey, "message key", 128);
portable_identifier!(RuntimeModuleKey, "runtime module key", 128);
portable_identifier!(RuntimeModuleName, "runtime module name", 128);
portable_identifier!(OperationName, "operation name", 128);

impl ReasonCode {
    /// Returns the contract-owned emergency reason used only when richer
    /// invariant vocabulary itself cannot be constructed.
    ///
    /// This is deliberately narrow: callers must not use it to bypass normal
    /// validation or replace a recoverable input error.
    pub fn invariant_fallback() -> Self {
        Self("contract.invariant-failed".into())
    }
}

impl MessageKey {
    /// Returns the contract-owned emergency message key used only when richer
    /// invariant vocabulary itself cannot be constructed.
    ///
    /// The value is owned here so error paths can remain total without panic,
    /// abort, unsafe construction, or an invalid wire envelope.
    pub fn invariant_fallback() -> Self {
        Self("control-plane.contract-invariant-failed".into())
    }
}

/// UTF-8 text whose encoded byte length is fixed by the surrounding contract.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct BoundedText<const MAX_BYTES: usize>(Box<str>);

impl<const MAX_BYTES: usize> BoundedText<MAX_BYTES> {
    /// Validates and constructs bounded text.
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, ContractValueError> {
        let value = value.into();
        if value.len() > MAX_BYTES {
            return Err(ContractValueError::TextTooLong {
                max_bytes: MAX_BYTES,
                actual_bytes: value.len(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the text contents.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the encoded byte length.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true when the text is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<const MAX_BYTES: usize> fmt::Display for BoundedText<MAX_BYTES> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de, const MAX_BYTES: usize> Deserialize<'de> for BoundedText<MAX_BYTES> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageKey, ReasonCode};

    #[test]
    fn invariant_fallback_vocabulary_is_itself_contract_valid() {
        let reason = ReasonCode::invariant_fallback();
        let message = MessageKey::invariant_fallback();

        assert!(ReasonCode::new(reason.as_str()).is_ok());
        assert!(MessageKey::new(message.as_str()).is_ok());
    }
}
