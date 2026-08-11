//! Physical child claims are daemon-issued and delivered only on the Host lane.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;

use support::{
    Harness, open_root_session, params, register_workspace, result, session_params, stable_error,
};

const ADAPTER: &str = "host-claim-owner";
const CHILD: &str = "00000000-0000-0000-0000-000000001101";
const DECISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn only_the_claim_delivered_to_the_host_can_bootstrap_the_child() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "host-owned-claim");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"requirement-analysis",
            "requiredArtifacts":["RA"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("host-owned-claim-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();
    assert!(
        delegation.get("claimId").is_none(),
        "the Root response must not receive the child bootstrap claim: {delegation}"
    );

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .unwrap_or_else(|| panic!("Host delivery must carry the daemon-issued claim: {action}"))
        .to_owned();

    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001102",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-claim-owner",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("host-owned-claim-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let accept_params = |claim_id: &str, key: &str| {
        let mut accept = params(
            json!({
                "delegationId":delegation_id,
                "claimId":claim_id,
                "actionId":action["actionId"],
                "childSessionId":CHILD,
                "expiresAtUnixMs":4_900
            }),
            1_000,
        );
        accept.workspace_id = Some(workspace.workspace_id.clone());
        accept.work_item_id = Some("WORK".to_owned());
        accept.idempotency_key = Some(key.to_owned());
        accept
    };

    let forged = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(
            "00000000-0000-0000-0000-000000001103",
            "host-owned-claim-forged",
        ),
    );
    assert_eq!(
        stable_error(&forged),
        "DELEGATION_ATTESTATION_FAILED",
        "a caller-minted UUID must not be accepted: {forged}"
    );

    let accepted = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_params(&claim_id, "host-owned-claim-accept"),
    ));
    assert_eq!(accepted["status"], "running");
}

/// A2 host-native delegation has no field in the host's `SubagentStart`
/// payload that could disambiguate which of two concurrently pending Create
/// actions a given child belongs to (`hook_event_name`/`agent_id`/`agent_type`
/// only). Rather than guess via queue order, a second concurrent create from
/// the same root session must be rejected outright, which keeps the existing
/// FIFO `host.action_next` delivery exact instead of a heuristic.
#[test]
fn a_second_concurrent_delegation_create_from_the_same_root_session_is_allowed() {
    // ROUTE-C (Plan §2.4): the "at most one spawning delegation per root
    // session" guard is gone. Child Self-Claim makes concurrency safe because
    // each delegation carries its own daemon-minted claim_id, so FIFO queue
    // disambiguation is no longer the concurrency model. The same root session
    // may now open several spawning delegations at once; liveness and
    // preemption are owned by ROUTE-A's binding ledger, not by this gate.
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "concurrent-claim");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"requirement-analysis",
            "requiredArtifacts":["RA"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let create_params = |idempotency_key: &str| {
        let mut create = session_params(
            &workspace,
            &root,
            "root-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some("WORK".to_owned());
        create.idempotency_key = Some(idempotency_key.to_owned());
        create
    };

    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("concurrent-claim-create-1"),
    ));
    assert_eq!(first["status"], "spawning");
    let first_delegation_id = first["delegationId"].as_str().unwrap().to_owned();

    // A distinct idempotency key makes this a genuinely new create attempt,
    // not an idempotent replay of the first. It must now succeed: the
    // concurrency ceiling is lifted.
    let second = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("concurrent-claim-create-2"),
    ));
    assert_eq!(
        second["status"], "spawning",
        "a root session may hold multiple spawning delegations: {second}"
    );
    let second_delegation_id = second["delegationId"].as_str().unwrap().to_owned();
    assert_ne!(
        first_delegation_id, second_delegation_id,
        "the two concurrent creates must be distinct delegations"
    );

    // The first delegation is then accepted into `running`; the second stays
    // spawning. This proves sequence and concurrency coexist under the new
    // model — exactly the regression §2.4 / ROUTE-A revision 1 calls for.
    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001104",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-concurrent-claim",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("concurrent-claim-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let mut accept = params(
        json!({
            "delegationId":first_delegation_id,
            "claimId":claim_id,
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some("concurrent-claim-accept".to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    // A third create must also succeed now that the guard is gone — regardless
    // of whether an earlier delegation is still active. This is the
    // multi-active-binding regression pinned by ROUTE-A's ledger design.
    let third = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("concurrent-claim-create-3"),
    ));
    assert_eq!(third["status"], "spawning");
}

/// ROUTE-702d576a Task 2 Admission RED: duplicate-event case. A claim that
/// already produced a `running` delegation must replay the same receipt when
/// the exact same accept request arrives again (the host's `SubagentStart`
/// hook retrying after a lost response), never mint a second physical child
/// or otherwise mutate state a second time.
#[test]
fn a_duplicate_accept_of_the_same_claim_replays_the_original_receipt() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "duplicate-accept");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"requirement-analysis",
            "requiredArtifacts":["RA"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("duplicate-accept-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001105",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-duplicate-accept",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("duplicate-accept-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    let accept_once = || {
        let mut accept = params(
            json!({
                "delegationId":delegation_id,
                "claimId":claim_id,
                "actionId":action["actionId"],
                "childSessionId":CHILD,
                "expiresAtUnixMs":4_900
            }),
            1_000,
        );
        accept.workspace_id = Some(workspace.workspace_id.clone());
        accept.work_item_id = Some("WORK".to_owned());
        // Same idempotency key both times: this is the exact same request
        // arriving twice, not a second distinct accept attempt.
        accept.idempotency_key = Some("duplicate-accept-accept".to_owned());
        accept
    };

    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_once(),
    ));
    assert_eq!(first["status"], "running");

    let replay = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        accept_once(),
    ));
    assert_eq!(
        replay, first,
        "a replayed accept of the same claim must return the identical receipt, \
         never mint a second child or advance any further state"
    );
}

/// ROUTE-702d576a Task 2 Admission RED: wrong-parent case, corrected. The
/// first attempt at this test asserted that `DelegationAccept` must reject a
/// caller-supplied `sessionId`/`capabilityToken` that names a different root
/// session -- that assertion was wrong and has been withdrawn (see below);
/// this replacement asserts the actual security boundary that protects
/// against parent impersonation.
///
/// `DelegationAcceptPayload` (`model.rs`) carries no session/agent identity
/// field at all, and `delegation_accept` (`service_host.rs`) never calls
/// `session_identity()` -- unlike `delegation.collect`, which does check
/// `record.parent_session_id` against the caller. Accept's authority is the
/// `claimId` alone: `delegation_claim_digest` is computed from the *daemon's
/// own stored* `record.parent_session_id`, deadline, role, and other fields
/// baked in at `create` time, never from anything the accept caller supplies.
/// A caller cannot "impersonate the parent" by changing `sessionId` because
/// accept never reads that field; the actual protection is that the raw
/// `claimId` is delivered only through the authenticated `host.action_next`
/// lane and is never observable to an ordinary session (constraints/security.md
/// §四: claims never enter argv/env/logs/transcript).
///
/// So the real wrong-parent boundary to prove is: a claim minted for
/// delegation A must never authorize accepting delegation B, even when B's
/// own `delegationId`/`actionId`/`childSessionId` are supplied verbatim
/// alongside A's `claimId`. This is the cross-delegation confusion case,
/// already covered at the pure-function layer by
/// `a_claim_for_a_different_delegation_is_rejected` in
/// `ae-sdd-host/tests/host_ack_claim.rs`; this test proves the same
/// invariant holds through the full daemon-integration path.
#[test]
fn a_claim_minted_for_one_delegation_cannot_accept_a_different_one() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "wrong-parent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"requirement-analysis",
            "requiredArtifacts":["RA"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let create_params = |idempotency_key: &str| {
        let mut create = session_params(
            &workspace,
            &root,
            "root-agent",
            json!({"flowDecisionDigest":DECISION}),
            1_000,
        );
        create.work_item_id = Some("WORK".to_owned());
        create.idempotency_key = Some(idempotency_key.to_owned());
        create
    };

    // Delegation A: create, deliver, ACK -- but never accepted. Its claim is
    // the one the impersonation attempt below tries to reuse.
    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("wrong-parent-create-a"),
    ));
    let delegation_a_id = first["delegationId"]
        .as_str()
        .expect("delegation A id")
        .to_owned();
    let action_a = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_a_id = action_a["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack_a = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001107",
                "actionId":action_a["actionId"],
                "commandSeq":action_a["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-wrong-parent-a",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack_a.idempotency_key = Some("wrong-parent-ack-a".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack_a));

    // Complete delegation A so the concurrent-pending invariant does not
    // block delegation B's create below; A's claim remains valid to attempt
    // reuse against B regardless of A's own final state.
    let mut accept_a = params(
        json!({
            "delegationId":delegation_a_id,
            "claimId":claim_a_id,
            "actionId":action_a["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept_a.workspace_id = Some(workspace.workspace_id.clone());
    accept_a.work_item_id = Some("WORK".to_owned());
    accept_a.idempotency_key = Some("wrong-parent-accept-a".to_owned());
    let accepted_a =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept_a));
    assert_eq!(accepted_a["status"], "running");

    // Delegation B: a second, independent create/deliver/ACK cycle with its
    // own real claim.
    let second = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        create_params("wrong-parent-create-b"),
    ));
    let delegation_b_id = second["delegationId"]
        .as_str()
        .expect("delegation B id")
        .to_owned();
    let action_b = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let mut ack_b = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001108",
                "actionId":action_b["actionId"],
                "commandSeq":action_b["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-wrong-parent-b",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack_b.idempotency_key = Some("wrong-parent-ack-b".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack_b));

    // The impersonation attempt: accept delegation B using delegation A's
    // claim, with B's own actionId/childSessionId supplied verbatim.
    let mut confused_accept = params(
        json!({
            "delegationId":delegation_b_id,
            "claimId":claim_a_id,
            "actionId":action_b["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    confused_accept.workspace_id = Some(workspace.workspace_id.clone());
    confused_accept.work_item_id = Some("WORK".to_owned());
    confused_accept.idempotency_key = Some("wrong-parent-accept-confused".to_owned());

    let rejected = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        confused_accept,
    );
    assert_eq!(
        stable_error(&rejected),
        "DELEGATION_ATTESTATION_FAILED",
        "delegation A's claim must never authorize accepting delegation B: {rejected}"
    );
}

/// ROUTE-702d576a Task 2 Admission RED: replay-with-different-payload case.
/// The same idempotency key reused with a materially different payload
/// (a different `childSessionId`, here) must be rejected outright, never
/// silently accepted as if it were the original request nor treated as a
/// safe replay.
#[test]
fn reusing_an_accept_idempotency_key_with_a_different_payload_is_rejected() {
    let harness = Harness::new(RuntimeConfig::default());
    let mut host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "replay-payload");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some("WORK"),
    );

    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":1,
        "phase":"initialized",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"requirement-analysis",
            "requiredArtifacts":["RA"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some("WORK".to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some("WORK".to_owned());
    create.idempotency_key = Some("replay-payload-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    let delegation_id = delegation["delegationId"]
        .as_str()
        .expect("delegation id")
        .to_owned();

    let action = result(&harness.call(
        &mut host,
        RpcMethod::HostActionNext,
        params(json!({"adapterId":ADAPTER}), 1_000),
    ));
    let claim_id = action["claimId"]
        .as_str()
        .expect("Host delivery carries the daemon-issued claim")
        .to_owned();
    let mut ack = params(
        json!({
            "adapterId":ADAPTER,
            "ack": {
                "ackId":"00000000-0000-0000-0000-000000001109",
                "actionId":action["actionId"],
                "commandSeq":action["commandSeq"],
                "outcome":"accepted",
                "hostTaskId":"host-task-replay-payload",
                "sessionId":CHILD
            }
        }),
        1_000,
    );
    ack.idempotency_key = Some("replay-payload-ack".to_owned());
    let _ = result(&harness.call(&mut host, RpcMethod::HostActionAck, ack));

    const SHARED_KEY: &str = "replay-payload-accept";
    let mut accept = params(
        json!({
            "delegationId":delegation_id,
            "claimId":claim_id,
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_900
        }),
        1_000,
    );
    accept.workspace_id = Some(workspace.workspace_id.clone());
    accept.work_item_id = Some("WORK".to_owned());
    accept.idempotency_key = Some(SHARED_KEY.to_owned());
    let accepted = result(&harness.call(&mut root_connection, RpcMethod::DelegationAccept, accept));
    assert_eq!(accepted["status"], "running");

    // Same key, but a materially different payload (a different, otherwise
    // well-formed, expiresAtUnixMs): this must be rejected as a reused key,
    // not silently accepted and not treated as an idempotent replay of the
    // original request.
    let mut different_payload = params(
        json!({
            "delegationId":delegation_id,
            "claimId":claim_id,
            "actionId":action["actionId"],
            "childSessionId":CHILD,
            "expiresAtUnixMs":4_901
        }),
        1_000,
    );
    different_payload.workspace_id = Some(workspace.workspace_id.clone());
    different_payload.work_item_id = Some("WORK".to_owned());
    different_payload.idempotency_key = Some(SHARED_KEY.to_owned());

    let rejected = harness.call(
        &mut root_connection,
        RpcMethod::DelegationAccept,
        different_payload,
    );
    assert_eq!(
        stable_error(&rejected),
        "IDEMPOTENCY_KEY_REUSED",
        "the same idempotency key with a different canonical payload must \
         never be treated as the original request or a safe replay: {rejected}"
    );
}
