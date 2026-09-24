use db_operator_native::registry::WriteLevel;
use db_operator_native::security::{
    CapabilityVerifier, capability_digest, load_capability_verifier,
    validate_private_directory_sddl,
};

#[test]
fn capability_verifier_accepts_only_the_issued_token() {
    let verifier = CapabilityVerifier::from_token("issued-agent-token");

    assert!(verifier.verify("issued-agent-token"));
    assert!(!verifier.verify("wrong-agent-token"));
    assert!(!verifier.verify(""));
}

#[test]
fn capability_storage_is_a_fixed_length_digest() {
    let digest = capability_digest("issued-agent-token");

    assert_eq!(digest.len(), 64);
    assert!(!digest.contains("issued-agent-token"));
}

#[test]
fn capability_verifier_loads_only_valid_digest_hex() {
    let digest = capability_digest("issued-agent-token");
    let verifier = CapabilityVerifier::from_digest_hex(&digest).unwrap();

    assert!(verifier.verify("issued-agent-token"));
    assert!(CapabilityVerifier::from_digest_hex("not-a-digest").is_err());
}

#[test]
fn capability_verifier_loads_from_scoped_private_record() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("capability.json");
    std::fs::write(
        &path,
        format!(
            "{{\"version\":1,\"digest\":\"{}\",\"connection\":\"reporting-prod\"}}",
            capability_digest("issued-agent-token")
        ),
    )
    .unwrap();

    let verifier = load_capability_verifier(&path).unwrap();

    assert!(verifier.verify("issued-agent-token"));
    assert!(verifier.authorizes_connection("reporting-prod"));
    assert!(!verifier.authorizes_connection("finance-prod"));
    assert!(load_capability_verifier(&directory.path().join("missing")).is_err());
}

#[test]
fn capability_record_preserves_write_level() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("capability.json");
    std::fs::write(
        &path,
        format!(
            "{{\"version\":1,\"digest\":\"{}\",\"connection\":\"reporting-prod\",\"write_level\":\"ddl\"}}",
            capability_digest("ddl-token")
        ),
    )
    .unwrap();
    let verifier = load_capability_verifier(&path).unwrap();
    assert_eq!(verifier.write_level(), WriteLevel::Ddl);
    assert!(verifier.verify("ddl-token"));
}

#[test]
fn private_directory_sddl_rejects_inheritance_and_broad_principals() {
    for weak in [
        "D:(A;;FA;;;SY)(A;;FA;;;BA)",
        "D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;AU)",
        "D:P(A;;FA;;;SY)(A;;FA;;;S-1-5-80-1)",
    ] {
        assert!(
            validate_private_directory_sddl(weak).is_err(),
            "must reject {weak}"
        );
    }
    assert!(
        validate_private_directory_sddl("D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FA;;;S-1-5-80-1)").is_ok()
    );
}
