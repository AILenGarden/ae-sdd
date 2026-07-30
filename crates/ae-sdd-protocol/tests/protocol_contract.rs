use std::{collections::HashSet, str::FromStr};

use ae_sdd_protocol::{
    CapabilityTokenWire, ClientKind, CompactStatus, ContextPayloadKind,
    ENDPOINT_MANIFEST_SCHEMA_V1, ERROR_DATA_SCHEMA_V1, EndpointManifest, FrameError,
    GateOutcomeKind, HandshakeRequest, HandshakeResponse, HookDecision, HostAckOutcome,
    HostActionKind, JobStatus, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResponse,
    JsonRpcVersion, MAX_FRAME_BYTES, METHOD_COUNT, METHOD_REGISTRY, MethodRequirements,
    OperationScope, PROTOCOL_RANGE_V1, PROTOCOL_VERSION_V1, RequirementSource, RpcErrorObject,
    RpcMethod, SecretString, StableErrorCode, WorkspaceMode, decode_frame, encode_frame,
};
use serde_json::{Value, json};

const METHOD_NAMES: [&str; METHOD_COUNT] = [
    "runtime.handshake",
    "runtime.status",
    "runtime.drain",
    "workspace.register",
    "workspace.snapshot",
    "workspace.mode_transition",
    "session.open",
    "session.heartbeat",
    "session.close",
    "hook.user_prompt",
    "hook.pre_tool",
    "hook.post_tool",
    "hook.stop",
    "flow.snapshot",
    "flow.next",
    "delegation.create",
    "delegation.status",
    "delegation.accept",
    "delegation.report",
    "delegation.collect",
    "delegation.cancel",
    "host.register",
    "host.capabilities",
    "host.action_next",
    "host.action_ack",
    "host.pressure_report",
    "context.get",
    "context.project",
    "compact.request",
    "compact.status",
    "operation.describe",
    "operation.execute",
    "gate.evaluate",
    "events.subscribe",
    "job.submit",
    "job.status",
    "job.cancel",
];

#[test]
fn v1_method_registry_is_exact_and_ordered() {
    assert_eq!(RpcMethod::ALL.len(), METHOD_COUNT);
    assert_eq!(METHOD_REGISTRY.len(), METHOD_COUNT);

    let actual = RpcMethod::ALL.map(RpcMethod::as_str);
    assert_eq!(actual, METHOD_NAMES);
    for (index, method) in RpcMethod::ALL.into_iter().enumerate() {
        assert_eq!(method.to_string(), method.as_str());
        assert_eq!(method.spec(), &METHOD_REGISTRY[index]);
        assert_eq!(method.spec().method, method);
        assert_eq!(RpcMethod::from_str(method.as_str()), Ok(method));
        assert_eq!(
            serde_json::to_value(method).unwrap(),
            json!(method.as_str())
        );

        let mut components = method.as_str().split('.');
        let domain = components.next().unwrap();
        let verb = components.next().unwrap();
        assert!(components.next().is_none());
        assert!(domain.bytes().all(|byte| byte.is_ascii_lowercase()));
        assert!(
            verb.bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        );
    }
    assert!(RpcMethod::from_str("runtime/handshake").is_err());
    assert!(RpcMethod::from_str("Runtime.Handshake").is_err());
}

#[test]
fn public_constructors_and_secret_accessors_preserve_wire_values() {
    let token = CapabilityTokenWire::new_v1(
        "key-1",
        "boot-1",
        "flow.read",
        "session-1",
        "root",
        None,
        "grant-digest",
        1_000,
        2_000,
        "signed-value",
    );
    assert_eq!(token.signature(), "signed-value");

    let secret = SecretString::new("endpoint-secret");
    assert_eq!(secret.expose_secret(), "endpoint-secret");

    let success = JsonRpcResponse::new("request-1", json!({ "accepted": true }));
    assert_eq!(success.jsonrpc, JsonRpcVersion);
    assert_eq!(success.id, "request-1");
    assert_eq!(success.result, json!({ "accepted": true }));

    let error = RpcErrorObject::new(
        StableErrorCode::DaemonUnavailable,
        "daemon unavailable",
        Some("retry after daemon startup".to_owned()),
        Some("request-2".to_owned()),
    );
    assert_eq!(error.code, -32_000);
    assert_eq!(error.message, "daemon unavailable");
    assert_eq!(error.data.schema_version, ERROR_DATA_SCHEMA_V1);
    assert_eq!(error.data.stable_code, StableErrorCode::DaemonUnavailable);
    assert!(error.data.retryable);
    assert_eq!(
        error.data.remediation.as_deref(),
        Some("retry after daemon startup")
    );
    assert_eq!(error.data.request_id.as_deref(), Some("request-2"));
    assert_eq!(error.data.details_digest, None);

    let failure = JsonRpcErrorResponse::new("request-2", error.clone());
    assert_eq!(failure.jsonrpc, JsonRpcVersion);
    assert_eq!(failure.id, "request-2");
    assert_eq!(failure.error, error);
}

#[test]
fn method_precondition_flags_cover_bootstrap_hooks_and_typed_operations() {
    assert_requirements(
        RpcMethod::RuntimeHandshake,
        OperationScope::Runtime,
        MethodRequirements {
            requires_workspace: false,
            requires_work_item: false,
            writes: false,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency: false,
            requires_confirmation: false,
            source: RequirementSource::Method,
        },
    );
    assert_requirements(
        RpcMethod::WorkspaceRegister,
        OperationScope::Workspace,
        MethodRequirements {
            requires_workspace: false,
            requires_work_item: false,
            writes: true,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency: true,
            requires_confirmation: false,
            source: RequirementSource::Method,
        },
    );
    // A Hook is Session-scoped, so it must not carry a Work Item precondition:
    // `constraints/api.md` forbids locking session bootstrap behind Work Item
    // requirements, and doing so made the first host turn unreachable.
    for hook in [
        RpcMethod::HookUserPrompt,
        RpcMethod::HookPreTool,
        RpcMethod::HookPostTool,
        RpcMethod::HookStop,
    ] {
        assert_requirements(
            hook,
            OperationScope::Session,
            MethodRequirements {
                requires_workspace: true,
                requires_work_item: false,
                writes: true,
                requires_lease: false,
                requires_revision: false,
                requires_idempotency: true,
                requires_confirmation: false,
                source: RequirementSource::Method,
            },
        );
    }
    assert_requirements(
        RpcMethod::RuntimeDrain,
        OperationScope::Runtime,
        MethodRequirements {
            requires_workspace: false,
            requires_work_item: false,
            writes: true,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency: true,
            requires_confirmation: true,
            source: RequirementSource::Method,
        },
    );
    assert_requirements(
        RpcMethod::OperationExecute,
        OperationScope::WorkItem,
        MethodRequirements {
            requires_workspace: true,
            requires_work_item: true,
            writes: false,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency: false,
            requires_confirmation: false,
            source: RequirementSource::TypedOperation,
        },
    );
    assert_requirements(
        RpcMethod::GateEvaluate,
        OperationScope::WorkItem,
        MethodRequirements {
            requires_workspace: true,
            requires_work_item: true,
            writes: true,
            requires_lease: false,
            requires_revision: false,
            requires_idempotency: true,
            requires_confirmation: false,
            source: RequirementSource::Method,
        },
    );

    assert_eq!(
        METHOD_REGISTRY
            .iter()
            .filter(|spec| spec.requirements.source == RequirementSource::TypedOperation)
            .map(|spec| spec.method)
            .collect::<Vec<_>>(),
        vec![RpcMethod::OperationExecute]
    );

    let described = serde_json::to_value(RpcMethod::OperationExecute.spec()).unwrap();
    assert_eq!(described["method"], json!("operation.execute"));
    assert_eq!(described["scope"], json!("work_item"));
    assert_eq!(described["requiresWorkspace"], json!(true));
    assert_eq!(described["requiresWorkItem"], json!(true));
    assert_eq!(described["requiresLease"], json!(false));
    assert_eq!(described["requiresRevision"], json!(false));
    assert_eq!(described["requiresIdempotency"], json!(false));
    assert_eq!(described["requiresConfirmation"], json!(false));
    assert_eq!(described["source"], json!("typed_operation"));
}

fn assert_requirements(method: RpcMethod, scope: OperationScope, requirements: MethodRequirements) {
    assert_eq!(method.spec().scope, scope);
    assert_eq!(method.spec().requirements, requirements);
}

#[test]
fn requests_reject_unknown_fields_and_unknown_methods() {
    let valid = json!({
        "jsonrpc": "2.0",
        "id": "request-1",
        "method": "runtime.handshake",
        "params": handshake_request_json()
    });
    serde_json::from_value::<JsonRpcRequest<HandshakeRequest>>(valid.clone()).unwrap();

    let mut unknown_envelope = valid.clone();
    unknown_envelope["unexpected"] = json!(true);
    assert!(serde_json::from_value::<JsonRpcRequest<HandshakeRequest>>(unknown_envelope).is_err());

    let mut unknown_params = valid.clone();
    unknown_params["params"]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<JsonRpcRequest<HandshakeRequest>>(unknown_params).is_err());

    let mut unknown_method = valid;
    unknown_method["method"] = json!("runtime.future_method");
    assert!(serde_json::from_value::<JsonRpcRequest<HandshakeRequest>>(unknown_method).is_err());
}

#[test]
fn json_rpc_version_is_exact() {
    assert_eq!(serde_json::to_value(JsonRpcVersion).unwrap(), json!("2.0"));
    assert!(serde_json::from_value::<JsonRpcVersion>(json!("2.0")).is_ok());
    assert!(serde_json::from_value::<JsonRpcVersion>(json!("1.0")).is_err());
    assert!(serde_json::from_value::<JsonRpcVersion>(json!(2.0)).is_err());
}

#[test]
fn handshake_response_accepts_additive_minor_fields() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": "request-1",
        "serverTiming": { "decodeMicros": 3 },
        "result": {
            "protocolVersion": PROTOCOL_VERSION_V1,
            "bootId": "boot-1",
            "eventStoreId": "event-store-1",
            "daemonBuild": "ae-sddd/0.1.0",
            "capabilities": ["context.delta.v1"],
            "policyDigest": "a".repeat(64),
            "operationSchemaDigest": "b".repeat(64),
            "limits": {
                "maxFrameBytes": MAX_FRAME_BYTES,
                "maxAgentDepth": 2,
                "maxStringBytes": 65536,
                "maxCollectionItems": 4096,
                "maxDeadlineMs": 300000,
                "hookDeadlineMs": 250,
                "maxChildResultBytes": 65536,
                "maxChildSummaryBytes": 8192,
                "maxContextProjectionBytes": 65536,
                "futureOptionalLimit": 7
            },
            "capabilityKeyId": "boot-1:key-1",
            "capabilityPublicKey": "base64-public-key",
            "futureOptionalCapability": true
        }
    });

    let decoded = serde_json::from_value::<JsonRpcResponse<HandshakeResponse>>(response).unwrap();
    assert_eq!(decoded.result.boot_id, "boot-1");
    assert_eq!(
        decoded.result.limits.max_frame_bytes,
        MAX_FRAME_BYTES as u64
    );
}

#[test]
fn response_rejects_reserved_result_error_ambiguity() {
    let ambiguous_success = json!({
        "jsonrpc": "2.0",
        "id": "request-1",
        "result": { "value": 1 },
        "error": { "code": -32000 }
    });
    assert!(
        serde_json::from_value::<JsonRpcResponse<serde_json::Value>>(ambiguous_success).is_err()
    );

    let ambiguous_error = json!({
        "jsonrpc": "2.0",
        "id": "request-1",
        "result": { "value": 1 },
        "error": {
            "code": -32000,
            "message": "daemon unavailable",
            "data": {
                "schemaVersion": "ae-sdd-error/v1",
                "stableCode": "DAEMON_UNAVAILABLE",
                "retryable": true
            }
        }
    });
    assert!(serde_json::from_value::<JsonRpcErrorResponse>(ambiguous_error).is_err());
}

#[test]
fn endpoint_manifest_accepts_additive_fields_but_redacts_token_in_debug() {
    let manifest_json = json!({
        "schemaVersion": ENDPOINT_MANIFEST_SCHEMA_V1,
        "pid": 42,
        "bootId": "boot-1",
        "eventStoreId": "event-store-1",
        "endpoint": "local-endpoint",
        "endpointToken": "manifest-secret",
        "protocolRange": PROTOCOL_RANGE_V1,
        "daemonVersion": "0.1.0",
        "policyDigest": "a".repeat(64),
        "capabilityKeyId": "boot-1:key-1",
        "capabilityPublicKey": "base64-public-key",
        "startedAt": "2026-07-23T00:00:00Z",
        "futureOptionalField": "accepted"
    });

    let manifest = serde_json::from_value::<EndpointManifest>(manifest_json).unwrap();
    let debug = format!("{manifest:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("manifest-secret"));
}

#[test]
fn secret_debug_is_redacted_through_handshake_request() {
    let request = JsonRpcRequest::new(
        "request-1",
        RpcMethod::RuntimeHandshake,
        HandshakeRequest {
            protocol_range: PROTOCOL_RANGE_V1.to_owned(),
            client_build: "ae-sdd/0.1.0".to_owned(),
            client_kind: ClientKind::Hook,
            endpoint_token: SecretString::new("wire-secret"),
            expected_boot_id: "boot-1".to_owned(),
            expected_policy_digest: "a".repeat(64),
        },
    );

    let debug = format!("{request:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("wire-secret"));

    let wire = serde_json::to_value(&request).unwrap();
    assert_eq!(wire["params"]["endpointToken"], json!("wire-secret"));
}

#[test]
fn frame_round_trip_uses_four_byte_big_endian_prefix() {
    let payload = br#"{"jsonrpc":"2.0"}"#;
    let frame = encode_frame(payload).unwrap();
    assert_eq!(&frame[..4], &(payload.len() as u32).to_be_bytes());
    assert_eq!(decode_frame(&frame).unwrap(), payload);
}

#[test]
fn frame_rejects_empty_truncated_mismatched_and_oversize_payloads() {
    assert_eq!(encode_frame(b""), Err(FrameError::EmptyPayload));
    assert_eq!(
        decode_frame(&[0, 0, 0]),
        Err(FrameError::HeaderTooShort { actual: 3 })
    );
    assert_eq!(decode_frame(&[0, 0, 0, 0]), Err(FrameError::EmptyPayload));
    assert_eq!(
        decode_frame(&[0, 0, 0, 2, b'a']),
        Err(FrameError::LengthMismatch {
            declared: 2,
            actual: 1,
        })
    );

    let oversized = vec![b'x'; MAX_FRAME_BYTES + 1];
    assert_eq!(
        encode_frame(&oversized),
        Err(FrameError::PayloadTooLarge {
            actual: MAX_FRAME_BYTES + 1,
            maximum: MAX_FRAME_BYTES,
        })
    );
    let prefix_only = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes();
    assert_eq!(
        decode_frame(&prefix_only),
        Err(FrameError::PayloadTooLarge {
            actual: MAX_FRAME_BYTES + 1,
            maximum: MAX_FRAME_BYTES,
        })
    );
}

#[test]
fn stable_errors_have_exact_wire_names_unique_numbers_and_no_combined_codes() {
    let names = StableErrorCode::ALL
        .iter()
        .map(|code| code.as_str())
        .collect::<HashSet<_>>();
    let numbers = StableErrorCode::ALL
        .iter()
        .map(|code| code.json_rpc_code())
        .collect::<HashSet<_>>();
    assert_eq!(names.len(), StableErrorCode::ALL.len());
    assert_eq!(numbers.len(), StableErrorCode::ALL.len());
    assert!(
        StableErrorCode::ALL
            .iter()
            .all(|code| (-32_099..=-32_000).contains(&code.json_rpc_code()))
    );
    assert!(
        StableErrorCode::ALL
            .iter()
            .all(|code| !code.as_str().contains('/'))
    );
    assert!(StableErrorCode::ALL.iter().all(|code| {
        code.as_str()
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    }));

    assert!(names.contains("IDEMPOTENCY_KEY_REUSED"));
    assert!(names.contains("ENDPOINT_AUTH_FAILED"));
    assert!(names.contains("ENDPOINT_STALE"));
    assert!(names.contains("HOST_ACK_REJECTED"));
    assert!(names.contains("CONTEXT_BUDGET_EXCEEDED"));
    assert!(names.contains("COMPACT_ACK_INVALID"));
    assert!(names.contains("EXECUTION_CAPSULE_STALE"));
    assert!(names.contains("EXECUTION_SLICE_INVALID"));
    assert!(names.contains("EXECUTION_PROGRESS_REQUIRED"));
    assert!(names.contains("EXECUTION_RESOURCE_BUSY"));
    assert!(names.contains("EXECUTION_BUDGET_EXCEEDED"));
    assert_eq!(
        serde_json::to_value(StableErrorCode::IdempotencyKeyReused).unwrap(),
        json!("IDEMPOTENCY_KEY_REUSED")
    );

    let retryable = HashSet::from([
        StableErrorCode::DaemonUnavailable,
        StableErrorCode::EndpointStale,
        StableErrorCode::DaemonDraining,
        StableErrorCode::RevisionConflict,
        StableErrorCode::LeaseRequired,
        StableErrorCode::LeaseConflict,
        StableErrorCode::LeaseExpired,
        StableErrorCode::StaleFencingToken,
        StableErrorCode::ConfirmationRequired,
        StableErrorCode::StaleGateResult,
        StableErrorCode::GateError,
        StableErrorCode::GateTimeout,
        StableErrorCode::GateBlocked,
        StableErrorCode::SessionExpired,
        StableErrorCode::HostAckTimeout,
        StableErrorCode::HostAckRejected,
        StableErrorCode::ChildResultInvalid,
        StableErrorCode::ChildResultTooLarge,
        StableErrorCode::ContextRevisionStale,
        StableErrorCode::ContextBudgetExceeded,
        StableErrorCode::CompactAckTimeout,
        StableErrorCode::EventCursorGap,
        StableErrorCode::SubscriberBackpressure,
        StableErrorCode::ScopeAmbiguous,
        StableErrorCode::OperationSchemaInvalid,
        StableErrorCode::ExecutionCapsuleStale,
        StableErrorCode::ExecutionProgressRequired,
        StableErrorCode::ExecutionResourceBusy,
        StableErrorCode::ExecutionBudgetExceeded,
    ]);
    for code in StableErrorCode::ALL {
        assert_eq!(
            code.retryable_by_default(),
            retryable.contains(code),
            "unexpected retry default for {}",
            code.as_str()
        );
    }
}

#[test]
fn execution_supervisor_errors_have_frozen_wire_names_numbers_and_retry_defaults() {
    let cases = [
        (
            StableErrorCode::ExecutionCapsuleStale,
            "EXECUTION_CAPSULE_STALE",
            -32_093,
            true,
        ),
        (
            StableErrorCode::ExecutionSliceInvalid,
            "EXECUTION_SLICE_INVALID",
            -32_094,
            false,
        ),
        (
            StableErrorCode::ExecutionProgressRequired,
            "EXECUTION_PROGRESS_REQUIRED",
            -32_095,
            true,
        ),
        (
            StableErrorCode::ExecutionResourceBusy,
            "EXECUTION_RESOURCE_BUSY",
            -32_096,
            true,
        ),
        (
            StableErrorCode::ExecutionBudgetExceeded,
            "EXECUTION_BUDGET_EXCEEDED",
            -32_097,
            true,
        ),
    ];
    for (code, name, number, retryable) in cases {
        assert_eq!(code.as_str(), name);
        assert_eq!(code.json_rpc_code(), number);
        assert_eq!(code.retryable_by_default(), retryable);
        assert_eq!(serde_json::to_value(code).unwrap(), json!(name));
    }
}

#[test]
fn wire_enums_use_the_frozen_v1_spellings() {
    let cases: Vec<(Value, Value)> = vec![
        (
            serde_json::to_value(ClientKind::HostAdapter).unwrap(),
            json!("host_adapter"),
        ),
        (
            serde_json::to_value(OperationScope::WorkItem).unwrap(),
            json!("work_item"),
        ),
        (
            serde_json::to_value(WorkspaceMode::RustSoleWriter).unwrap(),
            json!("rust-sole-writer"),
        ),
        (
            serde_json::to_value(GateOutcomeKind::Cancelled).unwrap(),
            json!("CANCELLED"),
        ),
        (
            serde_json::to_value(HookDecision::Block).unwrap(),
            json!("block"),
        ),
        (
            serde_json::to_value(JobStatus::Stale).unwrap(),
            json!("stale"),
        ),
        (
            serde_json::to_value(ContextPayloadKind::NoChange).unwrap(),
            json!("no_change"),
        ),
        (
            serde_json::to_value(HostActionKind::Attest).unwrap(),
            json!("attest"),
        ),
        (
            serde_json::to_value(HostAckOutcome::Rejected).unwrap(),
            json!("rejected"),
        ),
        (
            serde_json::to_value(CompactStatus::ContextRestored).unwrap(),
            json!("context-restored"),
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
}

fn handshake_request_json() -> Value {
    json!({
        "protocolRange": PROTOCOL_RANGE_V1,
        "clientBuild": "ae-sdd/0.1.0",
        "clientKind": "hook",
        "endpointToken": "wire-secret",
        "expectedBootId": "boot-1",
        "expectedPolicyDigest": "a".repeat(64)
    })
}
