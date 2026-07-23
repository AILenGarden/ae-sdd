#[allow(dead_code, unused_imports)]
#[path = "../src/legacy/mod.rs"]
mod legacy;

use std::collections::BTreeSet;

use ae_sdd_protocol::RpcMethod;
use legacy::{
    ImplementationStatus, LegacyRouteError, LegacyRpcAdapter, LegacyTarget, NativeJobKind,
    embedded_routes, resolve_command_id, resolve_legacy_argv,
};

#[test]
fn embedded_fixture_exposes_all_113_fail_closed_routes_with_evidence() {
    let routes = embedded_routes().expect("valid embedded routes");
    assert_eq!(routes.len(), legacy::LEGACY_COMMAND_COUNT);

    let mut ids = BTreeSet::new();
    for route in routes {
        assert!(
            ids.insert(route.command_id.as_str()),
            "duplicate command id"
        );
        assert!(route.contract.fail_closed, "{}", route.command_id);
        assert!(route.contract.deadline_ms > 0, "{}", route.command_id);
        assert!(!route.contract.fixture.is_empty(), "{}", route.command_id);
        assert!(!route.contract.evidence.is_empty(), "{}", route.command_id);
        match route.contract.status {
            ImplementationStatus::Pending => assert!(route.is_provisional()),
            ImplementationStatus::Implemented | ImplementationStatus::BreakingFixVerified => {
                assert!(!route.is_provisional());
            }
        }
    }
}

#[test]
fn admin_diagnostics_use_daemon_job_submit_and_only_build_kernels_stay_native() {
    let routes = embedded_routes().expect("valid embedded routes");
    let mut admin_rpc = 0;
    let mut native = 0;
    for route in routes {
        match &route.target {
            LegacyTarget::Rpc {
                method: RpcMethod::JobSubmit,
                adapter:
                    LegacyRpcAdapter::JobSubmission {
                        job: NativeJobKind::Admin,
                        entrypoint,
                    },
            } => {
                admin_rpc += 1;
                assert!(!entrypoint.is_empty());
            }
            LegacyTarget::NativeBuildJob { job, entrypoint } => {
                native += 1;
                assert_ne!(*job, NativeJobKind::Admin);
                assert_eq!(*job, NativeJobKind::Offline);
                assert!(!entrypoint.is_empty());
            }
            _ => {}
        }
    }
    assert_eq!(admin_rpc, 40);
    assert_eq!(native, 13);
}

#[test]
fn typed_operations_route_through_operation_execute_without_losing_selector() {
    let route = resolve_command_id("lease acquire").expect("known command");
    assert!(route.identity_workspace);
    assert!(route.identity_work_item);
    assert!(route.identity_session);
    assert_eq!(
        route.target,
        LegacyTarget::Rpc {
            method: RpcMethod::OperationExecute,
            adapter: LegacyRpcAdapter::TypedOperation {
                operation: "lease.acquire".to_owned(),
            },
        }
    );
}

#[test]
fn removed_old_spellings_carry_a_stable_remediation_and_never_dispatch() {
    let cases = [
        "context-pressure",
        "enter",
        "gate-intercept",
        "prompt-inject",
        "ra-gate",
        "runtime compact",
        "scripts-dir",
        "state confirm",
        "state lock",
        "state unlock",
        "stop-check",
        "subprocess collect",
        "subprocess list",
        "subprocess spawn",
        "subprocess status",
    ];
    for command in cases {
        let route = resolve_command_id(command).expect("known removed spelling");
        assert!(matches!(
            route.target,
            LegacyTarget::Rejected {
                ref stable_code,
                ref remediation,
            } if stable_code == "LEGACY_COMMAND_REMOVED" && !remediation.is_empty()
        ));
        let args = command.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert!(matches!(
            resolve_legacy_argv(&args),
            Err(LegacyRouteError::RemovedDeprecated {
                ref command_id,
                ref stable_code,
                ref remediation,
            }) if command_id == command
                && stable_code == "LEGACY_COMMAND_REMOVED"
                && !remediation.is_empty()
        ));
    }
}

#[test]
fn argv_resolution_uses_longest_exact_prefix_and_preserves_trailing_arguments() {
    let args = [
        "state".to_owned(),
        "register-review-consensus".to_owned(),
        "--work-item".to_owned(),
        "WORK-001".to_owned(),
    ];
    let resolved = resolve_legacy_argv(&args).expect("known leaf");
    assert_eq!(resolved.route.command_id, "state register-review-consensus");
    assert_eq!(resolved.consumed_arguments, 2);
    assert_eq!(
        resolved.trailing_arguments,
        ["--work-item".to_owned(), "WORK-001".to_owned()]
    );
}

#[test]
fn missing_unknown_and_removed_deprecated_commands_fail_closed() {
    assert_eq!(
        resolve_legacy_argv(&[]),
        Err(LegacyRouteError::MissingCommand)
    );
    let unknown = ["state".to_owned(), "overwrite-directly".to_owned()];
    assert!(matches!(
        resolve_legacy_argv(&unknown),
        Err(LegacyRouteError::UnknownOrRemovedDeprecated(command))
            if command == "state overwrite-directly"
    ));
    assert!(matches!(
        resolve_command_id("STATE READ"),
        Err(LegacyRouteError::UnknownOrRemovedDeprecated(_))
    ));
}

#[test]
fn fake_launchers_receive_every_route_once_without_a_fallback_branch() {
    #[derive(Default)]
    struct FakeLauncher {
        rpc: usize,
        native: usize,
    }

    let mut launcher = FakeLauncher::default();
    for route in embedded_routes().expect("valid embedded routes") {
        match &route.target {
            LegacyTarget::Rpc { .. } => launcher.rpc += 1,
            LegacyTarget::NativeBuildJob { .. } => launcher.native += 1,
            LegacyTarget::Rejected { .. } => {}
        }
    }
    assert_eq!(launcher.rpc, 85);
    assert_eq!(launcher.native, 13);
    assert_eq!(launcher.rpc + launcher.native, 98);
}
