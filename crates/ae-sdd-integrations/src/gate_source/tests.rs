use std::{fs, path::Path, time::Duration};

use ae_sdd_domain::{ArtifactDigest, FreshnessDimension, GateOutcome, GateResult};
use ae_sdd_protocol::WorkspaceMode;
use ae_sdd_runtime::BusinessWorkspace;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

use super::{
    AuthoritativeGateRuntime, contracts::plan_contract_complete, gate_result_json,
    predicate::ac_ids,
};

fn workspace(root: &Path) -> BusinessWorkspace {
    BusinessWorkspace {
        workspace_id: Uuid::from_u128(7).to_string(),
        canonical_root: root.to_string_lossy().into_owned(),
        project_key: "gate-test".to_owned(),
        mode: WorkspaceMode::Shadow,
        agent_role: None,
        agent_grant: None,
        caller_kind: None,
        inventory_generation: 1,
    }
}

fn write_state(root: &Path, revision: u64, extra: Value) {
    let directory = root.join(".auto-engineering/work-item");
    fs::create_dir_all(&directory).expect("state directory");
    let mut value = json!({
        "stateMachineName":"WI-001",
        "currentWorkItem":"WI-001",
        "activeStory":"STORY-001",
        "revision":revision,
        "lastFencingToken":3
    });
    value
        .as_object_mut()
        .expect("object")
        .extend(extra.as_object().expect("extra").clone());
    fs::write(
        directory.join("state.json"),
        serde_json::to_vec(&value).expect("json"),
    )
    .expect("state write");
}

fn runtime(temp: &TempDir) -> AuthoritativeGateRuntime {
    AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "WI-001",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
}

#[test]
fn bare_gate_results_pass_is_never_trusted() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({"gateResults":{"G-14":{"outcome":"PASS"}}}),
    );
    let result = runtime(&temp)
        .evaluate("G-14", Duration::from_secs(1))
        .expect("Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
    assert_eq!(gate_result_json(&result)["outcome"]["kind"], "FAIL");
}

#[test]
fn state_revision_change_invalidates_a_recorded_pass() {
    let temp = TempDir::new().expect("temp");
    write_state(temp.path(), 1, json!({}));
    let runtime = runtime(&temp);
    let snapshot = runtime.snapshot_key("G-14").expect("snapshot");
    write_state(temp.path(), 2, json!({}));
    let current = runtime.current_key("G-14").expect("current");
    let outcome = GateResult::new(snapshot, GateOutcome::Pass).outcome_against(&current);

    let GateOutcome::Stale(stale) = outcome else {
        panic!("revision drift must return STALE");
    };
    assert!(stale.changed().contains(&FreshnessDimension::StateRevision));
}

fn install_dr_context(root: &Path) {
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("project asset");
    let constraints = root.join("constraints");
    fs::create_dir_all(&constraints).expect("constraints directory");
    fs::write(
        constraints.join("README.md"),
        "| File | Owner |\n| --- | --- |\n| `testing.md` | tests |\n",
    )
    .expect("constraints index");
    fs::write(constraints.join("testing.md"), "# Testing\n").expect("indexed constraint");
    for name in ["api.md", "security.md", "code-style.md"] {
        fs::write(constraints.join(name), format!("# {name}\n")).expect("project constraint");
    }
    let standards = root.join("source/standards");
    fs::create_dir_all(&standards).expect("standards directory");
    fs::write(standards.join("operation-protocol.md"), "# Protocol\n").expect("standard");
    let ra = root.join("ae-sdd-doc/RA/ROUTE-10b6bd28.md");
    fs::create_dir_all(ra.parent().expect("RA parent")).expect("RA directory");
    fs::write(ra, "# Requirement Analysis\n").expect("RA document");
}

#[test]
fn dr_context_uses_bound_route_project_truth_instead_of_loaded_context_flags() {
    let temp = TempDir::new().expect("temp");
    install_dr_context(temp.path());
    write_state(
        temp.path(),
        1,
        json!({
            "stateMachineName":"ROUTE-10b6bd28",
            "entryNode":"ROUTE",
            "documentPaths":{"RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md"}
        }),
    );

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-DR-CTX", Duration::from_secs(1))
    .expect("DR context Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "{}",
        gate_result_json(&result)
    );
}

#[test]
fn dr_context_rejects_loaded_context_flags_without_project_truth() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({
            "stateMachineName":"ROUTE-10b6bd28",
            "loadedContexts":{
                "prd":{"complete":true,"source":"claimed"},
                "assets":{"complete":true,"source":"claimed"},
                "constraints":{"complete":true,"source":"claimed"},
                "standards":{"complete":true,"source":"claimed"}
            }
        }),
    );

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-DR-CTX", Duration::from_secs(1))
    .expect("DR context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn dr_context_fails_closed_when_any_route_context_is_missing() {
    for missing in ["ra", "constraints", "assets", "standards"] {
        let temp = TempDir::new().expect("temp");
        install_dr_context(temp.path());
        match missing {
            "ra" => fs::remove_file(temp.path().join("ae-sdd-doc/RA/ROUTE-10b6bd28.md"))
                .expect("remove RA"),
            "constraints" => fs::remove_file(temp.path().join("constraints/testing.md"))
                .expect("remove indexed constraint"),
            "assets" => {
                fs::remove_file(temp.path().join("Cargo.toml")).expect("remove project asset")
            }
            "standards" => {
                fs::remove_file(temp.path().join("source/standards/operation-protocol.md"))
                    .expect("remove standard")
            }
            _ => unreachable!(),
        }
        write_state(
            temp.path(),
            1,
            json!({
                "stateMachineName":"ROUTE-10b6bd28",
                "entryNode":"ROUTE",
                "documentPaths":{"RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md"}
            }),
        );

        let result = AuthoritativeGateRuntime::new(
            &workspace(temp.path()),
            "ROUTE-10b6bd28",
            &ae_sdd_policy::policy_digest().to_string(),
            Some(3),
        )
        .expect("runtime")
        .evaluate("G-DR-CTX", Duration::from_secs(1))
        .expect("DR context Gate evaluation");

        assert!(
            matches!(result.outcome(), GateOutcome::Fail(_)),
            "missing {missing} must fail closed"
        );
    }
}

#[test]
fn dr_context_requires_a_bound_prd_outside_route_work_items() {
    let temp = TempDir::new().expect("temp");
    install_dr_context(temp.path());
    write_state(
        temp.path(),
        1,
        json!({"documentPaths":{"RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md"}}),
    );

    let result = runtime(&temp)
        .evaluate("G-DR-CTX", Duration::from_secs(1))
        .expect("DR context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn dr_context_rejects_a_route_key_with_a_mismatched_entry_node() {
    let temp = TempDir::new().expect("temp");
    install_dr_context(temp.path());
    write_state(
        temp.path(),
        1,
        json!({
            "stateMachineName":"ROUTE-10b6bd28",
            "entryNode":"DR",
            "documentPaths":{"RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md"}
        }),
    );

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-DR-CTX", Duration::from_secs(1))
    .expect("DR context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn dr_context_rejects_wrong_kind_and_aliased_document_bindings() {
    for ra_path in ["Cargo.toml", "ae-sdd-doc/PRD/ROUTE-10b6bd28.md"] {
        let temp = TempDir::new().expect("temp");
        install_dr_context(temp.path());
        let prd = temp.path().join("ae-sdd-doc/PRD/ROUTE-10b6bd28.md");
        fs::create_dir_all(prd.parent().expect("PRD parent")).expect("PRD directory");
        fs::write(prd, "# PRD\n").expect("PRD document");
        write_state(
            temp.path(),
            1,
            json!({
                "stateMachineName":"ROUTE-10b6bd28",
                "entryNode":"ROUTE",
                "documentPaths":{"RA":ra_path,"PRD":ra_path}
            }),
        );

        let result = AuthoritativeGateRuntime::new(
            &workspace(temp.path()),
            "ROUTE-10b6bd28",
            &ae_sdd_policy::policy_digest().to_string(),
            Some(3),
        )
        .expect("runtime")
        .evaluate("G-DR-CTX", Duration::from_secs(1))
        .expect("DR context Gate evaluation");

        assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
    }
}

#[test]
fn dr_context_rejects_a_project_manifest_symlink_escape() {
    let temp = TempDir::new().expect("temp");
    let outside = TempDir::new().expect("outside");
    install_dr_context(temp.path());
    fs::remove_file(temp.path().join("Cargo.toml")).expect("remove project asset");
    fs::write(outside.path().join("Cargo.toml"), "[workspace]\n").expect("outside asset");
    let link = temp.path().join("Cargo.toml");

    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(outside.path().join("Cargo.toml"), &link);
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(outside.path().join("Cargo.toml"), &link);

    if let Err(error) = linked {
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
            ) || error.raw_os_error() == Some(1314),
            "unexpected symlink setup failure: {error}"
        );
        return;
    }
    write_state(
        temp.path(),
        1,
        json!({
            "stateMachineName":"ROUTE-10b6bd28",
            "entryNode":"ROUTE",
            "documentPaths":{"RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md"}
        }),
    );

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-DR-CTX", Duration::from_secs(1))
    .expect("DR context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn dr_context_rejects_a_prd_symlink_to_the_bound_ra() {
    let temp = TempDir::new().expect("temp");
    install_dr_context(temp.path());
    let prd = temp.path().join("ae-sdd-doc/PRD/PRD-001.md");
    fs::create_dir_all(prd.parent().expect("PRD parent")).expect("PRD directory");
    let ra = temp.path().join("ae-sdd-doc/RA/ROUTE-10b6bd28.md");

    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&ra, &prd);
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&ra, &prd);

    if let Err(error) = linked {
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
            ) || error.raw_os_error() == Some(1314),
            "unexpected symlink setup failure: {error}"
        );
        return;
    }
    write_state(
        temp.path(),
        1,
        json!({
            "documentPaths":{
                "RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md",
                "PRD":"ae-sdd-doc/PRD/PRD-001.md"
            }
        }),
    );

    let result = runtime(&temp)
        .evaluate("G-DR-CTX", Duration::from_secs(1))
        .expect("DR context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

fn install_route_story_context(root: &Path) {
    install_dr_context(root);
    let dr = root.join("ae-sdd-doc/DR/ROUTE-10b6bd28.md");
    fs::create_dir_all(dr.parent().expect("DR parent")).expect("DR directory");
    fs::write(dr, "# DR\n").expect("DR document");
    let story = root.join("ae-sdd-doc/Story/ROUTE-10b6bd28.md");
    fs::create_dir_all(story.parent().expect("Story parent")).expect("Story directory");
    fs::write(story, "# Story\n\nAC-01\n").expect("Story document");
}

fn route_story_state() -> Value {
    json!({
        "stateMachineName":"ROUTE-10b6bd28",
        "entryNode":"ROUTE",
        "activeStory":"STORY-ROUTE-10b6bd28",
        "selectedDesign":"DR",
        "routeDocuments":{"RA":true,"DR":true,"STORY":true},
        "documentPaths":{
            "RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md",
            "DR":"ae-sdd-doc/DR/ROUTE-10b6bd28.md",
            "STORY":"ae-sdd-doc/Story/ROUTE-10b6bd28.md"
        },
        "storyStates":{
            "STORY-ROUTE-10b6bd28":{"docPath":"ae-sdd-doc/Story/ROUTE-10b6bd28.md"}
        },
        "executionPlan":{"sourceReads":["constraints/testing.md"]}
    })
}

#[test]
fn story_review_accepts_an_atomically_committed_route_story() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    write_state(temp.path(), 1, route_story_state());

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let story_exists = runtime
        .evaluate("G-02", Duration::from_secs(1))
        .expect("Story existence Gate evaluation");
    assert!(
        matches!(story_exists.outcome(), GateOutcome::Pass),
        "{}",
        gate_result_json(&story_exists)
    );
    let result = runtime
        .evaluate("G-03", Duration::from_secs(1))
        .expect("Story review Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "{}",
        gate_result_json(&result)
    );
}

#[test]
fn story_context_uses_route_project_truth_instead_of_loaded_context_flags() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    write_state(temp.path(), 1, route_story_state());

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-STORY-CTX", Duration::from_secs(1))
    .expect("Story context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Pass));
}

#[test]
fn story_review_rejects_a_marker_without_matching_story_authority() {
    for authority in [
        json!({"activeStory":null,"storyStates":{}}),
        json!({
            "activeStory":"ROUTE-10b6bd28",
            "storyStates":{"ROUTE-10b6bd28":{"docPath":"ae-sdd-doc/Story/ROUTE-10b6bd28.md"}}
        }),
        json!({
            "activeStory":"STORY-ROUTE-10b6bd28",
            "storyStates":{"STORY-ROUTE-10b6bd28":{"docPath":"ae-sdd-doc/Story/OTHER.md"}}
        }),
    ] {
        let temp = TempDir::new().expect("temp");
        install_route_story_context(temp.path());
        let mut state = route_story_state();
        state["activeStory"] = authority["activeStory"].clone();
        state["storyStates"] = authority["storyStates"].clone();
        write_state(temp.path(), 1, state);

        let result = AuthoritativeGateRuntime::new(
            &workspace(temp.path()),
            "ROUTE-10b6bd28",
            &ae_sdd_policy::policy_digest().to_string(),
            Some(3),
        )
        .expect("runtime")
        .evaluate("G-03", Duration::from_secs(1))
        .expect("Story review Gate evaluation");

        assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
    }
}

#[test]
fn story_context_rejects_loaded_flags_without_route_project_truth() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({
            "stateMachineName":"ROUTE-10b6bd28",
            "entryNode":"ROUTE",
            "loadedContexts":{
                "constraints":{"complete":true,"source":"claimed"},
                "assets":{"complete":true,"source":"claimed"},
                "sourceTrace":{"complete":true,"source":"claimed"}
            }
        }),
    );

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-STORY-CTX", Duration::from_secs(1))
    .expect("Story context Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn ra_scanner_ignores_unrelated_ra_documents() {
    let temp = TempDir::new().expect("temp");
    let ra_dir = temp.path().join("ae-sdd-doc/RA");
    fs::create_dir_all(&ra_dir).expect("RA directory");
    fs::write(
        ra_dir.join("ROUTE-10b6bd28.md"),
        "# Current RA\n\nEvery deadline is bounded to 30 seconds.\n",
    )
    .expect("current RA");
    fs::write(
        ra_dir.join("unrelated.md"),
        "# Historical RA\n\nThis unrelated document says \u{7acb}\u{5373}.\n",
    )
    .expect("unrelated RA");
    write_state(
        temp.path(),
        1,
        json!({"documentPaths":{"RA":"ae-sdd-doc/RA/ROUTE-10b6bd28.md"}}),
    );

    let result = runtime(&temp)
        .evaluate("G-RA-4", Duration::from_secs(1))
        .expect("RA Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Pass));
}

#[test]
fn ra_scanner_rejects_a_non_ra_document_path() {
    let temp = TempDir::new().expect("temp");
    fs::write(
        temp.path().join("README.md"),
        "# Project\n\nEvery deadline is bounded to 30 seconds.\n",
    )
    .expect("README");
    write_state(temp.path(), 1, json!({"documentPaths":{"RA":"README.md"}}));

    let result = runtime(&temp)
        .evaluate("G-RA-4", Duration::from_secs(1))
        .expect("RA Gate evaluation");

    assert_eq!(gate_result_json(&result)["outcome"]["kind"], "ERROR");
    assert_eq!(
        gate_result_json(&result)["outcome"]["code"],
        "SCANNER_SCOPE_FAILED"
    );
}

#[test]
fn ra_scanner_rejects_a_missing_ra_document() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({"documentPaths":{"RA":"ae-sdd-doc/RA/RA-MISSING-001.md"}}),
    );

    let result = runtime(&temp)
        .evaluate("G-RA-4", Duration::from_secs(1))
        .expect("RA Gate evaluation");

    assert_eq!(gate_result_json(&result)["outcome"]["kind"], "ERROR");
    assert_eq!(
        gate_result_json(&result)["outcome"]["code"],
        "SCANNER_EXECUTION_FAILED"
    );
}

#[test]
fn ra_scanner_rejects_an_escaping_document_path() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({"documentPaths":{"RA":"../RA-ESCAPE-001.md"}}),
    );

    let result = runtime(&temp)
        .evaluate("G-RA-4", Duration::from_secs(1))
        .expect("RA Gate evaluation");

    assert_eq!(gate_result_json(&result)["outcome"]["kind"], "ERROR");
    assert_eq!(
        gate_result_json(&result)["outcome"]["code"],
        "SCANNER_SCOPE_FAILED"
    );
}

#[test]
fn test_scanner_uses_only_execution_plan_changed_paths() {
    let temp = TempDir::new().expect("temp");
    let changed = temp.path().join("crates/demo/tests/changed.rs");
    fs::create_dir_all(changed.parent().expect("changed parent")).expect("changed directory");
    fs::write(
        &changed,
        "#[test]\nfn changed_behavior() { assert_eq!(2 + 2, 4); }\n",
    )
    .expect("changed test");
    let unrelated = temp.path().join("crates/scanner/tests/unrelated.rs");
    fs::create_dir_all(unrelated.parent().expect("unrelated parent")).expect("unrelated directory");
    fs::write(
        unrelated,
        "#[test]\n#[ignore]\nfn scanner_fixture() { assert_eq!(2 + 2, 4); }\n",
    )
    .expect("unrelated scanner fixture");
    write_state(
        temp.path(),
        1,
        json!({"executionPlan":{"changedPaths":["crates/demo/tests/changed.rs"]}}),
    );

    let result = runtime(&temp)
        .evaluate("G-09", Duration::from_secs(1))
        .expect("test authenticity Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "{}",
        gate_result_json(&result)
    );
}

#[test]
fn coding_scanner_uses_only_execution_plan_changed_paths() {
    let temp = TempDir::new().expect("temp");
    let changed = temp.path().join("src/lib.rs");
    fs::create_dir_all(changed.parent().expect("changed parent")).expect("source directory");
    fs::write(&changed, "pub fn answer() -> u8 { 42 }\n").expect("changed source");
    let unrelated = temp.path().join("config/unrelated.yaml");
    fs::create_dir_all(unrelated.parent().expect("unrelated parent")).expect("config directory");
    fs::write(unrelated, "key: [unterminated\n").expect("unrelated malformed config");
    write_state(
        temp.path(),
        1,
        json!({"executionPlan":{"changedPaths":["src/lib.rs"]}}),
    );

    let result = runtime(&temp)
        .evaluate("G-CODE-1", Duration::from_secs(1))
        .expect("coding authenticity Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "{}",
        gate_result_json(&result)
    );
}

#[test]
fn evidence_and_execution_authority_satisfy_post_coding_gates() {
    let temp = TempDir::new().expect("temp");
    let evidence = temp.path().join(".auto-engineering/STORY-001/evidence");
    fs::create_dir_all(&evidence).expect("evidence directory");
    let ledger = b"{}\n";
    let manifest = b"{}\n";
    fs::write(evidence.join("ledger.jsonl"), ledger).expect("ledger");
    fs::write(evidence.join("manifest.json"), manifest).expect("manifest");
    write_state(
        temp.path(),
        1,
        json!({
            "evidenceAuthority":{
                "ledgerRef":".auto-engineering/STORY-001/evidence/ledger.jsonl",
                "ledgerDigest":format!("sha256:{}", ArtifactDigest::digest(ledger)),
                "manifestRef":".auto-engineering/STORY-001/evidence/manifest.json",
                "manifestDigest":format!("sha256:{}", ArtifactDigest::digest(manifest))
            },
            "executionRuntime":{
                "activeSliceStatus":"completed",
                "capsuleRef":".auto-engineering/WI-001/execution/capsule.json",
                "capsuleDigest":format!("sha256:{}", "3".repeat(64)),
                "ledgerRef":".auto-engineering/WI-001/execution/ledger.jsonl",
                "ledgerDigest":format!("sha256:{}", "4".repeat(64))
            }
        }),
    );

    for gate_id in ["G-10", "G-11"] {
        let result = runtime(&temp)
            .evaluate(gate_id, Duration::from_secs(1))
            .unwrap_or_else(|error| panic!("{gate_id} evaluation failed: {error:?}"));
        assert!(
            matches!(result.outcome(), GateOutcome::Pass),
            "{gate_id}: {}",
            gate_result_json(&result)
        );
    }
}

#[test]
fn evidence_authority_rejects_a_mismatched_file_digest() {
    let temp = TempDir::new().expect("temp");
    let evidence = temp.path().join(".auto-engineering/STORY-001/evidence");
    fs::create_dir_all(&evidence).expect("evidence directory");
    fs::write(evidence.join("ledger.jsonl"), "{}\n").expect("ledger");
    fs::write(evidence.join("manifest.json"), "{}\n").expect("manifest");
    write_state(
        temp.path(),
        1,
        json!({
            "evidenceAuthority":{
                "ledgerRef":".auto-engineering/STORY-001/evidence/ledger.jsonl",
                "ledgerDigest":format!("sha256:{}", "1".repeat(64)),
                "manifestRef":".auto-engineering/STORY-001/evidence/manifest.json",
                "manifestDigest":format!("sha256:{}", "2".repeat(64))
            }
        }),
    );

    let result = runtime(&temp)
        .evaluate("G-10", Duration::from_secs(1))
        .expect("evidence Gate evaluation");

    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn incomplete_evidence_and_execution_authority_fail_closed() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({
            "evidenceAuthority":{"manifestRef":"missing/manifest.json"},
            "executionRuntime":{"activeSliceStatus":"completed"}
        }),
    );

    for gate_id in ["G-10", "G-11"] {
        let result = runtime(&temp)
            .evaluate(gate_id, Duration::from_secs(1))
            .unwrap_or_else(|error| panic!("{gate_id} evaluation failed: {error:?}"));
        assert!(
            matches!(result.outcome(), GateOutcome::Fail(_)),
            "{gate_id} must fail closed"
        );
    }
}

fn verification_row(index: u32) -> Value {
    json!({
        "id":format!("V-{index:03}"),
        "acId":format!("AC-{index}"),
        "boundary":"unit",
        "command":"cargo test",
        "expected":"pass"
    })
}

fn approved_plan(verification: Vec<Value>) -> Value {
    json!({
        "goal":"implement the story",
        "changedPaths":["src/lib.rs"],
        "verification":verification,
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":["src/lib.rs"]
    })
}

fn complete_plan() -> Value {
    approved_plan((1..=9).map(verification_row).collect())
}

#[test]
fn plan_contract_complete_accepts_nine_complete_verification_rows() {
    // Regression: G-08 is a plan-completeness check, not a row count; a Story
    // with fewer than fourteen ACs must still pass.
    assert!(plan_contract_complete(&complete_plan()));
}

#[test]
fn plan_contract_complete_rejects_an_empty_verification_matrix() {
    let plan = approved_plan(Vec::new());
    assert!(!plan_contract_complete(&plan));
}

#[test]
fn plan_contract_complete_rejects_a_row_with_an_empty_field() {
    let mut verification: Vec<Value> = (1..=9).map(verification_row).collect();
    verification[3]["expected"] = json!("");
    assert!(!plan_contract_complete(&approved_plan(verification)));
}

#[test]
fn plan_contract_complete_rejects_an_unapproved_plan() {
    let mut plan = complete_plan();
    plan["approved"] = json!(false);
    assert!(!plan_contract_complete(&plan));
}

#[test]
fn plan_contract_complete_rejects_missing_or_blank_source_reads() {
    let mut missing = complete_plan();
    missing.as_object_mut().expect("plan").remove("sourceReads");
    assert!(!plan_contract_complete(&missing));

    let mut empty = complete_plan();
    empty["sourceReads"] = json!([]);
    assert!(!plan_contract_complete(&empty));

    let mut blank = complete_plan();
    blank["sourceReads"] = json!(["  "]);
    assert!(!plan_contract_complete(&blank));
}

/// Installs a Story document declaring `acs` and points the state at it.
fn install_story(root: &Path, acs: &str) {
    let path = root.join("ae-sdd-doc/Story/STORY-001.md");
    fs::create_dir_all(path.parent().expect("story parent")).expect("story directory");
    fs::write(path, format!("# Story\n\n{acs}\n")).expect("story document");
}

fn story_state(plan: Value) -> Value {
    json!({
        "executionPlan":plan,
        "storyStates":{"STORY-001":{"docPath":"ae-sdd-doc/Story/STORY-001.md"}}
    })
}

#[test]
fn g08_passes_with_nine_rows_covering_every_story_ac() {
    let temp = TempDir::new().expect("temp");
    install_story(temp.path(), "AC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7 AC-8 AC-9");
    write_state(temp.path(), 1, story_state(complete_plan()));

    let result = runtime(&temp)
        .evaluate("G-08", Duration::from_secs(1))
        .expect("Gate evaluation");
    assert!(matches!(result.outcome(), GateOutcome::Pass));
}

#[test]
fn g08_fails_when_the_plan_misses_a_story_ac() {
    let temp = TempDir::new().expect("temp");
    install_story(
        temp.path(),
        "AC-1 AC-2 AC-3 AC-4 AC-5 AC-6 AC-7 AC-8 AC-9 AC-10",
    );
    write_state(temp.path(), 1, story_state(complete_plan()));

    let result = runtime(&temp)
        .evaluate("G-08", Duration::from_secs(1))
        .expect("Gate evaluation");
    assert!(matches!(result.outcome(), GateOutcome::Fail(_)));
}

#[test]
fn g08_skips_story_coverage_without_an_active_story() {
    let temp = TempDir::new().expect("temp");
    write_state(
        temp.path(),
        1,
        json!({"activeStory":null, "executionPlan":complete_plan()}),
    );

    let result = runtime(&temp)
        .evaluate("G-08", Duration::from_secs(1))
        .expect("Gate evaluation");
    assert!(matches!(result.outcome(), GateOutcome::Pass));
}

#[test]
fn ac_ids_accepts_descriptive_and_numeric_suffixes() {
    let ids = ac_ids("AC-1 AC-001 AC-NAME-01 AC-DC AC-");

    assert!(ids.contains("AC-1"));
    assert!(ids.contains("AC-001"));
    assert!(ids.contains("AC-NAME-01"));
    assert!(!ids.contains("AC-DC"));
    assert_eq!(ids.len(), 3);
}
