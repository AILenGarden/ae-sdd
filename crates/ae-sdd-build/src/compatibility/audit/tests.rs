use super::*;

fn entry(id: &str) -> SurfaceEntry {
    SurfaceEntry {
        id: id.to_owned(),
        source: "source/path".to_owned(),
        owner: "ae-sdd-build".to_owned(),
        disposition: Disposition::Preserve,
    }
}

fn manifest() -> CompatibilityManifest {
    CompatibilityManifest {
        schema_version: "1".to_owned(),
        routing_manifest: "routing.json".to_owned(),
        sources: InventorySources {
            cli_parser: "tools/bin/ae-sdd".to_owned(),
            operation_registry: "tools/lib/operations.py".to_owned(),
            gate_registry: "tools/lib/gates.py".to_owned(),
            scanner_registry: "scripts".to_owned(),
        },
        commands: vec![entry("version")],
        operations: vec![entry("workitem.get")],
        gates: vec![entry("G-00")],
        scanners: vec![entry("flow-violation-scan")],
    }
}

#[test]
fn inventory_audit_accepts_exact_non_empty_surface() {
    let summary = manifest()
        .audit(ExpectedCounts {
            commands: 1,
            operations: 1,
            gates: 1,
            scanners: 1,
        })
        .expect("valid inventory");
    assert_eq!(summary.command_count, 1);
    assert_eq!(summary.route_count, 0);
}

#[test]
fn inventory_rejects_duplicate_ids() {
    let mut value = manifest();
    value.commands.push(entry("version"));
    assert!(matches!(
        value.audit(ExpectedCounts {
            commands: 2,
            operations: 1,
            gates: 1,
            scanners: 1,
        }),
        Err(ManifestError::DuplicateId { .. })
    ));
}

#[test]
fn route_schema_has_no_fallback_or_stub_variant() {
    let fallback = r#"{
      "id":"version","route":{"kind":"native-build-job","job":"admin","entrypoint":"version","fallback":"legacy"},
      "identity":{"workspace":false,"workItem":false,"session":false},"deadlineMs":1000,
      "failClosed":true,"fixture":"a","evidence":"b","status":"implemented"
    }"#;
    assert!(serde_json::from_str::<CommandRoute>(fallback).is_err());

    let stub = fallback
        .replace("\"fallback\":\"legacy\",", "")
        .replace("\"implemented\"", "\"stub\"");
    assert!(serde_json::from_str::<CommandRoute>(&stub).is_err());
}

#[test]
fn exact_native_entrypoint_is_enforced() {
    let route = CommandRoute {
        id: "assets check".to_owned(),
        route: RouteTarget::NativeBuildJob {
            job: NativeJobKind::Admin,
            entrypoint: "assets.wrong".to_owned(),
        },
        identity: RouteIdentity {
            workspace: true,
            work_item: false,
            session: true,
        },
        deadline_ms: 1_000,
        fail_closed: true,
        fixture: "a".to_owned(),
        evidence: "b".to_owned(),
        status: ImplementationStatus::Implemented,
    };
    assert!(matches!(
        validate_route_target(&route),
        Err(ManifestError::RouteTarget { .. })
    ));
}
