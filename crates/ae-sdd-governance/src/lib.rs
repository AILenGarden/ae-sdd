use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

const RECEIPT_SCHEMA: &str = "ae-sdd-governance-receipt/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GovernanceArea {
    Protocol,
    OperationRegistry,
    GateRegistry,
    FlowPolicy,
    Hooks,
    Distribution,
    Release,
    Compatibility,
    ServiceLifecycle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlignmentRule {
    pub id: &'static str,
    pub area: GovernanceArea,
    pub triggers: &'static [&'static str],
    pub required_companions: &'static [&'static str],
}

const PROTOCOL_TRIGGERS: &[&str] = &["crates/ae-sdd-protocol/"];
const PROTOCOL_COMPANIONS: &[&str] = &["constraints/api.md", "crates/ae-sdd-protocol/tests/"];
const OPERATION_TRIGGERS: &[&str] = &["crates/ae-sdd-operations/"];
const OPERATION_COMPANIONS: &[&str] = &[
    "tests/fixtures/compatibility/legacy-surface.v1.json",
    "crates/ae-sdd-operations/tests/",
];
const GATE_TRIGGERS: &[&str] = &["crates/ae-sdd-gates/", "crates/ae-sdd-scanners/"];
const GATE_COMPANIONS: &[&str] = &[
    "tests/fixtures/compatibility/legacy-surface.v1.json",
    "crates/ae-sdd-gates/tests/",
];
const FLOW_TRIGGERS: &[&str] = &["crates/ae-sdd-policy/", "crates/ae-sdd-flow/"];
const FLOW_COMPANIONS: &[&str] = &["crates/ae-sdd-flow/tests/", "constraints/layered-arch.md"];
const HOOK_TRIGGERS: &[&str] = &["bins/ae-sdd-cli/", "crates/ae-sdd-client/"];
const HOOK_COMPANIONS: &[&str] = &[".codex/hooks.json", ".harness/agent.md"];
const DISTRIBUTION_TRIGGERS: &[&str] = &["crates/ae-sdd-build/"];
const DISTRIBUTION_COMPANIONS: &[&str] = &["crates/ae-sdd-build/tests/", "RELEASING.md"];
const RELEASE_TRIGGERS: &[&str] = &["bins/ae-sdd-daemon/", "bins/ae-sdd-cli/"];
const RELEASE_COMPANIONS: &[&str] = &[".github/workflows/ae-sdd-rust.yml", "Cargo.lock"];
const COMPATIBILITY_TRIGGERS: &[&str] = &["tests/fixtures/compatibility/"];
const COMPATIBILITY_COMPANIONS: &[&str] = &[
    "crates/ae-sdd-build/tests/compatibility_routes.rs",
    "bins/ae-sdd-cli/tests/legacy_dispatch.rs",
];
const SERVICE_TRIGGERS: &[&str] = &["crates/ae-sdd-build/src/config.rs", "bins/ae-sdd-daemon/"];
const SERVICE_COMPANIONS: &[&str] = &[
    ".github/workflows/ae-sdd-rust.yml",
    "tests/fixtures/protocol/",
];

pub const ALIGNMENT_RULES: [AlignmentRule; 9] = [
    AlignmentRule {
        id: "UG-PROTOCOL",
        area: GovernanceArea::Protocol,
        triggers: PROTOCOL_TRIGGERS,
        required_companions: PROTOCOL_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-OPERATIONS",
        area: GovernanceArea::OperationRegistry,
        triggers: OPERATION_TRIGGERS,
        required_companions: OPERATION_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-GATES",
        area: GovernanceArea::GateRegistry,
        triggers: GATE_TRIGGERS,
        required_companions: GATE_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-FLOW",
        area: GovernanceArea::FlowPolicy,
        triggers: FLOW_TRIGGERS,
        required_companions: FLOW_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-HOOKS",
        area: GovernanceArea::Hooks,
        triggers: HOOK_TRIGGERS,
        required_companions: HOOK_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-DISTRIBUTION",
        area: GovernanceArea::Distribution,
        triggers: DISTRIBUTION_TRIGGERS,
        required_companions: DISTRIBUTION_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-RELEASE",
        area: GovernanceArea::Release,
        triggers: RELEASE_TRIGGERS,
        required_companions: RELEASE_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-COMPATIBILITY",
        area: GovernanceArea::Compatibility,
        triggers: COMPATIBILITY_TRIGGERS,
        required_companions: COMPATIBILITY_COMPANIONS,
    },
    AlignmentRule {
        id: "UG-SERVICE",
        area: GovernanceArea::ServiceLifecycle,
        triggers: SERVICE_TRIGGERS,
        required_companions: SERVICE_COMPANIONS,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeSet {
    paths: BTreeSet<Box<str>>,
}

impl ChangeSet {
    pub fn new(
        paths: impl IntoIterator<Item = impl Into<Box<str>>>,
    ) -> Result<Self, GovernanceError> {
        let paths: BTreeSet<Box<str>> = paths.into_iter().map(Into::into).collect();
        if paths.iter().any(|path| {
            path.is_empty()
                || path.starts_with('/')
                || path.contains('\\')
                || path
                    .split('/')
                    .any(|segment| matches!(segment, "" | "." | ".."))
        }) {
            return Err(GovernanceError::InvalidPath);
        }
        Ok(Self { paths })
    }

    #[must_use]
    pub fn contains_prefix(&self, prefix: &str) -> bool {
        if prefix.ends_with('/') {
            self.paths.iter().any(|path| path.starts_with(prefix))
        } else {
            self.paths.iter().any(|path| path.as_ref() == prefix)
        }
    }

    #[must_use]
    pub fn paths(&self) -> &BTreeSet<Box<str>> {
        &self.paths
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentFinding {
    pub rule_id: &'static str,
    pub area: GovernanceArea,
    pub missing_companion: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlignmentAudit {
    pub triggered_rules: BTreeSet<&'static str>,
    pub findings: Vec<AlignmentFinding>,
}

impl AlignmentAudit {
    #[must_use]
    pub fn run(changes: &ChangeSet) -> Self {
        let mut triggered_rules = BTreeSet::new();
        let mut findings = Vec::new();
        for rule in ALIGNMENT_RULES {
            if !rule
                .triggers
                .iter()
                .any(|trigger| changes.contains_prefix(trigger))
            {
                continue;
            }
            triggered_rules.insert(rule.id);
            for companion in rule.required_companions {
                if !changes.contains_prefix(companion) {
                    findings.push(AlignmentFinding {
                        rule_id: rule.id,
                        area: rule.area,
                        missing_companion: companion,
                    });
                }
            }
        }
        Self {
            triggered_rules,
            findings,
        }
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn receipt(
        &self,
        changes: &ChangeSet,
        policy_digest: &str,
    ) -> Result<GovernanceReceipt, GovernanceError> {
        if policy_digest.len() != 64
            || !policy_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(GovernanceError::InvalidPolicyDigest);
        }
        let changed_paths: Vec<_> = changes.paths.iter().map(Box::as_ref).collect();
        let change_set_digest = digest_json(&changed_paths)?;
        let triggered_rules = self.triggered_rules.iter().copied().collect();
        let findings = self.findings.clone();
        let receipt_digest = digest_json(&(
            RECEIPT_SCHEMA,
            policy_digest,
            change_set_digest.as_str(),
            &triggered_rules,
            &findings,
        ))?;
        Ok(GovernanceReceipt {
            schema_version: RECEIPT_SCHEMA,
            policy_digest: policy_digest.to_owned(),
            change_set_digest,
            triggered_rules,
            findings,
            passed: self.passed(),
            receipt_digest,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceReceipt {
    pub schema_version: &'static str,
    pub policy_digest: String,
    pub change_set_digest: String,
    pub triggered_rules: Vec<&'static str>,
    pub findings: Vec<AlignmentFinding>,
    pub passed: bool,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum GovernanceError {
    #[error("change set contains a non-canonical repository-relative path")]
    InvalidPath,
    #[error("policy digest must be 64 lowercase hexadecimal characters")]
    InvalidPolicyDigest,
    #[error("governance receipt serialization failed")]
    Encode,
}

fn digest_json(value: &impl Serialize) -> Result<String, GovernanceError> {
    let bytes = serde_json::to_vec(value).map_err(|_| GovernanceError::Encode)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_protocol_requires_constraint_and_contract_test_alignment() {
        let incomplete =
            ChangeSet::new(["crates/ae-sdd-protocol/src/method.rs"]).expect("valid change set");
        let audit = AlignmentAudit::run(&incomplete);
        assert!(!audit.passed());
        assert_eq!(audit.findings.len(), 2);

        let complete = ChangeSet::new([
            "crates/ae-sdd-protocol/src/method.rs",
            "constraints/api.md",
            "crates/ae-sdd-protocol/tests/protocol_contract.rs",
        ])
        .expect("valid change set");
        assert!(AlignmentAudit::run(&complete).passed());
    }

    #[test]
    fn unrelated_change_does_not_trigger_false_findings() {
        let changes = ChangeSet::new(["README.md"]).expect("valid change set");
        let audit = AlignmentAudit::run(&changes);
        assert!(audit.passed());
        assert!(audit.triggered_rules.is_empty());
    }

    #[test]
    fn file_companion_requires_exact_path_not_similar_prefix() {
        let changes = ChangeSet::new([
            "crates/ae-sdd-protocol/src/method.rs",
            "constraints/api.md.backup",
            "crates/ae-sdd-protocol/tests/protocol_contract.rs",
        ])
        .expect("valid paths");
        let audit = AlignmentAudit::run(&changes);
        assert!(
            audit
                .findings
                .iter()
                .any(|finding| finding.missing_companion == "constraints/api.md")
        );
    }

    #[test]
    fn receipt_is_deterministic_and_binds_policy_and_findings() {
        let changes = ChangeSet::new(["README.md"]).expect("valid paths");
        let audit = AlignmentAudit::run(&changes);
        let digest = "a".repeat(64);
        let first = audit.receipt(&changes, &digest).expect("receipt");
        let second = audit.receipt(&changes, &digest).expect("receipt");
        assert_eq!(first, second);
        assert!(first.passed);
        assert_eq!(first.receipt_digest.len(), 64);
    }
}
