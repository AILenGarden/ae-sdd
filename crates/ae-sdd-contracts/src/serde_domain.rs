//! Checked serde adapters for domain-owned semantic types.

#![allow(dead_code)] // C0 freezes adapters consumed incrementally by independent Parts.

use std::str::FromStr;

use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, BootId, CapabilityId, ClaimId, CompactId,
    ContextDigest, ContextGeneration, ContextProjectionId, DecisionDigest, DelegationId,
    DeliverableContract, DeliverableRequirement, DesignRoute, EventSequence, EventStoreId,
    EvidenceDigest, EvidenceId, EvidenceRef, GateId, HostAckId, HostActionId, InputFingerprint,
    InventoryGeneration, JobId, LeaseId, OperationId, PolicyDigest, ProcessPhase, ProjectKey,
    ProjectPathScope, ProjectRelativePath, RequestId, ResultDigest, SampleSequence, SessionId,
    StateRevision, StoryId, ToolchainDigest, TurnId, TurnSequence, VerificationId, WorkItemId,
    WorkScale, WorkspaceId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

macro_rules! display_parse_adapter {
    ($module:ident, $type:ty) => {
        pub(crate) mod $module {
            use super::*;

            pub(crate) fn serialize<S>(value: &$type, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&value.to_string())
            }

            pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<$type, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                <$type>::from_str(&value).map_err(de::Error::custom)
            }
        }
    };
}

display_parse_adapter!(artifact_digest, ArtifactDigest);
display_parse_adapter!(decision_digest, DecisionDigest);
display_parse_adapter!(input_fingerprint, InputFingerprint);
display_parse_adapter!(context_digest, ContextDigest);
display_parse_adapter!(evidence_digest, EvidenceDigest);
display_parse_adapter!(policy_digest, PolicyDigest);
display_parse_adapter!(result_digest, ResultDigest);
display_parse_adapter!(toolchain_digest, ToolchainDigest);
display_parse_adapter!(boot_id, BootId);
display_parse_adapter!(claim_id, ClaimId);
display_parse_adapter!(compact_id, CompactId);
display_parse_adapter!(context_projection_id, ContextProjectionId);
display_parse_adapter!(delegation_id, DelegationId);
display_parse_adapter!(event_store_id, EventStoreId);
display_parse_adapter!(host_ack_id, HostAckId);
display_parse_adapter!(host_action_id, HostActionId);
display_parse_adapter!(job_id, JobId);
display_parse_adapter!(lease_id, LeaseId);
display_parse_adapter!(request_id, RequestId);
display_parse_adapter!(session_id, SessionId);
display_parse_adapter!(turn_id, TurnId);
display_parse_adapter!(workspace_id, WorkspaceId);
display_parse_adapter!(capability_id, CapabilityId);
display_parse_adapter!(evidence_id, EvidenceId);
display_parse_adapter!(gate_id, GateId);
display_parse_adapter!(operation_id, OperationId);
display_parse_adapter!(project_key, ProjectKey);
display_parse_adapter!(story_id, StoryId);
display_parse_adapter!(verification_id, VerificationId);

macro_rules! counter_adapter {
    ($module:ident, $type:ty) => {
        pub(crate) mod $module {
            use super::*;

            pub(crate) fn serialize<S>(value: &$type, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_u64(value.get())
            }

            pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<$type, D::Error>
            where
                D: Deserializer<'de>,
            {
                Ok(<$type>::new(u64::deserialize(deserializer)?))
            }
        }
    };
}

counter_adapter!(context_generation, ContextGeneration);
counter_adapter!(event_sequence, EventSequence);
counter_adapter!(inventory_generation, InventoryGeneration);
counter_adapter!(sample_sequence, SampleSequence);
counter_adapter!(state_revision, StateRevision);
counter_adapter!(turn_sequence, TurnSequence);

pub(crate) mod work_item_id {
    use super::*;

    pub(crate) fn serialize<S>(value: &WorkItemId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.as_str())
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<WorkItemId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        WorkItemId::new(value).map_err(de::Error::custom)
    }
}

pub(crate) mod work_scale {
    use super::*;

    pub(crate) fn serialize<S>(value: &WorkScale, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match value {
            WorkScale::Large => "large",
            WorkScale::Medium => "medium",
            WorkScale::Small => "small",
            WorkScale::Micro => "micro",
        })
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<WorkScale, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "large" => Ok(WorkScale::Large),
            "medium" => Ok(WorkScale::Medium),
            "small" => Ok(WorkScale::Small),
            "micro" => Ok(WorkScale::Micro),
            _ => Err(de::Error::custom("unknown work scale")),
        }
    }
}

pub(crate) mod design_route {
    use super::*;

    pub(crate) fn serialize<S>(value: &DesignRoute, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match value {
            DesignRoute::Dr => "dr",
            DesignRoute::Story => "story",
            DesignRoute::CodingPlan => "coding_plan",
        })
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<DesignRoute, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "dr" => Ok(DesignRoute::Dr),
            "story" => Ok(DesignRoute::Story),
            "coding_plan" => Ok(DesignRoute::CodingPlan),
            _ => Err(de::Error::custom("unknown design route")),
        }
    }
}

pub(crate) mod agent_role {
    use super::*;

    pub(crate) fn serialize<S>(value: &AgentRole, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match value {
            AgentRole::Root => "root",
            AgentRole::Series => "series",
            AgentRole::Task => "task",
            AgentRole::Reviewer => "reviewer",
        })
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<AgentRole, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "root" => Ok(AgentRole::Root),
            "series" => Ok(AgentRole::Series),
            "task" => Ok(AgentRole::Task),
            "reviewer" => Ok(AgentRole::Reviewer),
            _ => Err(de::Error::custom("unknown Agent role")),
        }
    }
}

pub(crate) mod process_phase {
    use super::*;

    pub(crate) fn serialize<S>(value: &ProcessPhase, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match value {
            ProcessPhase::Initialized => "initialized",
            ProcessPhase::RouteSelected => "route_selected",
            ProcessPhase::RequirementAnalyzed => "requirement_analyzed",
            ProcessPhase::DrGenerated => "dr_generated",
            ProcessPhase::StoryGenerated => "story_generated",
            ProcessPhase::TestcaseGenerated => "testcase_generated",
            ProcessPhase::CodingProcess => "coding_process",
            ProcessPhase::Coding => "coding",
            ProcessPhase::TestRunning => "test_running",
            ProcessPhase::CodeReviewed => "code_reviewed",
            ProcessPhase::Completed => "completed",
            ProcessPhase::Paused => "paused",
        })
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<ProcessPhase, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "initialized" => Ok(ProcessPhase::Initialized),
            "route_selected" => Ok(ProcessPhase::RouteSelected),
            "requirement_analyzed" => Ok(ProcessPhase::RequirementAnalyzed),
            "dr_generated" => Ok(ProcessPhase::DrGenerated),
            "story_generated" => Ok(ProcessPhase::StoryGenerated),
            "testcase_generated" => Ok(ProcessPhase::TestcaseGenerated),
            "coding_process" => Ok(ProcessPhase::CodingProcess),
            "coding" => Ok(ProcessPhase::Coding),
            "test_running" => Ok(ProcessPhase::TestRunning),
            "code_reviewed" => Ok(ProcessPhase::CodeReviewed),
            "completed" => Ok(ProcessPhase::Completed),
            "paused" => Ok(ProcessPhase::Paused),
            _ => Err(de::Error::custom("unknown process phase")),
        }
    }
}

pub(crate) mod project_relative_path {
    use super::*;

    pub(crate) fn serialize<S>(
        value: &ProjectRelativePath,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.as_str())
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<ProjectRelativePath, D::Error>
    where
        D: Deserializer<'de>,
    {
        ProjectRelativePath::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProjectPathScopeWire {
    ProjectRoot,
    Subtree { path: String },
}

pub(crate) mod project_path_scope {
    use super::*;

    pub(crate) fn serialize<S>(value: &ProjectPathScope, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            ProjectPathScope::ProjectRoot => ProjectPathScopeWire::ProjectRoot,
            ProjectPathScope::Subtree(path) => ProjectPathScopeWire::Subtree {
                path: path.to_string(),
            },
        }
        .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<ProjectPathScope, D::Error>
    where
        D: Deserializer<'de>,
    {
        match ProjectPathScopeWire::deserialize(deserializer)? {
            ProjectPathScopeWire::ProjectRoot => Ok(ProjectPathScope::ProjectRoot),
            ProjectPathScopeWire::Subtree { path } => ProjectRelativePath::new(path)
                .map(ProjectPathScope::Subtree)
                .map_err(de::Error::custom),
        }
    }
}

pub(crate) mod project_path_scopes {
    use super::*;

    pub(crate) fn serialize<S>(value: &[ProjectPathScope], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let values: Vec<ProjectPathScopeWire> = value
            .iter()
            .map(|scope| match scope {
                ProjectPathScope::ProjectRoot => ProjectPathScopeWire::ProjectRoot,
                ProjectPathScope::Subtree(path) => ProjectPathScopeWire::Subtree {
                    path: path.to_string(),
                },
            })
            .collect();
        values.serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ProjectPathScope>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<ProjectPathScopeWire>::deserialize(deserializer)?
            .into_iter()
            .map(|value| match value {
                ProjectPathScopeWire::ProjectRoot => Ok(ProjectPathScope::ProjectRoot),
                ProjectPathScopeWire::Subtree { path } => ProjectRelativePath::new(path)
                    .map(ProjectPathScope::Subtree)
                    .map_err(de::Error::custom),
            })
            .collect()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArtifactRefWire {
    kind: String,
    path: String,
    digest: String,
    byte_length: u64,
}

impl ArtifactRefWire {
    fn from_domain(value: &ArtifactRef) -> Self {
        Self {
            kind: value.kind().to_string(),
            path: value.path().to_string(),
            digest: value.digest().to_string(),
            byte_length: value.byte_length(),
        }
    }

    fn into_domain(self) -> Result<ArtifactRef, String> {
        let kind = ArtifactKind::new(self.kind).map_err(|error| error.to_string())?;
        let path = ProjectRelativePath::new(self.path).map_err(|error| error.to_string())?;
        let digest = ArtifactDigest::from_str(&self.digest).map_err(|error| error.to_string())?;
        Ok(ArtifactRef::new(kind, path, digest, self.byte_length))
    }
}

pub(crate) mod artifact_ref {
    use super::*;

    pub(crate) fn serialize<S>(value: &ArtifactRef, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ArtifactRefWire::from_domain(value).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<ArtifactRef, D::Error>
    where
        D: Deserializer<'de>,
    {
        ArtifactRefWire::deserialize(deserializer)?
            .into_domain()
            .map_err(de::Error::custom)
    }
}

pub(crate) mod optional_artifact_ref {
    use super::*;

    pub(crate) fn serialize<S>(
        value: &Option<ArtifactRef>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .map(ArtifactRefWire::from_domain)
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<ArtifactRef>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<ArtifactRefWire>::deserialize(deserializer)?
            .map(ArtifactRefWire::into_domain)
            .transpose()
            .map_err(de::Error::custom)
    }
}

pub(crate) mod artifact_refs {
    use super::*;

    pub(crate) fn serialize<S>(value: &[ArtifactRef], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ArtifactRefWire::from_domain)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<ArtifactRef>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<ArtifactRefWire>::deserialize(deserializer)?
            .into_iter()
            .map(|value| value.into_domain().map_err(de::Error::custom))
            .collect()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceRefWire {
    evidence_id: String,
    verification_id: String,
    path: String,
    digest: String,
    byte_length: u64,
}

impl EvidenceRefWire {
    fn from_domain(value: &EvidenceRef) -> Self {
        Self {
            evidence_id: value.evidence_id().to_string(),
            verification_id: value.verification_id().to_string(),
            path: value.path().to_string(),
            digest: value.digest().to_string(),
            byte_length: value.byte_length(),
        }
    }

    fn into_domain(self) -> Result<EvidenceRef, String> {
        Ok(EvidenceRef::new(
            EvidenceId::new(self.evidence_id).map_err(|error| error.to_string())?,
            VerificationId::new(self.verification_id).map_err(|error| error.to_string())?,
            ProjectRelativePath::new(self.path).map_err(|error| error.to_string())?,
            EvidenceDigest::from_str(&self.digest).map_err(|error| error.to_string())?,
            self.byte_length,
        ))
    }
}

pub(crate) mod evidence_refs {
    use super::*;

    pub(crate) fn serialize<S>(value: &[EvidenceRef], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(EvidenceRefWire::from_domain)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<EvidenceRef>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<EvidenceRefWire>::deserialize(deserializer)?
            .into_iter()
            .map(|value| value.into_domain().map_err(de::Error::custom))
            .collect()
    }
}

pub(crate) mod verification_ids {
    use super::*;

    pub(crate) fn serialize<S>(value: &[VerificationId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<VerificationId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| VerificationId::new(value).map_err(de::Error::custom))
            .collect()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeliverableRequirementWire {
    id: String,
    kind: String,
    path: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeliverableContractWire {
    required: Vec<DeliverableRequirementWire>,
    max_result_bytes: u32,
    max_summary_bytes: u32,
}

impl DeliverableContractWire {
    fn from_domain(value: &DeliverableContract) -> Self {
        Self {
            required: value
                .required()
                .values()
                .map(|requirement| DeliverableRequirementWire {
                    id: requirement.id().to_string(),
                    kind: requirement.kind().to_string(),
                    path: requirement.path().to_string(),
                })
                .collect(),
            max_result_bytes: value.max_result_bytes(),
            max_summary_bytes: value.max_summary_bytes(),
        }
    }

    fn into_domain(self) -> Result<DeliverableContract, String> {
        let required = self
            .required
            .into_iter()
            .map(|requirement| {
                Ok(DeliverableRequirement::new(
                    ae_sdd_domain::DeliverableId::new(requirement.id)
                        .map_err(|error| error.to_string())?,
                    ArtifactKind::new(requirement.kind).map_err(|error| error.to_string())?,
                    ProjectRelativePath::new(requirement.path)
                        .map_err(|error| error.to_string())?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        DeliverableContract::new(required, self.max_result_bytes, self.max_summary_bytes)
            .map_err(|error| error.to_string())
    }
}

pub(crate) mod deliverable_contract {
    use super::*;

    pub(crate) fn serialize<S>(
        value: &DeliverableContract,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DeliverableContractWire::from_domain(value).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<DeliverableContract, D::Error>
    where
        D: Deserializer<'de>,
    {
        DeliverableContractWire::deserialize(deserializer)?
            .into_domain()
            .map_err(de::Error::custom)
    }
}
