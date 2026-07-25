//! `SessionBootstrapPort::bootstrap` pure decision implementation.

use ae_sdd_contracts::session::SessionBootstrapRequest;
use sha2::{Digest, Sha256};

use crate::error::SessionBootstrapError;
use crate::model::{BootstrapPlan, BootstrapSnapshot, BootstrapStep};

/// Decides the ordered bootstrap plan for a session bootstrap request,
/// given an explicit, already-resolved snapshot of the three facts the
/// decision depends on. Performs no I/O and reads no ambient state: every
/// fact the decision needs is either in `request` or `snapshot`.
///
/// # Errors
///
/// Returns [`SessionBootstrapError::CrossWorkspaceSession`] when
/// `snapshot.existing_session()` is bound to a workspace other than
/// `request.workspace_id()`. The frozen C0 root/delegation invariant is
/// already enforced by `SessionBootstrapRequest::new` before a request can
/// reach this function, so it is not re-checked here.
pub fn bootstrap(
    request: &SessionBootstrapRequest,
    snapshot: &BootstrapSnapshot,
) -> Result<BootstrapPlan, SessionBootstrapError> {
    if let Some(existing) = snapshot.existing_session()
        && existing.workspace_id() != request.workspace_id()
    {
        return Err(SessionBootstrapError::CrossWorkspaceSession);
    }

    let mut steps = Vec::with_capacity(4);
    if !snapshot.workspace_registered() {
        steps.push(BootstrapStep::RegisterWorkspace);
    }
    if snapshot.existing_session().is_none() {
        steps.push(BootstrapStep::OpenSession);
    }
    steps.push(BootstrapStep::GrantScope);
    if !snapshot.context_ready() {
        steps.push(BootstrapStep::ProjectContext);
    }

    let plan_digest = canonical_digest(request, snapshot, &steps);
    Ok(BootstrapPlan::new(steps, plan_digest))
}

fn canonical_digest(
    request: &SessionBootstrapRequest,
    snapshot: &BootstrapSnapshot,
    steps: &[BootstrapStep],
) -> [u8; 32] {
    let mut encoder = Encoder::new(b"ae-sdd-session-bootstrap-plan/v1");

    encoder.string(&request.workspace_id().to_string());
    encoder.string(request.external_session_key().as_str());
    encoder.string(request.adapter_id().as_str());
    encoder.byte(role_tag(request.role()));
    encoder.boolean(request.engaged());
    match request.delegation_id() {
        Some(delegation_id) => {
            encoder.boolean(true);
            encoder.string(&delegation_id.to_string());
        }
        None => encoder.boolean(false),
    }
    encoder.usize(request.capabilities().len());
    for capability in request.capabilities() {
        encoder.string(capability.as_str());
    }
    match request.context_bundle_id() {
        Some(context_bundle_id) => {
            encoder.boolean(true);
            encoder.string(context_bundle_id.as_str());
        }
        None => encoder.boolean(false),
    }

    encoder.boolean(snapshot.workspace_registered());
    match snapshot.existing_session() {
        Some(existing) => {
            encoder.boolean(true);
            encoder.string(&existing.session_id().to_string());
            encoder.string(&existing.workspace_id().to_string());
        }
        None => encoder.boolean(false),
    }
    encoder.boolean(snapshot.context_ready());

    encoder.usize(steps.len());
    for step in steps {
        encoder.byte(step_tag(*step));
    }

    encoder.finish()
}

const fn role_tag(role: ae_sdd_domain::AgentRole) -> u8 {
    match role {
        ae_sdd_domain::AgentRole::Root => 1,
        ae_sdd_domain::AgentRole::Series => 2,
        ae_sdd_domain::AgentRole::Task => 3,
        ae_sdd_domain::AgentRole::Reviewer => 4,
    }
}

const fn step_tag(step: BootstrapStep) -> u8 {
    match step {
        BootstrapStep::RegisterWorkspace => 1,
        BootstrapStep::OpenSession => 2,
        BootstrapStep::GrantScope => 3,
        BootstrapStep::ProjectContext => 4,
    }
}

/// Minimal length-prefixed canonical encoder, matching the pattern used by
/// `ae-sdd-lifecycle::canonical::Encoder`: a fixed domain tag plus explicit
/// length prefixes for every variable-length field, so no two distinct
/// logical inputs can ever collide onto the same byte stream.
struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new(tag: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(tag);
        bytes.push(0);
        Self { bytes }
    }

    fn finish(self) -> [u8; 32] {
        Sha256::digest(&self.bytes).into()
    }

    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn usize(&mut self, value: usize) {
        let encoded = u64::try_from(value).unwrap_or(u64::MAX);
        self.u64(encoded);
    }

    fn string(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes.extend_from_slice(value.as_bytes());
    }
}
