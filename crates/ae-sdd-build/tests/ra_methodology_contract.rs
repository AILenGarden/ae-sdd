//! RA methodology source contract (Task 2 RED).
//!
//! These tests pin the source contract that the RA full SKILL and the single
//! SRS template must satisfy after the generality refactor. They read the
//! authoritative source assets directly (not a rendered copy) so a regression
//! that re-introduces the old multi-artifact, implementation-perspective, or
//! fixed-loop RA is caught before any downstream build.
//!
//! Contract scope:
//! - The full SKILL owns the new pure-need semantics: single `intent=RA`
//!   output, no front-side PRD/Issue, no RA GeneratePlan/Impact/etc. sidecars,
//!   no RA-G01~16 / I1-I7 / R-R' / 六类业务模式 / DR handoff / 下游关联矩阵 /
//!   fixed three-round / per-step confirmation.
//! - The template is a single adaptive SRS: schema `ae-sdd-ra-srs/v2`, the
//!   seven applicability keys, REQ/AC/REF/GAP ids, and a pure-need six-dimension
//!   scale rubric that takes the max score.
//! - Approval is held by an external daemon receipt, never written back into
//!   the approved SRS content.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read_source(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read source {relative:?} at {}: {error}", path.display()))
}

fn ra_full_skill() -> String {
    read_source("source/skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md")
}

fn ra_template() -> String {
    read_source("source/templates/design/ra-template.md")
}

fn source_asset(relative: &str) -> String {
    read_source(relative)
}

#[test]
fn update_graph_has_one_ra_srs_v2_alignment_node_covering_every_consumer() {
    let graph: serde_json::Value =
        serde_json::from_str(&source_asset("source/standards/update-graph.json"))
            .expect("update graph must be valid JSON");
    let rules = graph["rules"].as_array().expect("update graph rules array");
    let matching: Vec<&serde_json::Value> = rules
        .iter()
        .filter(|rule| rule["name"] == "ra-srs-v2-alignment")
        .collect();
    assert_eq!(matching.len(), 1, "RA SRS v2 must have one alignment node");

    let affected = matching[0]["affected"]
        .as_array()
        .expect("RA alignment affected array");
    let paths: Vec<&str> = affected
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect();
    for required in [
        "source/skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md",
        "source/templates/design/ra-template.md",
        "crates/ae-sdd-scanners/src/ra_specification.rs",
        "crates/ae-sdd-gates/src/registry.rs",
        "crates/ae-sdd-policy/src/transition.rs",
        "source/standards/runtime/methodology-catalog.v1.json",
        "tests/fixtures/gates/ra-v2/**",
        "source/skills/phase1-design/requirement-analysis-skill.md",
    ] {
        assert!(
            paths.contains(&required),
            "RA alignment misses `{required}`"
        );
    }
    let encoded = serde_json::to_string(matching[0]).expect("RA alignment serializes");
    for alias in ["G-RA-5", "G-RA-6", "G-RA-FLOW-VIOLATION"] {
        assert!(
            encoded.contains(alias),
            "RA alignment misses alias `{alias}`"
        );
    }
}

/// Legacy RA artifacts and rules that must NOT reappear in the full SKILL.
const FORBIDDEN_FULL_SKILL_MARKERS: &[&str] = &[
    // Front-side precursor artifacts RA must no longer generate.
    "RAGeneratePlan",
    "RA GeneratePlan",
    // Mechanical derivation gates removed from the new contract.
    "RA-G01",
    // Fixed implementation-perspective seven-element mining (I1-I7).
    "I1-I7",
    // Derived rule R/R' business-pattern mining.
    "R/R'",
    // Six business-pattern catalog (账号状态变更/订单状态变更/...).
    "六类业务模式",
    // DR handoff package sidecar.
    "DR 交接",
    "DR handoff",
    // Downstream correlation matrix section removed from RA.
    "RA ↔ DR/Story/Task 关联矩阵",
    // Fixed three-round no-new-findings loop.
    "三轮无新增",
    // Per-step user confirmation cadence removed; only one final approval.
    "每步用户确认",
];

#[test]
fn full_skill_declares_single_intent_ra_output() {
    let skill = ra_full_skill();
    assert!(
        skill.contains("intent RA") || skill.contains("intent=RA"),
        "full SKILL must declare the single `intent RA` output"
    );
}

#[test]
fn full_skill_does_not_require_front_side_prd_or_issue() {
    let skill = ra_full_skill();
    // RA must not REQUIRE a front-side PRD/Issue as a precursor artifact. It is
    // fine to name PRD/Issue as optional inputs or to deprecate generating them;
    // it is not fine to make them a mandatory precursor step.
    for line in skill.lines() {
        let requires = ["必须生成", "应当生成", "先生成", "步骤", "强制生成"]
            .iter()
            .any(|phrase| line.contains(phrase));
        if requires && (line.contains("PRD") || line.contains("Issue")) {
            panic!("full SKILL must not require a front-side PRD/Issue precursor: `{line}`");
        }
    }
}

/// A legacy marker only counts as a violation when it appears as an affirmative
/// requirement, not when it is named inside an explicit deprecation (❌ / 不 /
/// 禁止 / 删除 / removed). This keeps the contract honest: RA may name what it
/// retires, but must not re-require it.
fn is_affirmative_requirement(line: &str) -> bool {
    let deprecation = [
        "❌",
        "不生成",
        "不做",
        "不再",
        "禁止",
        "删除",
        "removed",
        "不使用",
        "不得",
    ];
    if deprecation.iter().any(|signal| line.contains(signal)) {
        return false;
    }
    ["必须", "应当", "需要", "要求", "强制", "步骤"]
        .iter()
        .any(|phrase| line.contains(phrase))
}

#[test]
fn full_skill_does_not_re_require_legacy_ra_artifacts_and_fixed_loops() {
    let skill = ra_full_skill();
    let violations: Vec<&str> = FORBIDDEN_FULL_SKILL_MARKERS
        .iter()
        .copied()
        .filter(|marker| {
            skill
                .lines()
                .any(|line| line.contains(marker) && is_affirmative_requirement(line))
        })
        .collect();
    assert!(
        violations.is_empty(),
        "full SKILL re-requires legacy RA markers as affirmative steps: {violations:?}"
    );
}

#[test]
fn full_skill_states_context_is_globally_optional() {
    let skill = ra_full_skill();
    // Context (project assets, code, history RA, protocol, logs, evidence) is
    // globally optional; it becomes required only when RA claims a system fact.
    assert!(
        skill.contains("全局选填") || skill.contains("选填"),
        "full SKILL must state context is globally optional"
    );
}

#[test]
fn full_skill_states_single_final_approval_held_by_daemon_receipt() {
    let skill = ra_full_skill();
    // Only one final SRS + scale approval; it is held by the daemon receipt,
    // never written back into the approved SRS content.
    assert!(
        skill.contains("一次") && (skill.contains("批准") || skill.contains("approval")),
        "full SKILL must require exactly one final approval"
    );
}

#[test]
fn full_skill_documents_seven_applicability_dimensions() {
    let skill = ra_full_skill();
    for key in [
        "participants",
        "scenarios",
        "state_lifecycle",
        "data_semantics",
        "external_contracts",
        "quality_security_compliance",
        "compatibility_migration_operations",
    ] {
        assert!(
            skill.contains(key),
            "full SKILL must document applicability dimension `{key}`"
        );
    }
}

#[test]
fn full_skill_documents_pure_need_six_dimension_scale_taking_max() {
    let skill = ra_full_skill();
    assert!(
        skill.contains("六维") || skill.contains("6 维") || skill.contains("6维"),
        "full SKILL must document the six pure-need scale dimensions"
    );
    assert!(
        skill.contains("最高分"),
        "full SKILL must state the scale takes the highest dimension score"
    );
}

#[test]
fn template_declares_v2_schema() {
    let template = ra_template();
    assert!(
        template.contains("ae-sdd-ra-srs/v2"),
        "template must declare the ae-sdd-ra-srs/v2 schema"
    );
}

#[test]
fn template_carries_core_sections_and_id_contracts() {
    let template = ra_template();
    for marker in [
        "## 0. 文档与需求身份",
        "## 1. 问题、目标与非目标",
        "## 2. 范围",
        "## 3. 适用性判定",
        "## 4. 需求清单",
        "## 5. 验收与追溯",
        "## 6. 约束、假设、冲突、风险与未决",
        "## 7. 规模裁定",
    ] {
        assert!(
            template.contains(marker),
            "template must contain core section `{marker}`"
        );
    }
    // Normative id contracts.
    assert!(template.contains("REQ-"), "template must define REQ-* ids");
    assert!(template.contains("AC-"), "template must define AC-* ids");
    assert!(template.contains("REF-"), "template must define REF-* ids");
    assert!(template.contains("GAP-"), "template must define GAP-* ids");
}

#[test]
fn template_carries_seven_applicability_keys() {
    let template = ra_template();
    for key in [
        "participants",
        "scenarios",
        "state_lifecycle",
        "data_semantics",
        "external_contracts",
        "quality_security_compliance",
        "compatibility_migration_operations",
    ] {
        assert!(
            template.contains(key),
            "template applicability table must include key `{key}`"
        );
    }
}

#[test]
fn template_carries_pure_need_six_dimension_scale_rubric() {
    let template = ra_template();
    assert!(
        template.contains("可观察行为与场景广度")
            && template.contains("参与方、权限或业务域广度")
            && template.contains("状态、数据语义与不变量复杂度")
            && template.contains("外部契约与协调范围")
            && template.contains("性能、安全、合规、可用性等质量风险")
            && template.contains("兼容、迁移、回滚和运行影响"),
        "template §7 must carry the six pure-need scale dimensions"
    );
}

#[test]
fn template_does_not_back_write_approval_into_srs_content() {
    let template = ra_template();
    // The template must only record analysisState draft/complete; user approval
    // is an external daemon receipt, never a back-written field in the SRS body.
    assert!(
        !template.contains("approved") && !template.contains("approval_ref"),
        "template must not back-write an approval field into the SRS content"
    );
    assert!(
        template.contains("draft") && template.contains("complete"),
        "template must record analysisState as draft/complete only"
    );
}

#[test]
fn template_does_not_require_fixed_charts_or_empty_not_applicable_sections() {
    let template = ra_template();
    // Fixed mandatory mindmap/state/sequence/ER charts and empty N/A fill are
    // removed; charts are conditional and not_applicable leaves only a §3 verdict.
    assert!(
        !template.contains("强制") || !template.contains("mindmap"),
        "template must not make a fixed mindmap mandatory"
    );
}

#[test]
fn methodology_consumers_do_not_restore_legacy_ra_rules() {
    let review_loop =
        source_asset("source/skill-fallbacks/skills/cross-cutting/review-loop-skill.full.md");
    assert!(review_loop.contains("blocking gap"));
    assert!(review_loop.contains("用户补充"));
    assert!(review_loop.contains("Gate finding"));

    let self_audit = source_asset(
        "source/skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md",
    );
    for marker in [
        "唯一 SRS",
        "SRS Core",
        "适用性闭合",
        "REQ",
        "AC",
        "六维评分",
    ] {
        assert!(self_audit.contains(marker), "self-audit misses `{marker}`");
    }

    let update =
        source_asset("source/skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md");
    for marker in ["ra-authenticity-scan", "8 类禁止规则"] {
        assert!(
            !update.contains(marker),
            "update health checks restore legacy RA contract marker `{marker}`"
        );
    }
}

#[test]
fn dr_consumer_reads_srs_v2_contract_instead_of_legacy_ra_sections() {
    let dr = source_asset("source/skill-fallbacks/skills/phase1-design/dr-generate-skill.full.md");
    for required in ["§4 REQ", "§5 AC", "§8.1", "§8.7", "RA 不预选方案"] {
        assert!(
            dr.contains(required),
            "DR consumer misses v2 mapping `{required}`"
        );
    }
    for forbidden in ["RA 8 维度", "RA §11", "RA §7 设计方向", "RA §6.2"] {
        assert!(
            !dr.contains(forbidden),
            "DR consumer still requires legacy mapping `{forbidden}`"
        );
    }
}

#[test]
fn design_docs_describe_authoritative_srs_v2_and_ra_first_route_binding() {
    let design = source_asset("source/docs/ae-sdd-design.md");
    let implementation = source_asset("source/docs/ae-sdd-implementation-architecture.md");
    for (name, document) in [("design", &design), ("implementation", &implementation)] {
        for forbidden in [
            "_resolve_selected_ra()",
            "state.raDocPath",
            "latest formal",
            "G-RA-5 机械派生",
            "G-RA-6 实现视角",
        ] {
            assert!(
                !document.contains(forbidden),
                "{name} doc restores legacy RA authority marker `{forbidden}`"
            );
        }
        assert!(document.contains("documentPaths/RA"));
        assert!(document.contains("RouteBinding"));
    }

    let daemon = source_asset("source/docs/ae-sdd-daemon-design.md");
    for forbidden in [
        "紧凑 RA 模板",
        "推荐设计路线及理由",
        "单文件内几行局部改动",
        "不超过 3 个文件",
    ] {
        assert!(
            !daemon.contains(forbidden),
            "daemon design restores legacy RA scale/output marker `{forbidden}`"
        );
    }
    assert!(daemon.contains("Initialized -> RequirementAnalyzed -> RouteSelected"));
    assert!(daemon.contains("六维"));
    assert!(daemon.contains("最高分"));
}
