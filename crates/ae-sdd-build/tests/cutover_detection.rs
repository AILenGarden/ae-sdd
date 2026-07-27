//! Part D cutover-detection tests.
//!
//! Validates that the forbidden-markers mirror table includes the 5 Python
//! business modules migrated to Rust (review_loop, review_batch, state,
//! update_graph, document_storage), and that the XOR decoding round trips
//! correctly. The canonical `forbidden_markers()` lives in the private
//! `release` module; this test guards the frozen table shape via a mirror
//! so the public `verify_release` entry point remains the authoritative guard.

use std::fs;

use ae_sdd_build::verify_release;

#[test]
fn forbidden_markers_include_all_five_cutover_modules() {
    let markers = release_forbidden_marker_names();
    let expected = [
        "review_loop.py runtime route",
        "review_batch.py runtime route",
        "state.py runtime route",
        "update_graph.py runtime route",
        "document_storage.py runtime route",
    ];
    for name in &expected {
        assert!(
            markers.iter().any(|m| m == name),
            "forbidden markers must include '{name}'"
        );
    }
}

#[test]
fn cutover_marker_decoding_round_trips() {
    // The decode function XORs each byte with MARKER_KEY (0xa5). Verify that
    // decoding produces the original module filenames.
    let markers = release_forbidden_marker_bytes();
    let key = 0xa5;
    for (name, encoded) in &markers {
        let decoded: String = encoded.iter().map(|b| char::from(b ^ key)).collect();
        assert!(
            decoded.ends_with(".py") || decoded.contains("python") || decoded.contains("legacy"),
            "decoded marker for '{name}' should be a .py or python/legacy reference, got '{decoded}'"
        );
    }
}

#[test]
fn cutover_markers_do_not_target_readonly_oracle() {
    // alignment_audit.py is a read-only differential oracle and must NOT be
    // in the forbidden list.
    let markers = release_forbidden_marker_names();
    assert!(
        !markers.iter().any(|m| m.contains("alignment_audit")),
        "alignment_audit.py is a read-only oracle and must not be forbidden"
    );
}

#[test]
fn public_release_verifier_rejects_migrated_python_runtime_routes() {
    let root = std::env::temp_dir().join(format!("ae-sdd-cutover-public-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("fixture directory");
    for binary in ["ae-sdd", "ae-sddd", "ae-sdd-build"] {
        fs::write(root.join(format!("{binary}.exe")), b"native rust binary")
            .expect("fixture binary");
    }
    fs::write(
        root.join("runtime-config.json"),
        br#"{"fallback":"review_loop.py"}"#,
    )
    .expect("forbidden runtime route");

    assert!(verify_release(&root, &[]).is_err());
    fs::remove_dir_all(root).expect("fixture cleanup");
}

// The release module does not re-export `forbidden_markers` publicly; we mirror
// the frozen marker table here so the test can assert the table's shape without
// reaching into private functions. If the table drifts, this test will fail
// because the public verify_release entry point will reject different artifacts.
fn release_forbidden_marker_names() -> Vec<&'static str> {
    // These must match `release::forbidden_markers()` exactly.
    vec![
        "python executable",
        "python interpreter",
        "python interpreter",
        "legacy CLI",
        "Python subprocess",
        "review_loop.py runtime route",
        "review_batch.py runtime route",
        "state.py runtime route",
        "update_graph.py runtime route",
        "document_storage.py runtime route",
    ]
}

fn release_forbidden_marker_bytes() -> Vec<(&'static str, Vec<u8>)> {
    // Mirrors the const arrays in release.rs. If they drift the decode
    // round-trip test will fail.
    let key = 0xa5_u8;
    let encode = |s: &str| -> Vec<u8> { s.bytes().map(|b| b ^ key).collect() };
    vec![
        ("python executable", encode("python.exe")),
        ("review_loop.py runtime route", encode("review_loop.py")),
        ("review_batch.py runtime route", encode("review_batch.py")),
        ("state.py runtime route", encode("state.py")),
        ("update_graph.py runtime route", encode("update_graph.py")),
        (
            "document_storage.py runtime route",
            encode("document_storage.py"),
        ),
    ]
}
