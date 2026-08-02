//! Root-to-Series delegation consumes a committed daemon flow reference.

mod support;

use ae_sdd_protocol::{ClientKind, RpcMethod};
use ae_sdd_runtime::{PersistencePort, RuntimeConfig};
use serde_json::json;

use support::{
    Harness, open_root_session, register_workspace, result, session_params, stable_error,
};

const ADAPTER: &str = "host-series-intent";
const WORK_ITEM: &str = "WORK";
const DECISION: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

#[test]
fn root_references_committed_flow_intent_instead_of_supplying_authority_fields() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert_eq!(flow["decisionDigest"], DECISION);

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));
    assert_eq!(delegation["childRole"], "series");
    assert!(
        delegation["grant"]["operations"]
            .as_array()
            .is_some_and(|operations| operations.iter().any(|value| value == "document.save")),
        "daemon policy must derive the semantic Series grant: {delegation}"
    );
    assert_eq!(delegation["status"], "spawning");

    let mut forged = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({
            "childRole":"series",
            "parentDelegationId":null,
            "inputRevision":7,
            "inputFingerprint":DECISION,
            "deadlineUnixMs":5_000,
            "adapterId":ADAPTER,
            "grant":{"operations":[],"capabilities":[],"paths":[]}
        }),
        1_000,
    );
    forged.work_item_id = Some(WORK_ITEM.to_owned());
    forged.idempotency_key = Some("series-intent-forged-authority".to_owned());
    let rejected = harness.call(&mut root_connection, RpcMethod::DelegationCreate, forged);
    assert_eq!(stable_error(&rejected), "OPERATION_SCHEMA_INVALID");
}

#[test]
fn root_can_delegate_daemon_committed_coding_work_without_supplying_authority() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "coding-series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "coding-root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":9,
        "phase":"coding",
        "nextAction":{"kind":"await-agent-work"}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("coding-series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(delegation["childRole"], "series");
    assert_eq!(
        delegation["briefing"],
        "Execute the daemon-committed coding Series"
    );
}

#[test]
fn root_can_delegate_daemon_committed_testing_work_without_supplying_authority() {
    const TEST_DECISION: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "testing-series-intent");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "testing-root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":TEST_DECISION,
        "stateRevision":10,
        "phase":"test-running",
        "nextAction":{"kind":"await-agent-work"}
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":TEST_DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("testing-series-intent-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(delegation["childRole"], "series");
    assert_eq!(
        delegation["briefing"],
        "Execute the daemon-committed testing Series"
    );
}

/// `ae-sdd-daemon-audit-report.md` F-10: the flow decision carries
/// `inputFingerprint` and `decisionDigest` as separate proofs, but the committed
/// intent copied the decision digest into the fingerprint slot and then required
/// the two to be equal. That makes input freshness unobservable — the same
/// decision taken against newer Spec content produces an identical fingerprint,
/// so nothing can tell the two apart.
///
/// Here the flow reports a fingerprint that genuinely differs from the decision
/// digest, which is the normal case once the fingerprint is computed from state
/// revision, DocumentVersion refs, context bundle, policy digest and inventory
/// generation.
#[test]
fn committed_intent_preserves_a_fingerprint_distinct_from_the_decision_digest() {
    const FRESHNESS: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-fingerprint");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "inputFingerprint":FRESHNESS,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert_eq!(flow["decisionDigest"], DECISION);

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-fingerprint-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(
        delegation["status"], "spawning",
        "a fingerprint that differs from the decision digest is the normal case, \
         not an attestation failure: {delegation}"
    );
    let record = harness
        .persistence
        .load_record(
            "delegation/v1",
            delegation["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("the delegation was committed");

    assert_eq!(
        record["inputFingerprint"].as_str(),
        Some(FRESHNESS),
        "the committed intent must carry the flow's own fingerprint, not a copy \
         of the decision digest: {record}"
    );
    assert_ne!(
        record["inputFingerprint"].as_str(),
        Some(DECISION),
        "conflating the two proofs makes input freshness unobservable"
    );
}

/// F-06: "当前 delegation ID 不能替代 SeriesRunId，因为一次 Series 重试可能产生
/// 新的物理运行和新的 delegation，但仍属于同一逻辑 Series."
///
/// So a delegation record has to carry three separable facts: which logical
/// Series this is, which *attempt* of it this is, and which attempt it replaces.
/// Without `seriesRunId` the two attempts are only distinguishable by their
/// delegation ids, which says nothing about them being the same Series; without
/// `retryOf` the history is a flat list of unrelated delegations, so "this Series
/// was retried twice" is not answerable after a restart.
#[test]
fn a_series_retry_gets_a_new_run_identity_that_still_names_what_it_replaces() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-retry");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut first_create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    first_create.work_item_id = Some(WORK_ITEM.to_owned());
    first_create.idempotency_key = Some("retry-attempt-1".to_owned());
    let first = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        first_create,
    ));
    let first_record = harness
        .persistence
        .load_record(
            "delegation/v1",
            first["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("first attempt committed");

    let first_run = first_record["seriesRunId"]
        .as_str()
        .expect("a delegation must name the Series attempt it is running");
    assert!(
        first_record["retryOf"].is_null(),
        "a first attempt replaces nothing: {first_record}"
    );
    // F-06 is explicit that "当前 delegation ID 不能替代 SeriesRunId". Aliasing the
    // two would pass a two-attempt inequality check for the wrong reason, while
    // still leaving the attempt indistinguishable from the delegation edge.
    assert_ne!(
        first_record["seriesRunId"].as_str(),
        first_record["delegationId"].as_str(),
        "the attempt identity must be its own key, not an alias of the delegation          edge: {first_record}"
    );

    // Retry lineage is authority, so it arrives on the flow decision — a root
    // cannot name its own predecessor. Re-running `flow.next` with the retry edge
    // is what a real retry looks like.
    // The retry is a *new* decision, so it carries its own digest. This is what
    // makes the stable-Series claim testable: an implementation that derived
    // `seriesId` from anything per-decision or per-attempt would now disagree
    // across the two records, while a correct one still resolves to one Series.
    const RETRY_DECISION: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":RETRY_DECISION,
        "stateRevision":8,
        "phase":"requirement-analyzed",
        "retryOfSeriesRunId":first_run,
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut retry_next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    retry_next.work_item_id = Some(WORK_ITEM.to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, retry_next));

    let mut retry_create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":RETRY_DECISION}),
        1_000,
    );
    retry_create.work_item_id = Some(WORK_ITEM.to_owned());
    retry_create.idempotency_key = Some("retry-attempt-2".to_owned());
    let retry = result(&harness.call(
        &mut root_connection,
        RpcMethod::DelegationCreate,
        retry_create,
    ));
    let retry_record = harness
        .persistence
        .load_record(
            "delegation/v1",
            retry["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("retry committed");

    assert_ne!(
        retry_record["seriesRunId"].as_str(),
        Some(first_run),
        "a retry is a new physical run, so it cannot reuse the run identity"
    );
    assert_eq!(
        retry_record["retryOf"].as_str(),
        Some(first_run),
        "and it must still name the attempt it replaces, or the retry history is \
         a flat list of unrelated delegations: {retry_record}"
    );

    // D-03 item 3 separates a *stable* SeriesId from the per-attempt run. Without
    // it the two attempts carry a `retryOf` edge but no shared owner, so "all
    // attempts of this Series" is not a query — which is precisely the
    // independent-queryability the completion criterion asks for.
    let series = first_record["seriesId"]
        .as_str()
        .expect("a delegation must name the logical Series it is an attempt of");
    assert_eq!(
        retry_record["seriesId"].as_str(),
        Some(series),
        "a retry is a new run of the *same* logical Series: {retry_record}"
    );
    assert_ne!(
        first_record["seriesId"].as_str(),
        first_record["seriesRunId"].as_str(),
        "the stable Series and the attempt must be separate keys"
    );

    // The separation above is only useful if the projection preserves it. Line 767
    // requires the execution tree not be polluted by retries, and a projection keyed
    // by the logical Series would collapse both attempts onto one row — the defect
    // the `series_plan_projection` table still has, whose primary key is
    // `(workspace_id, series_id)`.
    let runs = harness
        .persistence
        .list_records("series_run/v1")
        .expect("series_run namespace is listable");
    assert_eq!(
        runs.len(),
        2,
        "two attempts at one Series must occupy two rows, not overwrite one: {runs:?}"
    );
    let by_series: Vec<_> = runs
        .iter()
        .filter(|(_, value)| value["seriesId"].as_str() == Some(series))
        .collect();
    assert_eq!(
        by_series.len(),
        2,
        "and both must be reachable from the stable SeriesId, which is what makes          \"every attempt of this Series\" one query instead of a delegation scan"
    );
    let retry_projection = runs
        .iter()
        .find(|(_, value)| value["retryOf"].as_str() == Some(first_run))
        .expect("the replacing attempt is identifiable by the run it replaces");
    assert_ne!(
        retry_projection.1["seriesRunId"].as_str(),
        Some(first_run),
        "the retry row is the new attempt, not a rewrite of the old one"
    );
}

/// The pre-F-10 compatibility read, pinned as deliberate rather than incidental.
///
/// A flow decision minted before F-10 emits no `inputFingerprint`, so
/// `service_host.rs` falls back to the decision digest to fill the slot. That
/// fallback is load-bearing: the committed intent is rejected when the
/// fingerprint is empty, so without it every legacy decision would fail
/// delegation with an attestation error that names nothing about compatibility.
///
/// Five other tests in this file already omit `inputFingerprint` and therefore
/// exercise this path, but none assert it, so deleting the fallback would surface
/// as an unrelated attestation failure rather than a compatibility regression.
/// This test states the contract: a legacy decision still delegates, and the
/// fingerprint it lands on is the digest — which is exactly why such a decision
/// carries no freshness signal and must not be treated as if it does.
#[test]
fn a_pre_f10_decision_without_a_fingerprint_still_delegates_via_the_digest() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-legacy");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let flow = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));
    assert!(
        flow.get("inputFingerprint").is_none(),
        "this fixture models a pre-F-10 decision, which emits no fingerprint: {flow}"
    );

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-legacy-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    assert_eq!(
        delegation["status"], "spawning",
        "a legacy decision must remain delegable, or the compatibility read is \
         not actually compatible: {delegation}"
    );
    let record = harness
        .persistence
        .load_record(
            "delegation/v1",
            delegation["delegationId"].as_str().expect("delegation id"),
        )
        .expect("record loads")
        .expect("the delegation was committed");

    assert_eq!(
        record["inputFingerprint"].as_str(),
        Some(DECISION),
        "with no fingerprint of its own, a legacy decision lands on the digest: \
         {record}"
    );
}

/// D-03 item 5 and §4.2: the execution flow tree must be queryable apart from the
/// delegation tree.
///
/// The delegation record cannot answer this on its own — it is keyed by
/// `delegationId`, so "every attempt of this Series" means listing all delegations
/// and filtering their payloads. This projection is keyed by `seriesRunId` and
/// carries `seriesId`, which is what makes the query direct rather than a scan.
#[test]
fn creating_a_series_delegation_publishes_a_queryable_series_run_projection() {
    let harness = Harness::new(RuntimeConfig::default());
    let _host = harness.connection_as(ClientKind::HostAdapter, Some(ADAPTER));
    let mut root_connection = harness.connection(ClientKind::Hook);
    let workspace = register_workspace(&harness, &mut root_connection, "series-run-proj");
    let root = open_root_session(
        &harness,
        &mut root_connection,
        &workspace,
        "root-agent",
        "root-external",
        Some(WORK_ITEM),
    );
    harness.business.set_flow_next_result(json!({
        "schemaVersion":"flow-decision/v1",
        "decisionDigest":DECISION,
        "stateRevision":7,
        "phase":"requirement-analyzed",
        "flowRunId":"0192f0c0-1111-7000-8000-000000000001",
        "nextAction":{
            "kind":"delegate-series",
            "seriesKind":"design-review",
            "requiredArtifacts":["DR"]
        }
    }));
    let mut next = session_params(&workspace, &root, "root-agent", json!({}), 1_000);
    next.work_item_id = Some(WORK_ITEM.to_owned());
    let _ = result(&harness.call(&mut root_connection, RpcMethod::FlowNext, next));

    let mut create = session_params(
        &workspace,
        &root,
        "root-agent",
        json!({"flowDecisionDigest":DECISION}),
        1_000,
    );
    create.work_item_id = Some(WORK_ITEM.to_owned());
    create.idempotency_key = Some("series-run-proj-create".to_owned());
    let delegation =
        result(&harness.call(&mut root_connection, RpcMethod::DelegationCreate, create));

    let records = harness
        .persistence
        .list_records("series_run/v1")
        .expect("series_run namespace is listable");
    assert_eq!(
        records.len(),
        1,
        "one Series delegation is one attempt, so one projection"
    );
    let (key, projection) = &records[0];
    // Read from the durable record, not the response: `project_delegation` exposes
    // ten fields and the attempt identity is not among them, so a caller currently
    // cannot correlate its delegation to the Series Run. That is a separate gap from
    // D-03 item 5, which asks only that the runs themselves be queryable.
    let stored = harness
        .persistence
        .load_record(
            "delegation/v1",
            delegation["delegationId"].as_str().expect("id"),
        )
        .expect("delegation record is readable")
        .expect("the create wrote a delegation record");
    let series_run_id = stored["seriesRunId"]
        .as_str()
        .expect("the delegation record names its attempt")
        .to_owned();
    let series_run_id = series_run_id.as_str();
    assert!(
        key.contains(series_run_id),
        "the projection is keyed per attempt, not per logical Series: keying by \
         seriesId would make a retry overwrite the attempt it replaces, which is \
         the pollution line 767 forbids"
    );
    assert_eq!(projection["seriesRunId"], json!(series_run_id));
    assert_eq!(
        projection["seriesId"], stored["seriesId"],
        "the stable Series is carried so all attempts of it are one query"
    );
    assert_eq!(
        projection["flowRunId"],
        json!("0192f0c0-1111-7000-8000-000000000001"),
        "§4.2 needs FR -> Series Run, so the attempt names its Flow Run — taken \
         from the committed decision, never from the root payload"
    );
    assert_eq!(
        projection["lifecycleState"],
        json!("spawn_requested"),
        "§7 rule 13 forbids a Series showing as running before the child claims it, \
         so a spawning delegation must not project as running"
    );
}

/// The projection carries a §11.2 lifecycle state as a bare string, so it can drift
/// from the frozen `SeriesLifecycleState` vocabulary without anything noticing. This
/// pins every value the mapping can emit against the typed enum's own wire form.
#[test]
fn the_projected_lifecycle_states_are_all_frozen_contract_spellings() {
    use ae_sdd_contracts::SeriesLifecycleState;

    for (state, expected) in [
        (SeriesLifecycleState::SpawnRequested, "spawn_requested"),
        (SeriesLifecycleState::Running, "running"),
        (SeriesLifecycleState::ResultStaged, "result_staged"),
        (SeriesLifecycleState::Validated, "validated"),
        (SeriesLifecycleState::Completed, "completed"),
        (SeriesLifecycleState::Cancelled, "cancelled"),
    ] {
        assert_eq!(
            serde_json::to_value(state).expect("serialize"),
            json!(expected),
            "the projection emits {expected}, so the frozen contract must spell it \
             that way or the two representations have diverged"
        );
    }
}
