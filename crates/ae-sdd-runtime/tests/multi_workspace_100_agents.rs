mod support;

use ae_sdd_domain::{DesignRoute, InputFingerprint, ProcessPhase, StateRevision, WorkScale};
use ae_sdd_flow::{FlowEnvironment, FlowInput, FlowSnapshot, RouteSelection};
use ae_sdd_protocol::{ClientKind, ConfirmationRef, RpcMethod, WorkspaceMode};
use ae_sdd_runtime::RuntimeConfig;
use serde_json::json;
use std::sync::atomic::Ordering;

use support::{
    Harness, open_root_session, params, parity_transition_payload, register_workspace, result,
    session_params, stable_error,
};

#[test]
fn admits_one_hundred_agents_across_bounded_workspaces_and_rejects_overflow() {
    let config = RuntimeConfig {
        max_workspaces: 10,
        max_sessions: 100,
        ..RuntimeConfig::default()
    };
    let harness = Harness::new(config);
    let mut admin = harness.connection(ClientKind::Cli);
    let mut workspaces = (0..10)
        .map(|index| register_workspace(&harness, &mut admin, &index.to_string()))
        .collect::<Vec<_>>();
    let mut mode_admin = harness.connection(ClientKind::Admin);
    for (index, workspace) in workspaces.iter_mut().enumerate() {
        let mut drain = params(json!({"stop":false}), 1_000);
        drain.idempotency_key = Some(format!("drain-{index}"));
        drain.confirmation = Some(confirmation(index));
        let _ = result(&harness.call(&mut mode_admin, RpcMethod::RuntimeDrain, drain));
        let mut transition = params(
            parity_transition_payload(WorkspaceMode::RustCanary, 1_000),
            1_000,
        );
        transition.workspace_id = Some(workspace.workspace_id.clone());
        transition.idempotency_key = Some(format!("mode-{index}"));
        transition.confirmation = Some(confirmation(index));
        *workspace = serde_json::from_value(result(&harness.call(
            &mut mode_admin,
            RpcMethod::WorkspaceModeTransition,
            transition,
        )))
        .expect("canary workspace");
    }

    let mut agents = Vec::with_capacity(100);
    for index in 0..100 {
        let mut connection = harness.connection(ClientKind::Hook);
        let workspace_index = index % workspaces.len();
        let workspace = &workspaces[workspace_index];
        let session = open_root_session(
            &harness,
            &mut connection,
            workspace,
            &format!("agent-{index}"),
            &format!("external-{index}"),
            None,
        );
        assert!(session.engaged);
        agents.push((
            connection,
            session,
            workspace_index,
            format!("agent-{index}"),
        ));
    }
    for (index, (connection, session, workspace_index, agent_id)) in agents.iter_mut().enumerate() {
        let workspace = &workspaces[*workspace_index];
        let mut operation = session_params(
            workspace,
            session,
            agent_id,
            json!({"operation":"lease.acquire","payload":{}}),
            1_000,
        );
        operation.work_item_id = Some(format!("WORK-{index}"));
        operation.idempotency_key = Some(format!("mutation-{index}"));
        assert!(
            harness
                .call(connection, RpcMethod::OperationExecute, operation)
                .get("result")
                .is_some()
        );
    }
    assert_eq!(
        harness.business.operation_calls.load(Ordering::Acquire),
        100
    );

    let store_id = harness.runtime.event_store_id().expect("event store id");
    for (index, workspace) in workspaces.iter().enumerate() {
        let input = FlowInput::new(
            FlowSnapshot::new(ProcessPhase::Initialized, StateRevision::new(1), 0),
            FlowEnvironment::new(
                store_id,
                InputFingerprint::digest(format!("workspace-{index}").as_bytes()),
                RouteSelection::new(WorkScale::Small, DesignRoute::CodingPlan),
            ),
        );
        harness
            .runtime
            .flow_supervisor()
            .replay(
                &workspace.workspace_id,
                "ROOT",
                input,
                Vec::<ae_sdd_flow::FlowEvent>::new(),
            )
            .expect("workspace flow checkpoint");
        let checkpoint = harness
            .runtime
            .flow_supervisor()
            .checkpoint(&workspace.workspace_id, "ROOT")
            .expect("checkpoint read")
            .expect("checkpoint exists");
        assert_eq!(checkpoint["workspaceId"], workspace.workspace_id);
    }

    let status = result(&harness.call(
        &mut admin,
        RpcMethod::RuntimeStatus,
        params(json!({}), 1_000),
    ));
    assert_eq!(status["workspaceCount"], 10);
    assert_eq!(status["sessionCount"], 100);
    assert_eq!(agents.len(), 100);

    let mut overflow_connection = harness.connection(ClientKind::Hook);
    let mut overflow = params(
        json!({
            "externalKey": "external-overflow",
            "role": "root",
            "engaged": true,
        }),
        1_000,
    );
    overflow.workspace_id = Some(workspaces[0].workspace_id.clone());
    overflow.agent_id = Some("agent-overflow".to_owned());
    overflow.idempotency_key = Some("session-open-overflow".to_owned());
    let response = harness.call(&mut overflow_connection, RpcMethod::SessionOpen, overflow);
    assert_eq!(stable_error(&response), "SUBSCRIBER_BACKPRESSURE");
}

fn confirmation(index: usize) -> ConfirmationRef {
    ConfirmationRef {
        confirmation_id: format!("confirmation-{index}"),
        approved_by: "test-user".to_owned(),
        approved_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}
