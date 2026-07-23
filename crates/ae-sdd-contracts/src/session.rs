//! Frozen workspace and Agent session bootstrap contracts.

use ae_sdd_domain::{
    AgentRole, BootId, CapabilityId, ContextGeneration, DelegationId, InventoryGeneration,
    SessionId, WorkspaceId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

use crate::{
    AdapterId, BoundedText, ContextBundleId, ContractValueError, ExternalSessionKey, SchemaVersion,
    serde_domain,
};

/// Maximum number of capabilities negotiated by one session bootstrap.
pub const MAX_SESSION_CAPABILITIES: usize = 32;

/// Maximum encoded byte length of a boot-signed session capability token.
pub const MAX_CAPABILITY_TOKEN_BYTES: usize = 8_192;

/// Strict request used to bind a host conversation to a daemon session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionBootstrapRequest {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::workspace_id")]
    workspace_id: WorkspaceId,
    external_session_key: ExternalSessionKey,
    adapter_id: AdapterId,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    engaged: bool,
    #[serde(with = "optional_delegation_id")]
    delegation_id: Option<DelegationId>,
    #[serde(with = "capability_ids")]
    capabilities: Vec<CapabilityId>,
    context_bundle_id: Option<ContextBundleId>,
}

impl SessionBootstrapRequest {
    /// Validates and constructs a bootstrap request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        workspace_id: WorkspaceId,
        external_session_key: ExternalSessionKey,
        adapter_id: AdapterId,
        role: AgentRole,
        engaged: bool,
        delegation_id: Option<DelegationId>,
        mut capabilities: Vec<CapabilityId>,
        context_bundle_id: Option<ContextBundleId>,
    ) -> Result<Self, SessionContractError> {
        match (role, delegation_id) {
            (AgentRole::Root, Some(_)) => {
                return Err(SessionContractError::RootDelegationForbidden);
            }
            (AgentRole::Root, None) => {}
            (_, None) => return Err(SessionContractError::DelegationRequired),
            (_, Some(_)) => {}
        }
        if capabilities.len() > MAX_SESSION_CAPABILITIES {
            return Err(SessionContractError::TooManyCapabilities {
                maximum: MAX_SESSION_CAPABILITIES,
                actual: capabilities.len(),
            });
        }
        capabilities.sort();
        if capabilities.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(SessionContractError::DuplicateCapability);
        }

        Ok(Self {
            schema_version,
            workspace_id,
            external_session_key,
            adapter_id,
            role,
            engaged,
            delegation_id,
            capabilities,
            context_bundle_id,
        })
    }

    /// Returns the contract schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the registered workspace identity.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Returns the host-stable external conversation key.
    #[must_use]
    pub const fn external_session_key(&self) -> &ExternalSessionKey {
        &self.external_session_key
    }

    /// Returns the authenticated host adapter identity.
    #[must_use]
    pub const fn adapter_id(&self) -> &AdapterId {
        &self.adapter_id
    }

    /// Returns the daemon-owned Agent role.
    #[must_use]
    pub const fn role(&self) -> AgentRole {
        self.role
    }

    /// Returns whether fail-closed Hook control is engaged.
    #[must_use]
    pub const fn engaged(&self) -> bool {
        self.engaged
    }

    /// Returns the required physical delegation binding for a child session.
    #[must_use]
    pub const fn delegation_id(&self) -> Option<DelegationId> {
        self.delegation_id
    }

    /// Returns the sorted, unique negotiated capability list.
    #[must_use]
    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    /// Returns the optional precomputed context bundle identity.
    #[must_use]
    pub const fn context_bundle_id(&self) -> Option<&ContextBundleId> {
        self.context_bundle_id.as_ref()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionBootstrapRequestWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::workspace_id")]
    workspace_id: WorkspaceId,
    external_session_key: ExternalSessionKey,
    adapter_id: AdapterId,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    engaged: bool,
    #[serde(with = "optional_delegation_id")]
    delegation_id: Option<DelegationId>,
    #[serde(with = "capability_ids")]
    capabilities: Vec<CapabilityId>,
    context_bundle_id: Option<ContextBundleId>,
}

impl<'de> Deserialize<'de> for SessionBootstrapRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SessionBootstrapRequestWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.workspace_id,
            wire.external_session_key,
            wire.adapter_id,
            wire.role,
            wire.engaged,
            wire.delegation_id,
            wire.capabilities,
            wire.context_bundle_id,
        )
        .map_err(de::Error::custom)
    }
}

/// Successful, boot-bound session bootstrap response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionBootstrapResponse {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::boot_id")]
    boot_id: BootId,
    #[serde(with = "serde_domain::workspace_id")]
    workspace_id: WorkspaceId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    engaged: bool,
    #[serde(with = "serde_domain::context_generation")]
    context_generation: ContextGeneration,
    #[serde(with = "serde_domain::inventory_generation")]
    inventory_generation: InventoryGeneration,
    capability_token: BoundedText<MAX_CAPABILITY_TOKEN_BYTES>,
    expires_at_unix_ms: u64,
}

impl SessionBootstrapResponse {
    /// Validates and constructs a bootstrap response.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: SchemaVersion,
        boot_id: BootId,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        role: AgentRole,
        engaged: bool,
        context_generation: ContextGeneration,
        inventory_generation: InventoryGeneration,
        capability_token: impl Into<Box<str>>,
        expires_at_unix_ms: u64,
    ) -> Result<Self, SessionContractError> {
        let capability_token = BoundedText::new(capability_token)?;
        if capability_token.is_empty() {
            return Err(SessionContractError::EmptyCapabilityToken);
        }
        if expires_at_unix_ms == 0 {
            return Err(SessionContractError::ZeroExpiry);
        }
        Ok(Self {
            schema_version,
            boot_id,
            workspace_id,
            session_id,
            role,
            engaged,
            context_generation,
            inventory_generation,
            capability_token,
            expires_at_unix_ms,
        })
    }

    /// Returns the contract schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the daemon boot identity that signed the capability.
    #[must_use]
    pub const fn boot_id(&self) -> BootId {
        self.boot_id
    }

    /// Returns the registered workspace identity.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Returns the trusted daemon session identity.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Returns the daemon-owned Agent role.
    #[must_use]
    pub const fn role(&self) -> AgentRole {
        self.role
    }

    /// Returns whether fail-closed Hook control is engaged.
    #[must_use]
    pub const fn engaged(&self) -> bool {
        self.engaged
    }

    /// Returns the current context generation.
    #[must_use]
    pub const fn context_generation(&self) -> ContextGeneration {
        self.context_generation
    }

    /// Returns the inventory generation captured at bootstrap.
    #[must_use]
    pub const fn inventory_generation(&self) -> InventoryGeneration {
        self.inventory_generation
    }

    /// Returns the bounded boot-signed capability token.
    #[must_use]
    pub const fn capability_token(&self) -> &BoundedText<MAX_CAPABILITY_TOKEN_BYTES> {
        &self.capability_token
    }

    /// Returns the absolute capability expiry timestamp.
    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionBootstrapResponseWire {
    schema_version: SchemaVersion,
    #[serde(with = "serde_domain::boot_id")]
    boot_id: BootId,
    #[serde(with = "serde_domain::workspace_id")]
    workspace_id: WorkspaceId,
    #[serde(with = "serde_domain::session_id")]
    session_id: SessionId,
    #[serde(with = "serde_domain::agent_role")]
    role: AgentRole,
    engaged: bool,
    #[serde(with = "serde_domain::context_generation")]
    context_generation: ContextGeneration,
    #[serde(with = "serde_domain::inventory_generation")]
    inventory_generation: InventoryGeneration,
    capability_token: BoundedText<MAX_CAPABILITY_TOKEN_BYTES>,
    expires_at_unix_ms: u64,
}

impl<'de> Deserialize<'de> for SessionBootstrapResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SessionBootstrapResponseWire::deserialize(deserializer)?;
        Self::new(
            wire.schema_version,
            wire.boot_id,
            wire.workspace_id,
            wire.session_id,
            wire.role,
            wire.engaged,
            wire.context_generation,
            wire.inventory_generation,
            wire.capability_token.as_str(),
            wire.expires_at_unix_ms,
        )
        .map_err(de::Error::custom)
    }
}

/// Validation error for the frozen session bootstrap contract.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SessionContractError {
    /// A root session attempted to claim a child delegation.
    #[error("root session cannot carry a delegation binding")]
    RootDelegationForbidden,
    /// A non-root session omitted its physical delegation binding.
    #[error("non-root session requires a delegation binding")]
    DelegationRequired,
    /// Capability negotiation exceeded its fixed array bound.
    #[error("session capability count exceeds {maximum} (actual: {actual})")]
    TooManyCapabilities {
        /// Maximum accepted capability count.
        maximum: usize,
        /// Observed capability count.
        actual: usize,
    },
    /// The same capability appeared more than once.
    #[error("session capability list contains a duplicate")]
    DuplicateCapability,
    /// The session capability token was empty.
    #[error("session capability token cannot be empty")]
    EmptyCapabilityToken,
    /// The session expiry timestamp was zero.
    #[error("session expiry timestamp must be greater than zero")]
    ZeroExpiry,
    /// A bounded contract value was invalid.
    #[error(transparent)]
    ContractValue(#[from] ContractValueError),
}

mod capability_ids {
    use super::*;

    pub(super) fn serialize<S>(values: &[CapabilityId], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<CapabilityId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| CapabilityId::new(value).map_err(de::Error::custom))
            .collect()
    }
}

mod optional_delegation_id {
    use std::str::FromStr;

    use super::*;

    pub(super) fn serialize<S>(
        value: &Option<DelegationId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|id| id.to_string()).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<DelegationId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| DelegationId::from_str(&value).map_err(de::Error::custom))
            .transpose()
    }
}
