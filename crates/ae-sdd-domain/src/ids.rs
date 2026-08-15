use std::{fmt, str::FromStr};

use uuid::Uuid;

use crate::{StringIdError, UuidIdError};

macro_rules! uuid_id {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Uuid);

        impl $name {
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            pub const fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = UuidIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|source| UuidIdError {
                        kind: $label,
                        source,
                    })
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

uuid_id!(RequestId, "RequestId");
uuid_id!(BootId, "BootId");
uuid_id!(EventStoreId, "EventStoreId");
uuid_id!(WorkspaceId, "WorkspaceId");
uuid_id!(SessionId, "SessionId");
// `ae-sdd-daemon-design.md` §4.1: one main-flow run instance, minted by the
// daemon as a time-ordered UUID so runs sort by creation. A retry never reuses
// it; recovering the same run keeps it. Minting lives in the runtime layer
// because `ae-sdd-domain` stays free of wall clock and randomness.
uuid_id!(FlowRunId, "FlowRunId");
// `ae-sdd-daemon-design.md` §4.1: one *physical* execution attempt of a Series.
// A retry produces a new `SeriesRunId` while keeping the same logical
// `SeriesId`, which is what lets attempts be queried independently.
uuid_id!(SeriesRunId, "SeriesRunId");
// One issued `InstructionEnvelope`. This identity comes from §10.2's field list
// rather than §4.1's identity table, which does not enumerate it: §4.1 names the
// durable business and run identities, while an instruction is a single issued
// directive. It is still daemon-minted, because §10.2 requires the envelope be
// verifiable and §9.3 forbids a child choosing its own `DelegationId` — letting a
// child name the instruction it answers would reopen that same hole.
uuid_id!(InstructionId, "InstructionId");
uuid_id!(TurnId, "TurnId");
uuid_id!(LeaseId, "LeaseId");
uuid_id!(DelegationId, "DelegationId");
uuid_id!(HostActionId, "HostActionId");
uuid_id!(HostAckId, "HostAckId");
uuid_id!(CompactId, "CompactId");
uuid_id!(ContextProjectionId, "ContextProjectionId");
uuid_id!(JobId, "JobId");
uuid_id!(ClaimId, "ClaimId");
// `ae-sdd-daemon-design.md` §9.4: the daemon-minted UUID that records one
// host-bound delegation's liveness bookkeeping. It is never an attestation
// identity (the `ClaimId` chain owns authentication); it only answers "is the
// binding this delegation opened still alive, or has it been
// released/preempted/expired?". Minted deterministically alongside the
// delegation so an idempotent replay of the same create recovers the same id.
uuid_id!(HostExecutionBindingId, "HostExecutionBindingId");

fn validate_string_id(
    kind: &'static str,
    max_bytes: usize,
    value: &str,
) -> Result<(), StringIdError> {
    if value.is_empty() {
        return Err(StringIdError::Empty { kind });
    }
    if value.len() > max_bytes {
        return Err(StringIdError::TooLong {
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
        return Err(StringIdError::InvalidStart { kind });
    }
    if let Some((byte_index, character)) = value
        .char_indices()
        .find(|(_, character)| !character.is_ascii_alphanumeric() && !"._:-".contains(*character))
    {
        return Err(StringIdError::InvalidCharacter {
            kind,
            byte_index,
            character,
        });
    }
    Ok(())
}

macro_rules! string_id {
    ($name:ident, $label:literal, $max:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Box<str>);

        impl $name {
            pub const MAX_BYTES: usize = $max;

            pub fn new(value: impl Into<Box<str>>) -> Result<Self, StringIdError> {
                let value = value.into();
                validate_string_id($label, Self::MAX_BYTES, &value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = StringIdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = StringIdError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = StringIdError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

string_id!(ProjectKey, "ProjectKey", 64);
string_id!(WorkItemId, "WorkItemId", 128);
string_id!(StoryId, "StoryId", 128);
string_id!(GateId, "GateId", 128);
string_id!(OperationId, "OperationId", 128);
string_id!(CapabilityId, "CapabilityId", 128);
string_id!(ArtifactKind, "ArtifactKind", 64);
string_id!(EvidenceId, "EvidenceId", 128);
string_id!(VerificationId, "VerificationId", 128);
string_id!(ExecutionSliceId, "ExecutionSliceId", 128);
string_id!(DeliverableId, "DeliverableId", 128);
string_id!(FindingCode, "FindingCode", 128);
string_id!(ErrorCode, "ErrorCode", 128);
string_id!(CancellationCode, "CancellationCode", 128);

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn uuid_ids_round_trip_without_random_generation(bytes in any::<[u8; 16]>()) {
            let expected = Uuid::from_bytes(bytes);
            let request_id = RequestId::from_uuid(expected);
            let reparsed: RequestId = request_id.to_string().parse().expect("canonical UUID parses");

            prop_assert_eq!(reparsed, request_id);
            prop_assert_eq!(reparsed.into_uuid(), expected);
        }
    }

    #[test]
    fn validated_string_ids_reject_whitespace_and_path_separators() {
        assert!(OperationId::new("state.transition").is_ok());
        assert!(GateId::new("G-14").is_ok());
        assert!(OperationId::new(" state.transition").is_err());
        assert!(OperationId::new("state/transition").is_err());
        assert!(OperationId::new("state transition").is_err());
    }
}
