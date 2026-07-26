//! Frozen append-only evidence ledger contracts.
//!
//! `.auto-engineering/{storyId}/evidence/ledger.jsonl` is the evidence truth:
//! one canonical JSON event per line forming a hash chain. `manifest.json` is
//! only the deterministic active projection of the ledger, and project state
//! stores nothing but the ledger/manifest locators and digests.
//!
//! Canonical event encoding (shared with the Python migration oracle; both
//! sides are tested against the same golden digest):
//!
//! - UTF-8 JSON, object keys sorted byte-wise, no insignificant whitespace and
//!   no ASCII escaping beyond the JSON-mandatory `"` and `\` escapes, the
//!   short escapes `\b \t \n \f \r` and `\u00XX` for the remaining C0 control
//!   characters. Non-ASCII text stays unescaped.
//! - Key order is therefore `artifactRefs, eventDigest, eventId,
//!   inputFingerprint, kind, logicalKey, previousEventDigest, sequence`;
//!   artifact references use `byteLength, digest, kind, path`.
//! - Digests serialize as 64 lowercase hex characters without a `sha256:`
//!   prefix; `previousEventDigest` is `null` exactly for the genesis event.
//! - `eventDigest` is the SHA-256 of the canonical encoding of the same event
//!   without the `eventDigest` key, so rewriting any historical line breaks
//!   every later `previousEventDigest` link and is detected by
//!   [`EvidenceLedgerEventV1::verify_chain`].

use ae_sdd_domain::{ArtifactDigest, ArtifactRef, EvidenceId, InputFingerprint};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::execution_runtime::MAX_EVIDENCE_LOGICAL_KEY_BYTES;
use crate::serde_domain;

/// Maximum artifact references one ledger event may bind.
pub const MAX_LEDGER_ARTIFACT_REFS: usize = 16;
/// Maximum events one evidence ledger may contain; decoding beyond this budget fails closed.
pub const MAX_LEDGER_EVENTS: usize = 65_536;

/// Validation and integrity errors for evidence ledger events and chains.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EvidenceLedgerError {
    /// The sequence was zero or otherwise not a positive 1-based position.
    #[error("evidence ledger sequence must be a positive 1-based position")]
    InvalidSequence,
    /// The genesis event declared a previous digest.
    #[error("evidence ledger genesis event must not reference a previous digest")]
    UnexpectedPreviousDigest,
    /// A non-genesis event did not declare the previous event digest.
    #[error("evidence ledger event after the genesis event must reference the previous digest")]
    MissingPreviousDigest,
    /// A recorded/superseded/invalidated event carried an empty or oversized logical key.
    #[error("evidence ledger logical key must be non-empty and within its byte limit")]
    InvalidLogicalKey,
    /// A finalized event carried a logical key; it binds the whole projection instead.
    #[error("evidence ledger finalized event must not carry a logical key")]
    UnexpectedLogicalKey,
    /// The event carried more artifact references than the frozen v1 limit.
    #[error("evidence ledger event exceeds its frozen v1 artifact reference limit")]
    CollectionLimitExceeded,
    /// The declared event digest did not match the canonical preimage.
    #[error("evidence ledger event digest does not match its canonical preimage")]
    DigestMismatch,
    /// A chain position did not link to the digest of its predecessor.
    #[error("evidence ledger chain link is broken")]
    ChainLinkBroken,
    /// A chain position was not the contiguous successor of its predecessor.
    #[error("evidence ledger sequence is not contiguous")]
    SequenceGap,
}

/// Machine lifecycle of one evidence ledger event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceLedgerEventKind {
    /// A new evidence entry was recorded.
    Recorded,
    /// The active entry for a logical key was superseded by a newer record.
    Superseded,
    /// The active projection was finalized and sealed.
    Finalized,
    /// The active entry for a logical key was invalidated.
    Invalidated,
}

impl EvidenceLedgerEventKind {
    /// Returns the stable wire value of the event kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Superseded => "superseded",
            Self::Finalized => "finalized",
            Self::Invalidated => "invalidated",
        }
    }
}

/// One immutable append-only evidence ledger event.
///
/// Construction validates the frozen v1 invariants and computes the
/// hash-chain digest; decoding re-verifies the declared digest so a tampered
/// line fails closed instead of entering the projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    try_from = "EvidenceLedgerEventV1Wire",
    into = "EvidenceLedgerEventV1Wire"
)]
pub struct EvidenceLedgerEventV1 {
    sequence: u64,
    event_id: EvidenceId,
    kind: EvidenceLedgerEventKind,
    logical_key: Box<str>,
    input_fingerprint: InputFingerprint,
    artifact_refs: Vec<ArtifactRef>,
    previous_event_digest: Option<ArtifactDigest>,
    event_digest: ArtifactDigest,
}

impl EvidenceLedgerEventV1 {
    /// Constructs a validated event and computes its hash-chain digest.
    ///
    /// The genesis event (`sequence == 1`) must not declare a previous digest;
    /// every later event must. Only `Finalized` events carry an empty logical
    /// key, because they bind the whole projection rather than one entry.
    pub fn new(
        sequence: u64,
        event_id: EvidenceId,
        kind: EvidenceLedgerEventKind,
        logical_key: impl Into<Box<str>>,
        input_fingerprint: InputFingerprint,
        artifact_refs: Vec<ArtifactRef>,
        previous_event_digest: Option<ArtifactDigest>,
    ) -> Result<Self, EvidenceLedgerError> {
        let logical_key = logical_key.into();
        Self::validate_fields(
            sequence,
            kind,
            &logical_key,
            &artifact_refs,
            previous_event_digest,
        )?;
        let event_digest = ArtifactDigest::digest(canonical_event_bytes(
            sequence,
            &event_id,
            kind,
            &logical_key,
            input_fingerprint,
            &artifact_refs,
            previous_event_digest,
            None,
        ));
        Ok(Self {
            sequence,
            event_id,
            kind,
            logical_key,
            input_fingerprint,
            artifact_refs,
            previous_event_digest,
            event_digest,
        })
    }

    fn validate_fields(
        sequence: u64,
        kind: EvidenceLedgerEventKind,
        logical_key: &str,
        artifact_refs: &[ArtifactRef],
        previous_event_digest: Option<ArtifactDigest>,
    ) -> Result<(), EvidenceLedgerError> {
        if sequence == 0 {
            return Err(EvidenceLedgerError::InvalidSequence);
        }
        match (sequence, previous_event_digest) {
            (1, Some(_)) => return Err(EvidenceLedgerError::UnexpectedPreviousDigest),
            (1, None) => {}
            (_, None) => return Err(EvidenceLedgerError::MissingPreviousDigest),
            (_, Some(_)) => {}
        }
        if kind == EvidenceLedgerEventKind::Finalized {
            if !logical_key.is_empty() {
                return Err(EvidenceLedgerError::UnexpectedLogicalKey);
            }
        } else if logical_key.trim().is_empty()
            || logical_key.len() > MAX_EVIDENCE_LOGICAL_KEY_BYTES
        {
            return Err(EvidenceLedgerError::InvalidLogicalKey);
        }
        if artifact_refs.len() > MAX_LEDGER_ARTIFACT_REFS {
            return Err(EvidenceLedgerError::CollectionLimitExceeded);
        }
        Ok(())
    }

    /// Returns the 1-based position of the event in the ledger.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the event identity; recorded events reuse the entry evidence id.
    pub const fn event_id(&self) -> &EvidenceId {
        &self.event_id
    }

    /// Returns the event kind.
    pub const fn kind(&self) -> EvidenceLedgerEventKind {
        self.kind
    }

    /// Returns the logical key the event applies to (empty for `Finalized`).
    pub fn logical_key(&self) -> &str {
        &self.logical_key
    }

    /// Returns the typed input fingerprint bound by the event.
    pub const fn input_fingerprint(&self) -> InputFingerprint {
        self.input_fingerprint
    }

    /// Returns the artifact references bound by the event.
    pub fn artifact_refs(&self) -> &[ArtifactRef] {
        &self.artifact_refs
    }

    /// Returns the digest of the previous event (`None` for the genesis event).
    pub const fn previous_event_digest(&self) -> Option<ArtifactDigest> {
        self.previous_event_digest
    }

    /// Returns the declared hash-chain digest of this event.
    pub const fn event_digest(&self) -> ArtifactDigest {
        self.event_digest
    }

    /// Returns the canonical JSON encoding of the event (one JSONL line
    /// without the trailing newline). Byte-stable across implementations.
    pub fn canonical_json(&self) -> Vec<u8> {
        canonical_event_bytes(
            self.sequence,
            &self.event_id,
            self.kind,
            &self.logical_key,
            self.input_fingerprint,
            &self.artifact_refs,
            self.previous_event_digest,
            Some(self.event_digest),
        )
    }

    /// Verifies one contiguous hash chain: sequences are 1-based and gapless,
    /// every event links to its predecessor digest. Event payloads were
    /// already digest-verified at construction or decode time.
    pub fn verify_chain(events: &[Self]) -> Result<(), EvidenceLedgerError> {
        if events.len() > MAX_LEDGER_EVENTS {
            return Err(EvidenceLedgerError::CollectionLimitExceeded);
        }
        let mut previous: Option<ArtifactDigest> = None;
        for (index, event) in events.iter().enumerate() {
            let expected_sequence = (index as u64) + 1;
            if event.sequence != expected_sequence {
                return Err(EvidenceLedgerError::SequenceGap);
            }
            if event.previous_event_digest != previous {
                return Err(EvidenceLedgerError::ChainLinkBroken);
            }
            previous = Some(event.event_digest);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for EvidenceLedgerEventV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        EvidenceLedgerEventV1Wire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceLedgerEventV1Wire {
    sequence: u64,
    #[serde(with = "serde_domain::evidence_id")]
    event_id: EvidenceId,
    kind: EvidenceLedgerEventKind,
    logical_key: Box<str>,
    #[serde(with = "serde_domain::input_fingerprint")]
    input_fingerprint: InputFingerprint,
    #[serde(with = "serde_domain::artifact_refs")]
    artifact_refs: Vec<ArtifactRef>,
    #[serde(with = "optional_artifact_digest")]
    previous_event_digest: Option<ArtifactDigest>,
    #[serde(with = "serde_domain::artifact_digest")]
    event_digest: ArtifactDigest,
}

impl TryFrom<EvidenceLedgerEventV1Wire> for EvidenceLedgerEventV1 {
    type Error = EvidenceLedgerError;

    fn try_from(value: EvidenceLedgerEventV1Wire) -> Result<Self, Self::Error> {
        let event = Self::new(
            value.sequence,
            value.event_id,
            value.kind,
            value.logical_key,
            value.input_fingerprint,
            value.artifact_refs,
            value.previous_event_digest,
        )?;
        if event.event_digest != value.event_digest {
            return Err(EvidenceLedgerError::DigestMismatch);
        }
        Ok(event)
    }
}

impl From<EvidenceLedgerEventV1> for EvidenceLedgerEventV1Wire {
    fn from(value: EvidenceLedgerEventV1) -> Self {
        Self {
            sequence: value.sequence,
            event_id: value.event_id,
            kind: value.kind,
            logical_key: value.logical_key,
            input_fingerprint: value.input_fingerprint,
            artifact_refs: value.artifact_refs,
            previous_event_digest: value.previous_event_digest,
            event_digest: value.event_digest,
        }
    }
}

/// Writes the canonical encoding of one event. Keys are emitted in sorted
/// byte order; `event_digest == None` produces the hash-chain preimage.
#[allow(clippy::too_many_arguments)]
fn canonical_event_bytes(
    sequence: u64,
    event_id: &EvidenceId,
    kind: EvidenceLedgerEventKind,
    logical_key: &str,
    input_fingerprint: InputFingerprint,
    artifact_refs: &[ArtifactRef],
    previous_event_digest: Option<ArtifactDigest>,
    event_digest: Option<ArtifactDigest>,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(256);
    output.extend_from_slice(b"{\"artifactRefs\":[");
    for (index, reference) in artifact_refs.iter().enumerate() {
        if index > 0 {
            output.push(b',');
        }
        output.extend_from_slice(b"{\"byteLength\":");
        output.extend_from_slice(reference.byte_length().to_string().as_bytes());
        output.extend_from_slice(b",\"digest\":");
        write_json_string(&mut output, &reference.digest().to_string());
        output.extend_from_slice(b",\"kind\":");
        write_json_string(&mut output, reference.kind().as_str());
        output.extend_from_slice(b",\"path\":");
        write_json_string(&mut output, reference.path().as_str());
        output.push(b'}');
    }
    output.push(b']');
    if let Some(event_digest) = event_digest {
        output.extend_from_slice(b",\"eventDigest\":");
        write_json_string(&mut output, &event_digest.to_string());
    }
    output.extend_from_slice(b",\"eventId\":");
    write_json_string(&mut output, event_id.as_str());
    output.extend_from_slice(b",\"inputFingerprint\":");
    write_json_string(&mut output, &input_fingerprint.to_string());
    output.extend_from_slice(b",\"kind\":");
    write_json_string(&mut output, kind.as_str());
    output.extend_from_slice(b",\"logicalKey\":");
    write_json_string(&mut output, logical_key);
    output.extend_from_slice(b",\"previousEventDigest\":");
    match previous_event_digest {
        Some(digest) => write_json_string(&mut output, &digest.to_string()),
        None => output.extend_from_slice(b"null"),
    }
    output.extend_from_slice(b",\"sequence\":");
    output.extend_from_slice(sequence.to_string().as_bytes());
    output.push(b'}');
    output
}

/// Writes one JSON string using exactly the escapes `serde_json` and Python's
/// `json.dumps(ensure_ascii=False)` produce: the mandatory quote/backslash
/// escapes, the short C0 escapes and `\u00XX` for the remaining controls.
fn write_json_string(output: &mut Vec<u8>, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(b'"');
    for &byte in value.as_bytes() {
        match byte {
            b'"' => output.extend_from_slice(b"\\\""),
            b'\\' => output.extend_from_slice(b"\\\\"),
            0x08 => output.extend_from_slice(b"\\b"),
            0x09 => output.extend_from_slice(b"\\t"),
            0x0a => output.extend_from_slice(b"\\n"),
            0x0c => output.extend_from_slice(b"\\f"),
            0x0d => output.extend_from_slice(b"\\r"),
            0x00..=0x1f => {
                output.extend_from_slice(b"\\u00");
                output.push(HEX[(byte >> 4) as usize]);
                output.push(HEX[(byte & 0x0f) as usize]);
            }
            _ => output.push(byte),
        }
    }
    output.push(b'"');
}

mod optional_artifact_digest {
    use std::str::FromStr;

    use ae_sdd_domain::ArtifactDigest;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub(super) fn serialize<S>(
        value: &Option<ArtifactDigest>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|digest| digest.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactDigest>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| ArtifactDigest::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ae_sdd_domain::{ArtifactKind, ProjectRelativePath};

    use super::*;

    const GOLDEN_EVENT_DIGEST: &str =
        "f413824b8d196be69690e917fe65ef5291e6dc49d68dea750cd306172b355e56";

    fn golden_event() -> EvidenceLedgerEventV1 {
        EvidenceLedgerEventV1::new(
            1,
            EvidenceId::new("ev-golden-00000001").expect("id"),
            EvidenceLedgerEventKind::Recorded,
            "tests/golden",
            InputFingerprint::from_str(
                "1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("fingerprint"),
            vec![ArtifactRef::new(
                ArtifactKind::new("evidence-entry").expect("kind"),
                ProjectRelativePath::new(
                    ".auto-engineering/STORY-GOLDEN/evidence/entries/entry.json",
                )
                .expect("path"),
                ArtifactDigest::from_str(
                    "2222222222222222222222222222222222222222222222222222222222222222",
                )
                .expect("digest"),
                128,
            )],
            None,
        )
        .expect("golden event")
    }

    #[test]
    fn golden_event_digest_is_stable_across_implementations() {
        assert_eq!(
            golden_event().event_digest().to_string(),
            GOLDEN_EVENT_DIGEST
        );
    }

    #[test]
    fn canonical_json_matches_the_sorted_compact_encoding() {
        let event = golden_event();
        let canonical = event.canonical_json();
        let expected = serde_json::to_vec(&serde_json::json!({
            "artifactRefs": [{
                "kind": "evidence-entry",
                "path": ".auto-engineering/STORY-GOLDEN/evidence/entries/entry.json",
                "digest": "2222222222222222222222222222222222222222222222222222222222222222",
                "byteLength": 128,
            }],
            "eventDigest": GOLDEN_EVENT_DIGEST,
            "eventId": "ev-golden-00000001",
            "inputFingerprint": "1111111111111111111111111111111111111111111111111111111111111111",
            "kind": "recorded",
            "logicalKey": "tests/golden",
            "previousEventDigest": null,
            "sequence": 1,
        }))
        .expect("expected encoding");
        assert_eq!(canonical, expected);
    }

    #[test]
    fn wire_round_trip_revalidates_the_declared_digest() {
        let event = golden_event();
        let bytes = event.canonical_json();
        let decoded: EvidenceLedgerEventV1 = serde_json::from_slice(&bytes).expect("event decodes");
        assert_eq!(decoded, event);
        let mut value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("event JSON value");
        value["logicalKey"] = serde_json::json!("tests/tampered");
        assert!(serde_json::from_value::<EvidenceLedgerEventV1>(value).is_err());
    }

    #[test]
    fn unknown_wire_fields_and_kinds_fail_closed() {
        let bytes = golden_event().canonical_json();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("event JSON");
        value["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<EvidenceLedgerEventV1>(value.clone()).is_err());
        value["unexpected"] = serde_json::Value::Null;
        value.as_object_mut().expect("object").remove("unexpected");
        value["kind"] = serde_json::json!("erased");
        assert!(serde_json::from_value::<EvidenceLedgerEventV1>(value).is_err());
    }

    #[test]
    fn field_invariants_are_enforced() {
        let fingerprint = InputFingerprint::digest(b"input");
        let id = || EvidenceId::new("ev-test").expect("id");
        assert_eq!(
            EvidenceLedgerEventV1::new(
                0,
                id(),
                EvidenceLedgerEventKind::Recorded,
                "k",
                fingerprint,
                vec![],
                None
            ),
            Err(EvidenceLedgerError::InvalidSequence)
        );
        assert_eq!(
            EvidenceLedgerEventV1::new(
                1,
                id(),
                EvidenceLedgerEventKind::Recorded,
                "k",
                fingerprint,
                vec![],
                Some(ArtifactDigest::digest(b"previous")),
            ),
            Err(EvidenceLedgerError::UnexpectedPreviousDigest)
        );
        assert_eq!(
            EvidenceLedgerEventV1::new(
                2,
                id(),
                EvidenceLedgerEventKind::Recorded,
                "k",
                fingerprint,
                vec![],
                None
            ),
            Err(EvidenceLedgerError::MissingPreviousDigest)
        );
        assert_eq!(
            EvidenceLedgerEventV1::new(
                1,
                id(),
                EvidenceLedgerEventKind::Recorded,
                " ",
                fingerprint,
                vec![],
                None
            ),
            Err(EvidenceLedgerError::InvalidLogicalKey)
        );
        assert_eq!(
            EvidenceLedgerEventV1::new(
                1,
                id(),
                EvidenceLedgerEventKind::Finalized,
                "k",
                fingerprint,
                vec![],
                None
            ),
            Err(EvidenceLedgerError::UnexpectedLogicalKey)
        );
        assert_eq!(
            EvidenceLedgerEventV1::new(
                1,
                id(),
                EvidenceLedgerEventKind::Recorded,
                "k",
                fingerprint,
                vec![
                    ArtifactRef::new(
                        ArtifactKind::new("evidence-entry").expect("kind"),
                        ProjectRelativePath::new("a.json").expect("path"),
                        ArtifactDigest::digest(b"a"),
                        1,
                    );
                    MAX_LEDGER_ARTIFACT_REFS + 1
                ],
                None,
            ),
            Err(EvidenceLedgerError::CollectionLimitExceeded)
        );
    }

    #[test]
    fn verify_chain_detects_gaps_and_broken_links() {
        let first = golden_event();
        let second = EvidenceLedgerEventV1::new(
            2,
            EvidenceId::new("ev-second").expect("id"),
            EvidenceLedgerEventKind::Superseded,
            "tests/golden",
            InputFingerprint::digest(b"next"),
            vec![],
            Some(first.event_digest()),
        )
        .expect("second event");
        EvidenceLedgerEventV1::verify_chain(&[first.clone(), second.clone()]).expect("chain");
        assert_eq!(
            EvidenceLedgerEventV1::verify_chain(&[second]),
            Err(EvidenceLedgerError::SequenceGap)
        );
        let orphan = EvidenceLedgerEventV1::new(
            2,
            EvidenceId::new("ev-orphan").expect("id"),
            EvidenceLedgerEventKind::Superseded,
            "tests/golden",
            InputFingerprint::digest(b"other"),
            vec![],
            Some(ArtifactDigest::digest(b"not-the-previous-event")),
        )
        .expect("orphan event");
        assert_eq!(
            EvidenceLedgerEventV1::verify_chain(&[first, orphan]),
            Err(EvidenceLedgerError::ChainLinkBroken)
        );
    }

    #[test]
    fn canonical_escaping_matches_serde_json() {
        let tricky = "quote\" backslash\\ ctrl\u{1} unicode-ä";
        let event = EvidenceLedgerEventV1::new(
            1,
            EvidenceId::new("ev-escapes").expect("id"),
            EvidenceLedgerEventKind::Recorded,
            tricky,
            InputFingerprint::digest(b"input"),
            vec![],
            None,
        )
        .expect("event");
        let canonical = event.canonical_json();
        let expected = serde_json::to_vec(&serde_json::json!({
            "artifactRefs": [],
            "eventDigest": event.event_digest().to_string(),
            "eventId": "ev-escapes",
            "inputFingerprint": event.input_fingerprint().to_string(),
            "kind": "recorded",
            "logicalKey": tricky,
            "previousEventDigest": null,
            "sequence": 1,
        }))
        .expect("expected encoding");
        assert_eq!(canonical, expected);
        let decoded: EvidenceLedgerEventV1 =
            serde_json::from_slice(&canonical).expect("escaped event decodes");
        assert_eq!(decoded.logical_key(), tricky);
    }
}
