use std::{fs, path::Path, time::Duration};

use ae_sdd_contracts::{
    DocumentId, EngineeringRoute, ReceiptStatus, RequirementAnalysisEvidence, RouteApprovalReceipt,
    RouteBindingInput, RouteMappingVersion, SchemaVersion, SeriesId,
};
use ae_sdd_domain::{
    ArtifactDigest, FreshnessDimension, GateOutcome, GateResult, StateRevision, WorkItemId,
    WorkScale,
};
use ae_sdd_flow::RouteEngine;
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
    fs::write(ra, minimal_v2_srs()).expect("RA document");
}

/// A minimal but structurally complete `ae-sdd-ra-srs/v2` document that the
/// bounded parser accepts. Used by gate_source tests that need an RA fixture
/// to pass the new RA content scanners.
fn minimal_v2_srs() -> String {
    "\
# 需求规格说明书：测试用最小 SRS

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-TEST-001 |
| Work Item | ROUTE-bound |
| Revision | 1 |
| Analysis state | complete |
| Scale | micro |
| Scale confidence | 90 |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 对话 | 测试输入 | v1 | 输入 |

## 1. 问题、目标与非目标
测试用最小 SRS，验证 bound RA 被 G-RA-2 识别。

## 2. 范围
- In Scope：测试。
- Out of Scope：其余。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | not_applicable | 无新参与方 | §3 |
| scenarios | not_applicable | 无独立场景 | §3 |
| state_lifecycle | not_applicable | 无状态变更 | §3 |
| data_semantics | not_applicable | 无数据语义变更 | §3 |
| external_contracts | not_applicable | 无外部契约 | §3 |
| quality_security_compliance | not_applicable | 无 | §3 |
| compatibility_migration_operations | not_applicable | 无 | §3 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 测试需求 | P0 | REF-001 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 可观察 |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| A-001 | 假设 | 测试假设 | 中 | 已确认 |

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 1 | 测试 |
| 参与方、权限或业务域广度 | 1 | 测试 |
| 状态、数据语义与不变量复杂度 | 1 | 测试 |
| 外部契约与协调范围 | 1 | 测试 |
| 性能、安全、合规、可用性等质量风险 | 1 | 测试 |
| 兼容、迁移、回滚和运行影响 | 1 | 测试 |

最高分 = 1 -> Scale = micro。
"
    .to_owned()
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

/// `G-04` asserts a TestCase *document* exists. Reading the Story for the
/// substring `AC-`/`verification` made every Story with acceptance criteria
/// prove the existence of a document that was never written, so the Gate could
/// never report the missing TestCase it exists to guard.
#[test]
fn testcase_existence_rejects_a_story_that_merely_mentions_acceptance_criteria() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    write_state(temp.path(), 1, route_story_state());
    // The fixture Story carries `AC-01`; no TestCase document is installed.
    assert!(
        !temp.path().join("ae-sdd-doc/Test").exists(),
        "the fixture must not carry a TestCase document"
    );

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let result = runtime
        .evaluate("G-04", Duration::from_secs(1))
        .expect("TestCase existence Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Fail(_)),
        "a Story mentioning AC- must not satisfy TestCase existence: {}",
        gate_result_json(&result)
    );
}

/// Work Items created before `documentPaths` carried a `TESTCASE` binding fall
/// back to scanning the tree. The scan derived its directory from the kind
/// (`testcase`), but the canonical directory is `Test/`, so a real TestCase
/// document in its canonical location was invisible and the Gate could never be
/// satisfied for those Work Items.
#[test]
fn testcase_existence_finds_a_canonical_document_without_a_documentpaths_binding() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    // Canonical TestCase documents are named for the Work Item that owns them,
    // which is the Story when a Story owns the sub-chain.
    let testcase = temp
        .path()
        .join("ae-sdd-doc/Test/STORY-ROUTE-10b6bd28/STORY-ROUTE-10b6bd28-testcase.md");
    fs::create_dir_all(testcase.parent().expect("TestCase parent")).expect("TestCase directory");
    fs::write(testcase, "# TestCase\n").expect("TestCase document");
    // Legacy shape: no TESTCASE key in documentPaths.
    let state = route_story_state();
    assert!(
        state["documentPaths"].get("TESTCASE").is_none(),
        "this fixture must reproduce the pre-binding state shape"
    );
    write_state(temp.path(), 1, state);

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let result = runtime
        .evaluate("G-04", Duration::from_secs(1))
        .expect("TestCase existence Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "a canonical Test/<id>/<id>-testcase.md must be found without a binding: {}",
        gate_result_json(&result)
    );
}

/// The same Gate must still pass once the bound TestCase document is present.
#[test]
fn testcase_existence_accepts_a_bound_testcase_document() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    // TestCase binds the owning Story, not the route: Test/<story>/<story>-testcase.md
    let testcase = temp
        .path()
        .join("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    fs::create_dir_all(testcase.parent().expect("TestCase parent")).expect("TestCase directory");
    fs::write(testcase, "# TestCase\n").expect("TestCase document");
    let mut state = route_story_state();
    state["activeStory"] = json!("STORY-001-BE");
    state["storyStates"]["STORY-001-BE"]["testCasePath"] =
        json!("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    write_state(temp.path(), 1, state);

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let result = runtime
        .evaluate("G-04", Duration::from_secs(1))
        .expect("TestCase existence Gate evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "a bound TestCase document must satisfy the Gate: {}",
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
    fs::write(ra_dir.join("ROUTE-10b6bd28.md"), minimal_v2_srs()).expect("current RA");
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
fn ra_gate_key_ignores_foreign_mapping_but_tracks_bound_bytes() {
    let temp = TempDir::new().expect("temp");
    let ra_dir = temp.path().join("ae-sdd-doc/RA");
    fs::create_dir_all(&ra_dir).expect("RA directory");
    let bound = ra_dir.join("WI-001.md");
    fs::write(&bound, minimal_v2_srs()).expect("bound RA");
    fs::write(ra_dir.join("foreign.md"), "# foreign\n").expect("foreign RA");
    write_state(
        temp.path(),
        1,
        json!({"documentPaths":{
            "RA":"ae-sdd-doc/RA/WI-001.md",
            "FOREIGN_RA":"ae-sdd-doc/RA/foreign.md"
        }}),
    );
    let first = runtime(&temp).snapshot_key("G-RA-2").expect("first key");

    write_state(
        temp.path(),
        1,
        json!({"documentPaths":{
            "RA":"ae-sdd-doc/RA/WI-001.md",
            "FOREIGN_RA":"ae-sdd-doc/RA/other.md"
        }}),
    );
    let foreign_changed = runtime(&temp)
        .snapshot_key("G-RA-2")
        .expect("foreign-changed key");
    assert_eq!(
        first.input(),
        foreign_changed.input(),
        "foreign document mappings are outside RequirementAnalysis authority"
    );

    fs::write(&bound, format!("{}\n", minimal_v2_srs())).expect("changed bound RA");
    let bound_changed = runtime(&temp)
        .snapshot_key("G-RA-2")
        .expect("bound-changed key");
    assert_ne!(
        first.input(),
        bound_changed.input(),
        "the exact bound RA bytes must invalidate the Gate key"
    );
}

fn install_route_binding_state(root: &Path, phase: &str) {
    let document = root.join("ae-sdd-doc/RA/WI-001.md");
    fs::create_dir_all(document.parent().expect("RA parent")).expect("RA directory");
    let text = minimal_v2_srs();
    fs::write(&document, &text).expect("RA document");
    let evidence = RequirementAnalysisEvidence::new(
        WorkItemId::new("WI-001").expect("work item"),
        SeriesId::new("SERIES-RA-WI-001").expect("series"),
        DocumentId::new("DOC-RA-WI-001").expect("document"),
        1,
        ArtifactDigest::digest(text.as_bytes()),
        StateRevision::new(1),
        ArtifactDigest::digest(b"verified RA receipt"),
        ReceiptStatus::Verified,
        WorkScale::Small,
        ArtifactDigest::digest(b"scale evidence"),
        ArtifactDigest::digest(b"closure receipt set"),
    );
    let binding = RouteBindingInput::new(evidence.clone(), RouteMappingVersion::V1);
    let candidate = RouteEngine::default()
        .decide_from_evidence(
            &binding,
            WorkItemId::new("WI-001").expect("work item"),
            SchemaVersion::V2,
        )
        .expect("route candidate");
    let approval = RouteApprovalReceipt::new(
        format!("route:{}", candidate.decision_digest()),
        "user".to_owned(),
        "2026-08-10T00:00:00Z".to_owned(),
        evidence.document_id().clone(),
        evidence.version(),
        *evidence.ra_content_digest(),
        evidence.scale(),
        candidate.decision_digest(),
    );
    let frozen = matches!(phase, "route-selected" | "route_selected").then(|| {
        EngineeringRoute::freeze(
            SchemaVersion::V2,
            &binding,
            candidate.clone(),
            &approval,
            &[],
        )
        .expect("frozen route")
    });
    let mut state = json!({
        "phase":phase,
        "currentPhase":phase,
        "documentPaths":{"RA":"ae-sdd-doc/RA/WI-001.md"},
        "seriesReceipts":{"RA":evidence},
        "routeCandidate":candidate,
        "routeApprovalReceipt":approval,
        "routeBlockingConflicts":[],
    });
    if let Some(frozen) = frozen {
        state["engineeringRoute"] = json!(frozen);
    }
    write_state(root, 1, state);
}

#[test]
fn flow_violation_gate_recomputes_the_typed_ra_route_binding() {
    let temp = TempDir::new().expect("temp");
    install_route_binding_state(temp.path(), "requirement_analyzed");
    let pass = runtime(&temp)
        .evaluate("G-RA-FLOW-VIOLATION", Duration::from_secs(1))
        .expect("binding Gate");
    assert!(matches!(pass.outcome(), GateOutcome::Pass));

    install_route_binding_state(temp.path(), "initialized");
    let route_before_ra = runtime(&temp)
        .evaluate("G-RA-FLOW-VIOLATION", Duration::from_secs(1))
        .expect("binding Gate");
    assert!(matches!(route_before_ra.outcome(), GateOutcome::Fail(_)));

    install_route_binding_state(temp.path(), "requirement_analyzed");
    fs::write(
        temp.path().join("ae-sdd-doc/RA/WI-001.md"),
        "stale document bytes",
    )
    .expect("stale RA");
    let stale = runtime(&temp)
        .evaluate("G-RA-FLOW-VIOLATION", Duration::from_secs(1))
        .expect("binding Gate");
    assert!(matches!(stale.outcome(), GateOutcome::Fail(_)));

    install_route_binding_state(temp.path(), "requirement_analyzed");
    let state_path = temp.path().join(".auto-engineering/work-item/state.json");
    let mut state: Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state bytes")).expect("state JSON");
    state["routeBlockingConflicts"] = json!({"malformed":true});
    fs::write(&state_path, serde_json::to_vec(&state).expect("state JSON"))
        .expect("malformed conflict state");
    let malformed_conflicts = runtime(&temp)
        .evaluate("G-RA-FLOW-VIOLATION", Duration::from_secs(1))
        .expect("binding Gate");
    assert!(matches!(
        malformed_conflicts.outcome(),
        GateOutcome::Fail(_)
    ));

    install_route_binding_state(temp.path(), "route-selected");
    let frozen = runtime(&temp)
        .evaluate("G-RA-FLOW-VIOLATION", Duration::from_secs(1))
        .expect("frozen binding Gate");
    assert!(matches!(frozen.outcome(), GateOutcome::Pass));
    let mut state: Value =
        serde_json::from_slice(&fs::read(&state_path).expect("state bytes")).expect("state JSON");
    state["routeApprovalReceipt"]["approvedBy"] = json!("different-user");
    fs::write(&state_path, serde_json::to_vec(&state).expect("state JSON"))
        .expect("mismatched approval state");
    let mismatched_approval = runtime(&temp)
        .evaluate("G-RA-FLOW-VIOLATION", Duration::from_secs(1))
        .expect("frozen binding Gate");
    assert!(matches!(
        mismatched_approval.outcome(),
        GateOutcome::Fail(_)
    ));
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

/// The RA directory accumulates one document per Work Item, so "some RA exists"
/// and "this Work Item's RA is complete" are different questions. Scanner Gates
/// already answer the second by reading `documentPaths/RA`; a predicate that
/// instead takes the alphabetically first file in the directory grades a
/// stranger's document, and its verdict says nothing about this Work Item
/// either way.
#[test]
fn ra_predicates_grade_the_bound_document_not_the_first_in_the_directory() {
    let temp = TempDir::new().expect("temp");
    let directory = temp.path().join("ae-sdd-doc/RA");
    fs::create_dir_all(&directory).expect("RA directory");

    // Sorts first, belongs to another Work Item, and carries the legacy shape:
    // it has the model and enough headings but no v2 schema, so a v2 scanner
    // would reject it. The fingerprint must not even consider this file.
    let mut foreign = String::from("# RA: another Work Item\n\nRequirementAnalysisModel\n");
    for index in 0..12 {
        foreign.push_str(&format!("\n## Section {index}\n\ntext\n"));
    }
    fs::write(directory.join("AAA-OTHER-WORK-ITEM.md"), &foreign).expect("foreign RA");

    // This Work Item's RA is a valid v2 SRS that the new bounded parser accepts.
    let bound = minimal_v2_srs();
    fs::write(directory.join("RA-ROUTE-bound.md"), &bound).expect("bound RA");

    write_state(
        temp.path(),
        1,
        json!({
            "stateMachineName":"ROUTE-bound",
            "entryNode":"ROUTE",
            "documentPaths":{"RA":"ae-sdd-doc/RA/RA-ROUTE-bound.md"}
        }),
    );

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-bound",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");

    let result = runtime
        .evaluate("G-RA-2", Duration::from_secs(1))
        .expect("G-RA-2 evaluation");
    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "G-RA-2 must grade the bound v2 SRS, not a foreign file: {}",
        gate_result_json(&result)
    );
}

/// `G-01` exists to answer one question: does *this* Work Item have a DR. Once
/// `documentPaths` names the document, that name is the answer; scanning the DR
/// directory would accept a neighbouring Work Item's file and leave the Gate
/// unable to ever report a missing DR.
#[test]
fn dr_existence_follows_the_binding_and_ignores_other_work_items_documents() {
    let temp = TempDir::new().expect("temp");
    let directory = temp.path().join("ae-sdd-doc/DR");
    fs::create_dir_all(&directory).expect("DR directory");
    fs::write(
        directory.join("DR-ANOTHER-WORK-ITEM-001.md"),
        "# DR: another Work Item\n",
    )
    .expect("foreign DR");

    let state = json!({
        "stateMachineName":"ROUTE-bound",
        "entryNode":"ROUTE",
        "documentPaths":{"DR":"ae-sdd-doc/DR/ROUTE-bound.md"}
    });
    write_state(temp.path(), 1, state.clone());

    let evaluate = |temp: &TempDir| {
        AuthoritativeGateRuntime::new(
            &workspace(temp.path()),
            "ROUTE-bound",
            &ae_sdd_policy::policy_digest().to_string(),
            Some(3),
        )
        .expect("runtime")
        .evaluate("G-01", Duration::from_secs(1))
        .expect("G-01 evaluation")
    };

    let missing = evaluate(&temp);
    assert!(
        !matches!(missing.outcome(), GateOutcome::Pass),
        "G-01 must not pass while the bound DR is absent: {}",
        gate_result_json(&missing)
    );

    // Writing the bound document is the only thing that may flip the Gate.
    fs::write(directory.join("ROUTE-bound.md"), "# DR: ROUTE-bound\n").expect("bound DR");
    let present = evaluate(&temp);
    assert!(
        matches!(present.outcome(), GateOutcome::Pass),
        "G-01 must pass once the bound DR exists: {}",
        gate_result_json(&present)
    );
}

/// State that carries no binding at all still has to fall back to the directory,
/// so the authority rule above must not turn legacy Work Items into hard blocks.
#[test]
fn dr_existence_still_falls_back_to_the_directory_without_a_binding() {
    let temp = TempDir::new().expect("temp");
    let directory = temp.path().join("ae-sdd-doc/DR");
    fs::create_dir_all(&directory).expect("DR directory");
    fs::write(directory.join("DR-LEGACY-001.md"), "# DR: legacy\n").expect("legacy DR");

    write_state(
        temp.path(),
        1,
        json!({"stateMachineName":"LEGACY-001","documentPaths":{}}),
    );

    let result = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "LEGACY-001",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-01", Duration::from_secs(1))
    .expect("G-01 evaluation");

    assert!(
        matches!(result.outcome(), GateOutcome::Pass),
        "unbound state must keep the directory fallback: {}",
        gate_result_json(&result)
    );
}

/// A document-existence Gate must not keep passing once its bound document is
/// deleted. The Gate key hashed only state fields, so removing the file left
/// `inputFingerprint` unchanged and the scheduler reused the stale PASS — a
/// Blocker Gate satisfied by a document that no longer exists.
#[test]
fn testcase_existence_stops_passing_once_the_bound_document_is_deleted() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    let testcase = temp
        .path()
        .join("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    fs::create_dir_all(testcase.parent().expect("TestCase parent")).expect("TestCase directory");
    fs::write(&testcase, "# TestCase\n").expect("TestCase document");
    let mut state = route_story_state();
    state["activeStory"] = json!("STORY-001-BE");
    state["storyStates"]["STORY-001-BE"]["testCasePath"] =
        json!("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    write_state(temp.path(), 1, state);

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let first = runtime
        .evaluate("G-04", Duration::from_secs(1))
        .expect("first evaluation");
    assert!(
        matches!(first.outcome(), GateOutcome::Pass),
        "the bound document is present: {}",
        gate_result_json(&first)
    );

    // Nothing in the authoritative state changes: only the document goes away.
    fs::remove_file(&testcase).expect("delete the bound TestCase document");
    let second = runtime
        .evaluate("G-04", Duration::from_secs(1))
        .expect("second evaluation");
    assert!(
        matches!(second.outcome(), GateOutcome::Fail(_)),
        "deleting the bound document must not leave a stale PASS: {}",
        gate_result_json(&second)
    );
}

/// `G-CODEPLAN-SRC` asserts that a CodingPlan's `sourceReads` name a file that
/// exists, so deleting that file must fail the Gate. Its selectors hashed the
/// `executionPlan` state and the `changedPaths` files only, so a `sourceReads`
/// entry outside those scopes never entered `inputFingerprint` and the Gate
/// reused a stale PASS after the file was gone.
#[test]
fn source_trace_stops_passing_once_the_read_source_is_deleted() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    // The traced source sits outside `changedPaths` and outside every other
    // declared file scope, which is what made the staleness observable.
    let traced = temp.path().join("source/method-source.md");
    fs::create_dir_all(traced.parent().expect("source parent")).expect("source directory");
    fs::write(&traced, "# method source\n").expect("traced source");
    let mut state = route_story_state();
    state["executionPlan"] = json!({
        "goal":"implement the story",
        "changedPaths":["src/lib.rs"],
        "verification":[],
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":["source/method-source.md"]
    });
    write_state(temp.path(), 1, state);

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let first = runtime
        .evaluate("G-CODEPLAN-SRC", Duration::from_secs(1))
        .expect("first evaluation");
    assert!(
        matches!(first.outcome(), GateOutcome::Pass),
        "the traced source is present: {}",
        gate_result_json(&first)
    );

    fs::remove_file(&traced).expect("delete the traced source");
    let second = runtime
        .evaluate("G-CODEPLAN-SRC", Duration::from_secs(1))
        .expect("second evaluation");
    assert!(
        matches!(second.outcome(), GateOutcome::Fail(_)),
        "deleting the traced source must not leave a stale PASS: {}",
        gate_result_json(&second)
    );
}

/// TestCase is a per-Story Spec: `ae-sdd-design.md` requires an independent
/// `Story -> TestCase -> CodingPlan` subchain for every Story, and a TestCase
/// receipt must bind Story identity. A route-level `documentPaths.TESTCASE`
/// cannot express that — one flat key holds one path for N Stories, and the
/// bound branch matches by substring with no Story filter, so one Story's
/// TestCase satisfied `G-04` for every other Story on the route.
#[test]
fn testcase_existence_is_scoped_to_the_active_story() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    let owned = temp
        .path()
        .join("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    fs::create_dir_all(owned.parent().expect("TestCase parent")).expect("TestCase directory");
    fs::write(
        &owned,
        "# TestCase STORY-001-BE
",
    )
    .expect("owned TestCase");

    let story_states = json!({
        "STORY-001-BE":{
            "phase":"initialized",
            "docPath":"ae-sdd-doc/Story/ROUTE-10b6bd28.md",
            "testCasePath":"ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md"
        },
        // The second Story has no TestCase of its own.
        "STORY-002-BE":{
            "phase":"initialized",
            "docPath":"ae-sdd-doc/Story/ROUTE-10b6bd28.md"
        }
    });
    let digest = ae_sdd_policy::policy_digest().to_string();

    let mut state = route_story_state();
    state["storyStates"] = story_states.clone();
    state["activeStory"] = json!("STORY-001-BE");
    write_state(temp.path(), 1, state);
    let satisfied =
        AuthoritativeGateRuntime::new(&workspace(temp.path()), "ROUTE-10b6bd28", &digest, Some(3))
            .expect("runtime")
            .evaluate("G-04", Duration::from_secs(1))
            .expect("owning Story evaluation");
    assert!(
        matches!(satisfied.outcome(), GateOutcome::Pass),
        "the Story that owns a TestCase passes: {}",
        gate_result_json(&satisfied)
    );

    // Same route, same files: only the active Story changes.
    let borrowed_temp = TempDir::new().expect("temp");
    install_route_story_context(borrowed_temp.path());
    let borrowed_owned = borrowed_temp
        .path()
        .join("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    fs::create_dir_all(borrowed_owned.parent().expect("TestCase parent"))
        .expect("TestCase directory");
    fs::write(
        &borrowed_owned,
        "# TestCase STORY-001-BE
",
    )
    .expect("owned TestCase");
    let mut other = route_story_state();
    other["storyStates"] = story_states;
    other["activeStory"] = json!("STORY-002-BE");
    // The route-level binding is what makes the borrow possible: the bound
    // branch matches `documentPaths` by substring and applies no Story filter,
    // so this one path answers for every Story on the route.
    other["documentPaths"]["TESTCASE"] =
        json!("ae-sdd-doc/Test/STORY-001-BE/STORY-001-BE-testcase.md");
    write_state(borrowed_temp.path(), 1, other);
    let borrowed = AuthoritativeGateRuntime::new(
        &workspace(borrowed_temp.path()),
        "ROUTE-10b6bd28",
        &digest,
        Some(3),
    )
    .expect("runtime")
    .evaluate("G-04", Duration::from_secs(1))
    .expect("borrowing Story evaluation");
    assert!(
        matches!(borrowed.outcome(), GateOutcome::Fail(_)),
        "a Story without its own TestCase must not borrow another's: {}",
        gate_result_json(&borrowed)
    );
}

/// `G-CODEPLAN-SRC` asserts the plan's source trace is *complete*, but the
/// predicate accepted any single surviving entry. A plan tracing several
/// sources therefore kept passing after one was deleted or relocated: the
/// fingerprint moved, the Gate re-evaluated, and the surviving sibling
/// answered for the missing file. F-09's test could not catch this because it
/// declared a single `sourceReads` entry, where "any" and "all" coincide.
#[test]
fn source_trace_requires_every_declared_read_to_survive() {
    let temp = TempDir::new().expect("temp");
    install_route_story_context(temp.path());
    let kept = temp.path().join("source/kept-source.md");
    let removed = temp.path().join("source/removed-source.md");
    fs::create_dir_all(kept.parent().expect("source parent")).expect("source directory");
    fs::write(&kept, "# kept\n").expect("kept source");
    fs::write(&removed, "# removed\n").expect("removed source");
    let mut state = route_story_state();
    state["executionPlan"] = json!({
        "goal":"implement the story",
        "changedPaths":["src/lib.rs"],
        "verification":[],
        "risks":["fixture risk"],
        "approved":true,
        "sourceReads":["source/kept-source.md","source/removed-source.md"]
    });
    write_state(temp.path(), 1, state);

    let runtime = AuthoritativeGateRuntime::new(
        &workspace(temp.path()),
        "ROUTE-10b6bd28",
        &ae_sdd_policy::policy_digest().to_string(),
        Some(3),
    )
    .expect("runtime");
    let both = runtime
        .evaluate("G-CODEPLAN-SRC", Duration::from_secs(1))
        .expect("first evaluation");
    assert!(
        matches!(both.outcome(), GateOutcome::Pass),
        "every traced source is present: {}",
        gate_result_json(&both)
    );

    fs::remove_file(&removed).expect("delete one traced source");
    let partial = runtime
        .evaluate("G-CODEPLAN-SRC", Duration::from_secs(1))
        .expect("second evaluation");
    assert!(
        matches!(partial.outcome(), GateOutcome::Fail(_)),
        "one surviving sibling must not answer for a missing traced source: {}",
        gate_result_json(&partial)
    );
}
