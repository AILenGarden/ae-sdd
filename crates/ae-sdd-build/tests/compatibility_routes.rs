use std::collections::BTreeSet;
use std::path::PathBuf;

use ae_sdd_build::{
    CompatibilityRoutingManifest, ExpectedCounts, ImplementationStatus, ManifestError, RouteTarget,
    audit_compatibility,
};
use ae_sdd_protocol::{CapabilityTokenWire, RequestParams};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn compatibility_audit_rejects_manifest_only_legacy_routes() {
    let root = repository_root();
    let inventory = root.join("tests/fixtures/compatibility/legacy-surface.v1.json");
    let error = audit_compatibility(
        &inventory,
        ExpectedCounts::legacy(),
        &[PathBuf::from("apps/ae-sdd-monitor/**")],
    )
    .expect_err("manifest-only routes must not pass parity");
    let ManifestError::UnimplementedRoutes(routes) = error else {
        panic!("unexpected audit error: {error}")
    };
    assert_eq!(routes.len(), 113);
}

#[test]
fn every_route_declares_one_typed_target_without_claiming_implementation() {
    let path = repository_root().join("tests/fixtures/compatibility/cli-routing.v1.json");
    let routing: CompatibilityRoutingManifest =
        serde_json::from_slice(&std::fs::read(path).expect("routing fixture"))
            .expect("strict routing schema");

    let mut command_ids = BTreeSet::new();
    let mut dispatches = Vec::new();
    for route in routing.commands {
        assert!(command_ids.insert(route.id.clone()), "duplicate command");
        assert!(route.fail_closed, "{} must fail closed", route.id);
        assert_eq!(route.status, ImplementationStatus::Pending);
        let dispatch = match route.route {
            RouteTarget::Rpc { method } => format!("rpc:{method}"),
            RouteTarget::TypedOperation { operation } => format!("operation:{operation}"),
            RouteTarget::NativeBuildJob { job, entrypoint } => {
                format!("native:{}:{entrypoint}", job.as_str())
            }
        };
        assert!(!dispatch.ends_with(':'), "{} has an empty target", route.id);
        dispatches.push((route.id, dispatch));
    }

    assert_eq!(command_ids.len(), 113);
    assert_eq!(dispatches.len(), 113);
}

#[test]
fn build_tool_contains_no_subprocess_fallback_executor() {
    let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source_root];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).expect("source directory") {
            let entry = entry.expect("source entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("Rust source");
            assert!(
                !source.contains("Command::new("),
                "logical subprocess fallback found in {}",
                path.display()
            );
        }
    }
}

#[test]
fn shared_capability_fixture_uses_protocol_canonical_claims_and_negative_cases() {
    let path = repository_root().join("tests/fixtures/protocol/capability-token.v1.json");
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).expect("capability fixture"))
        .expect("fixture JSON");
    let token: CapabilityTokenWire =
        serde_json::from_value(fixture["token"].clone()).expect("strict token wire");
    let canonical = token.canonical_claims_bytes().expect("canonical claims");
    assert_eq!(
        canonical,
        fixture["canonicalClaimsUtf8"]
            .as_str()
            .expect("canonical UTF-8")
            .as_bytes()
    );

    let public_key_bytes: [u8; 32] = hex::decode(
        fixture["publicKey"]["ed25519PublicKeyHex"]
            .as_str()
            .expect("public key"),
    )
    .expect("public key hex")
    .try_into()
    .expect("32-byte public key");
    let public_key = VerifyingKey::from_bytes(&public_key_bytes).expect("Ed25519 public key");
    let signature_bytes: [u8; 64] = hex::decode(token.signature())
        .expect("signature hex")
        .try_into()
        .expect("64-byte signature");
    public_key
        .verify(&canonical, &Signature::from_bytes(&signature_bytes))
        .expect("valid fixture signature");

    let cases = fixture["requestParamsCases"]
        .as_array()
        .expect("request cases");
    assert_eq!(cases.len(), 3);
    for case in cases {
        let params: RequestParams<Value> =
            serde_json::from_value(case["params"].clone()).expect("strict RequestParams");
        let encoded = params.capability_token.expect("capabilityToken");
        let candidate = CapabilityTokenWire::decode_json(&encoded).expect("token wire");
        let expected = case["expected"].as_str().expect("expected outcome");
        let key_matches =
            candidate.key_id() == fixture["publicKey"]["keyId"].as_str().expect("key id");
        let signature: [u8; 64] = hex::decode(candidate.signature())
            .expect("signature hex")
            .try_into()
            .expect("signature length");
        let signature_valid = public_key
            .verify(
                &candidate.canonical_claims_bytes().expect("claims"),
                &Signature::from_bytes(&signature),
            )
            .is_ok();
        assert_eq!(expected == "accepted", key_matches && signature_valid);
    }
}
