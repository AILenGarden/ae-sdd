use ae_sdd_scanners::ScannerId;

pub const GATE_COUNT: usize = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateSeverity {
    Blocker,
}

/// Stable predicate identity supplied by an artifact/state adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PredicateKey(&'static str);

impl PredicateKey {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Every Gate is backed by a native predicate or one in-process scanner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeGateRule {
    Predicate(PredicateKey),
    Scanner(ScannerId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub severity: GateSeverity,
    pub scope: &'static str,
    pub pass_condition: &'static str,
    pub failure_action: &'static str,
    pub rule: NativeGateRule,
}

/// Stable class of authoritative Gate inputs. A change to one input class
/// invalidates exactly the Gates that declare the matching selector, so an
/// evidence, review or source change never re-runs unrelated Gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateInputSelector {
    ProjectAssets,
    Story,
    Constraints,
    ThinkingEngine,
    ExecutionPlan,
    ChangedPaths,
    VerificationPlan,
    EvidenceLedger,
    ReviewBatch,
    Toolchain,
    Inventory,
    /// Binds only the current Work Item's bound RA document, its validated
    /// receipt, and the single file at `/documentPaths/RA`. Replaces the broad
    /// `ProjectAssets` scope used by legacy RA gates so foreign project assets
    /// cannot make an RA gate stale.
    RequirementAnalysis,
    /// Binds the route candidate, approval receipt, frozen EngineeringRoute
    /// evidence, open route-blocking conflicts, and scale evidence — used by
    /// `G-RA-FLOW-VIOLATION` at the `RouteSelected` boundary.
    RouteBinding,
}

/// Declarative incremental dependencies of one Gate: prerequisite Gates that
/// order evaluation and propagate invalidation to dependents, and the input
/// selectors whose change forces re-evaluation. A Gate without selectors
/// cannot prove freshness and fails closed to re-evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateDependencySpec {
    pub gate: &'static str,
    pub prerequisites: &'static [&'static str],
    pub selectors: &'static [GateInputSelector],
}

const fn predicate(value: &'static str) -> NativeGateRule {
    NativeGateRule::Predicate(PredicateKey::new(value))
}

const GATES: [GateSpec; GATE_COUNT] = [
    GateSpec {
        id: "G-00",
        name: "项目资产完整性",
        severity: GateSeverity::Blocker,
        scope: "entry",
        pass_condition: "project assets exist and 7-layer index is complete",
        failure_action: "BLOCK -> project-assets-update",
        rule: predicate("project.assets.complete"),
    },
    GateSpec {
        id: "G-01",
        name: "DR 文档存在",
        severity: GateSeverity::Blocker,
        scope: "before Story/Task generation",
        pass_condition: "DR document exists (large route)",
        failure_action: "BLOCK",
        rule: predicate("document.dr.exists"),
    },
    GateSpec {
        id: "G-02",
        name: "Story 文档存在",
        severity: GateSeverity::Blocker,
        scope: "before TestCase/Coding generation",
        pass_condition: "Story document exists",
        failure_action: "BLOCK",
        rule: predicate("document.story.exists"),
    },
    GateSpec {
        id: "G-03",
        name: "Story Review 通过",
        severity: GateSeverity::Blocker,
        scope: "before TestCase generation",
        pass_condition: "Story review loop exited normally",
        failure_action: "BLOCK -> re-review",
        rule: predicate("review.story.passed"),
    },
    GateSpec {
        id: "G-04",
        name: "TestCase 文档存在",
        severity: GateSeverity::Blocker,
        scope: "before Coding generation",
        pass_condition: "TestCase document exists",
        failure_action: "BLOCK",
        rule: predicate("document.testcase.exists"),
    },
    GateSpec {
        id: "G-05",
        name: "Task 文档存在",
        severity: GateSeverity::Blocker,
        scope: "before Coding execute (legacy)",
        pass_condition: "Task document exists (v3.10 skeleton merged into coding-process)",
        failure_action: "BLOCK",
        rule: predicate("document.task.exists"),
    },
    GateSpec {
        id: "G-06",
        name: "Task Review 通过",
        severity: GateSeverity::Blocker,
        scope: "before Coding execute (legacy)",
        pass_condition: "Task review passed (v3.10 merged into coding-process)",
        failure_action: "BLOCK",
        rule: predicate("review.task.passed"),
    },
    GateSpec {
        id: "G-07",
        name: "CodingPlan 存在",
        severity: GateSeverity::Blocker,
        scope: "before coding execute",
        pass_condition: "CodingPlan document exists",
        failure_action: "BLOCK",
        rule: predicate("coding_plan.exists"),
    },
    GateSpec {
        id: "G-08",
        name: "CodingPlan 14 门禁通过",
        severity: GateSeverity::Blocker,
        scope: "before coding execute",
        pass_condition: "Approved CodingPlan has complete goal/paths/risks/source-reads and a verification row for every Story AC",
        failure_action: "BLOCK",
        rule: predicate("coding_plan.fourteen_gates.complete"),
    },
    GateSpec {
        id: "G-HTTP-1",
        name: "HTTP 场景推导有效",
        severity: GateSeverity::Blocker,
        scope: "before coding execute",
        pass_condition: "HTTP AC has a derived, repeatable, independently observed scenario manifest",
        failure_action: "BLOCK",
        rule: predicate("http.scenario_manifest.valid"),
    },
    GateSpec {
        id: "G-09",
        name: "测试真实性扫描通过",
        severity: GateSeverity::Blocker,
        scope: "test review",
        pass_condition: "test authenticity scanner passes",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::TestAuthenticity),
    },
    GateSpec {
        id: "G-10",
        name: "测试报告存在",
        severity: GateSeverity::Blocker,
        scope: "after test run",
        pass_condition: "test report document exists",
        failure_action: "BLOCK -> run test",
        rule: predicate("test.evidence.exists"),
    },
    GateSpec {
        id: "G-11",
        name: "Coding 报告存在",
        severity: GateSeverity::Blocker,
        scope: "after coding",
        pass_condition: "coding report document exists",
        failure_action: "BLOCK",
        rule: predicate("coding.result.exists"),
    },
    GateSpec {
        id: "G-12",
        name: "CodeReview 报告存在",
        severity: GateSeverity::Blocker,
        scope: "after code review",
        pass_condition: "CodeReview report document exists",
        failure_action: "BLOCK",
        rule: predicate("review.findings.recorded"),
    },
    GateSpec {
        id: "G-13",
        name: "全链路对称性核查通过",
        severity: GateSeverity::Blocker,
        scope: "delivery check",
        pass_condition: "full-chain symmetry check passes",
        failure_action: "BLOCK -> fix gaps",
        rule: predicate("traceability.full_chain.symmetric"),
    },
    GateSpec {
        id: "G-14",
        name: "CodingPlan-Story 一致性",
        severity: GateSeverity::Blocker,
        scope: "before coding execute",
        pass_condition: "CodingPlan references Story and aligns with AC",
        failure_action: "BLOCK",
        rule: predicate("coding_plan.story.aligned"),
    },
    GateSpec {
        id: "G-CODEPLAN-SRC",
        name: "CodingPlan 源码核对",
        severity: GateSeverity::Blocker,
        scope: "before coding execute",
        pass_condition: "CodingPlan class skeleton has source-read evidence",
        failure_action: "BLOCK",
        rule: predicate("coding_plan.source_trace.complete"),
    },
    GateSpec {
        id: "G-DOC-STORAGE",
        name: "文档落地存放合规",
        severity: GateSeverity::Blocker,
        scope: "before doc write",
        pass_condition: "path/name resolved by document-storage",
        failure_action: "BLOCK",
        rule: predicate("document.storage.compliant"),
    },
    GateSpec {
        id: "G-PATH",
        name: "路径越界检测",
        severity: GateSeverity::Blocker,
        scope: "build/update check",
        pass_condition: "source docs do not hardcode output paths",
        failure_action: "BLOCK",
        rule: predicate("source.output_paths.compliant"),
    },
    GateSpec {
        id: "G-RA-1",
        name: "RA 唯一 SRS + receipt 绑定",
        severity: GateSeverity::Blocker,
        scope: "RequirementAnalyzed",
        pass_condition: "single ae-sdd-ra-srs/v2 SRS bound with verified RA receipt",
        failure_action: "BLOCK",
        rule: predicate("ra.srs.bound"),
    },
    GateSpec {
        id: "G-RA-2",
        name: "RA SRS Core 完整",
        severity: GateSeverity::Blocker,
        scope: "RequirementAnalyzed",
        pass_condition: "SRS core (schema, sections, ids) is complete and unique",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::RaCore),
    },
    GateSpec {
        id: "G-RA-3",
        name: "RA 适用性与条件章节一致",
        severity: GateSeverity::Blocker,
        scope: "RequirementAnalyzed",
        pass_condition: "seven applicability dimensions judged and consistent",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::RaApplicability),
    },
    GateSpec {
        id: "G-RA-4",
        name: "RA 需求可追溯可验收并闭合",
        severity: GateSeverity::Blocker,
        scope: "RequirementAnalyzed",
        pass_condition: "REQ traceable/acceptable, no blocking gap, scale consistent",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::RaClosure),
    },
    GateSpec {
        id: "G-RA-FLOW-VIOLATION",
        name: "RA -> Route 绑定门禁",
        severity: GateSeverity::Blocker,
        scope: "RouteSelected",
        pass_condition: "RA-first order, approval, receipt/digest/scale/route binding verified",
        failure_action: "BLOCK",
        rule: predicate("ra.route.binding"),
    },
    GateSpec {
        id: "G-RA-5",
        name: "RA 适用性检查兼容入口（别名 -> G-RA-3）",
        severity: GateSeverity::Blocker,
        scope: "compatibility-only",
        pass_condition: "returns the real G-RA-3 applicability diagnosis",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::RaApplicability),
    },
    GateSpec {
        id: "G-RA-6",
        name: "RA closure 检查兼容入口（别名 -> G-RA-4）",
        severity: GateSeverity::Blocker,
        scope: "compatibility-only",
        pass_condition: "returns the real G-RA-4 closure diagnosis",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::RaClosure),
    },
    GateSpec {
        id: "G-CODE-1",
        name: "Coding 真实性扫描通过",
        severity: GateSeverity::Blocker,
        scope: "coding/code review",
        pass_condition: "coding authenticity scanner passes",
        failure_action: "BLOCK",
        rule: NativeGateRule::Scanner(ScannerId::CodingAuthenticity),
    },
    GateSpec {
        id: "G-DOC-CONSISTENCY",
        name: "项目侧记忆-配置路径一致性",
        severity: GateSeverity::Blocker,
        scope: "entry/doc workspace check",
        pass_condition: "project memory path agrees with config",
        failure_action: "BLOCK",
        rule: predicate("memory.configuration_path.consistent"),
    },
    GateSpec {
        id: "G-REVIEW-LOOP",
        name: "review-loop 退出条件通过",
        severity: GateSeverity::Blocker,
        scope: "review phase transition",
        pass_condition: "review-loop exit condition is satisfied",
        failure_action: "BLOCK",
        rule: predicate("review.loop.exit_satisfied"),
    },
    GateSpec {
        id: "G-09B",
        name: "reviewer 独立性通过（多 reviewer 机械强制）",
        severity: GateSeverity::Blocker,
        scope: "review phase transition",
        pass_condition: "reviewer independence requirement passes",
        failure_action: "BLOCK",
        rule: predicate("review.independence.valid"),
    },
    GateSpec {
        id: "G-REVIEW-DEPTH",
        name: "Review 深度（禁裸勾 + 零发现举证）",
        severity: GateSeverity::Blocker,
        scope: "review phase transition",
        pass_condition: "review report has depth evidence (no bare check, zero-finding justified)",
        failure_action: "BLOCK -> add evidence or findings",
        rule: predicate("review.depth.valid"),
    },
    GateSpec {
        id: "G-AUTO-CONSENSUS",
        name: "自动化联审共识通过",
        severity: GateSeverity::Blocker,
        scope: "automation mode review transition",
        pass_condition: "state.reviewConsensus[point].passed=true + reviewer independence",
        failure_action: "BLOCK (non-automation or off-whitelist -> skipped)",
        rule: predicate("review.automation_consensus.valid_or_exempt"),
    },
    GateSpec {
        id: "G-DR-CTX",
        name: "DR 上下文加载",
        severity: GateSeverity::Blocker,
        scope: "before DR generation",
        pass_condition: "required DR contexts loaded (PRD/assets/constraints/standards)",
        failure_action: "BLOCK -> load CONTEXT_GATE_REGISTRY[G-DR-CTX].required",
        rule: predicate("context.dr.complete"),
    },
    GateSpec {
        id: "G-STORY-CTX",
        name: "Story 上下文加载",
        severity: GateSeverity::Blocker,
        scope: "before Story generation",
        pass_condition: "required Story contexts loaded (constraints/assets/DR/PRD/dependsStory/sourceTrace/standardsRef/outputBoundary)",
        failure_action: "BLOCK -> load CONTEXT_GATE_REGISTRY[G-STORY-CTX].required",
        rule: predicate("context.story.complete"),
    },
    GateSpec {
        id: "G-TESTCASE-CTX",
        name: "TestCase 上下文加载",
        severity: GateSeverity::Blocker,
        scope: "before TestCase generation",
        pass_condition: "required TestCase contexts loaded",
        failure_action: "BLOCK -> load CONTEXT_GATE_REGISTRY[G-TESTCASE-CTX].required",
        rule: predicate("context.testcase.complete"),
    },
    GateSpec {
        id: "G-TASK-CTX",
        name: "Task 上下文加载",
        severity: GateSeverity::Blocker,
        scope: "before Task generation (legacy)",
        pass_condition: "required Task contexts loaded (v3.10 merged into coding-process)",
        failure_action: "BLOCK -> load CONTEXT_GATE_REGISTRY[G-TASK-CTX].required",
        rule: predicate("context.task.complete"),
    },
];

use GateInputSelector::{
    ChangedPaths, Constraints, EvidenceLedger, ExecutionPlan, Inventory, ProjectAssets,
    RequirementAnalysis, ReviewBatch, RouteBinding, Story, ThinkingEngine, VerificationPlan,
};

const GATE_DEPENDENCIES: [GateDependencySpec; GATE_COUNT] = [
    GateDependencySpec {
        gate: "G-00",
        prerequisites: &[],
        selectors: &[ProjectAssets, Inventory],
    },
    GateDependencySpec {
        gate: "G-01",
        prerequisites: &["G-RA-1"],
        selectors: &[ProjectAssets],
    },
    GateDependencySpec {
        gate: "G-02",
        prerequisites: &["G-01"],
        selectors: &[Story],
    },
    GateDependencySpec {
        gate: "G-03",
        prerequisites: &["G-02"],
        selectors: &[Story],
    },
    GateDependencySpec {
        gate: "G-04",
        prerequisites: &["G-03"],
        selectors: &[Story, VerificationPlan],
    },
    GateDependencySpec {
        gate: "G-05",
        prerequisites: &["G-04"],
        selectors: &[Story],
    },
    GateDependencySpec {
        gate: "G-06",
        prerequisites: &["G-05"],
        selectors: &[Story],
    },
    GateDependencySpec {
        gate: "G-07",
        prerequisites: &["G-03"],
        selectors: &[Story, ThinkingEngine],
    },
    GateDependencySpec {
        gate: "G-08",
        prerequisites: &["G-07"],
        selectors: &[ExecutionPlan, Constraints],
    },
    GateDependencySpec {
        gate: "G-HTTP-1",
        prerequisites: &["G-07"],
        selectors: &[Story, VerificationPlan],
    },
    GateDependencySpec {
        gate: "G-09",
        prerequisites: &["G-04"],
        selectors: &[ChangedPaths],
    },
    GateDependencySpec {
        gate: "G-10",
        prerequisites: &["G-09"],
        selectors: &[EvidenceLedger],
    },
    GateDependencySpec {
        gate: "G-11",
        prerequisites: &["G-08", "G-14", "G-CODEPLAN-SRC"],
        selectors: &[ChangedPaths],
    },
    GateDependencySpec {
        gate: "G-12",
        prerequisites: &["G-10", "G-11"],
        selectors: &[ReviewBatch],
    },
    GateDependencySpec {
        gate: "G-13",
        prerequisites: &["G-12"],
        selectors: &[ProjectAssets, ReviewBatch],
    },
    GateDependencySpec {
        gate: "G-14",
        prerequisites: &["G-07"],
        selectors: &[Story, ExecutionPlan],
    },
    GateDependencySpec {
        gate: "G-CODEPLAN-SRC",
        prerequisites: &["G-07"],
        selectors: &[ExecutionPlan, ChangedPaths],
    },
    GateDependencySpec {
        gate: "G-DOC-STORAGE",
        prerequisites: &["G-00"],
        selectors: &[ProjectAssets],
    },
    GateDependencySpec {
        gate: "G-PATH",
        prerequisites: &["G-00"],
        selectors: &[ProjectAssets],
    },
    GateDependencySpec {
        gate: "G-RA-1",
        prerequisites: &[],
        selectors: &[RequirementAnalysis],
    },
    GateDependencySpec {
        gate: "G-RA-2",
        prerequisites: &["G-RA-1"],
        selectors: &[RequirementAnalysis],
    },
    GateDependencySpec {
        gate: "G-RA-3",
        prerequisites: &["G-RA-2"],
        selectors: &[RequirementAnalysis],
    },
    GateDependencySpec {
        gate: "G-RA-4",
        prerequisites: &["G-RA-3"],
        selectors: &[RequirementAnalysis],
    },
    GateDependencySpec {
        gate: "G-RA-FLOW-VIOLATION",
        prerequisites: &[],
        selectors: &[RequirementAnalysis, RouteBinding],
    },
    GateDependencySpec {
        gate: "G-RA-5",
        prerequisites: &[],
        selectors: &[RequirementAnalysis],
    },
    GateDependencySpec {
        gate: "G-RA-6",
        prerequisites: &[],
        selectors: &[RequirementAnalysis],
    },
    GateDependencySpec {
        gate: "G-CODE-1",
        prerequisites: &["G-11"],
        selectors: &[ChangedPaths],
    },
    GateDependencySpec {
        gate: "G-DOC-CONSISTENCY",
        prerequisites: &["G-00"],
        selectors: &[ProjectAssets],
    },
    GateDependencySpec {
        gate: "G-REVIEW-LOOP",
        prerequisites: &["G-12"],
        selectors: &[ReviewBatch],
    },
    GateDependencySpec {
        gate: "G-09B",
        prerequisites: &["G-12"],
        selectors: &[ReviewBatch],
    },
    GateDependencySpec {
        gate: "G-REVIEW-DEPTH",
        prerequisites: &["G-12"],
        selectors: &[ReviewBatch],
    },
    GateDependencySpec {
        gate: "G-AUTO-CONSENSUS",
        prerequisites: &["G-09B", "G-REVIEW-DEPTH"],
        selectors: &[ReviewBatch],
    },
    GateDependencySpec {
        gate: "G-DR-CTX",
        prerequisites: &["G-00"],
        selectors: &[ProjectAssets, Constraints, ThinkingEngine],
    },
    GateDependencySpec {
        gate: "G-STORY-CTX",
        prerequisites: &["G-01"],
        selectors: &[Story, Constraints, ProjectAssets],
    },
    GateDependencySpec {
        gate: "G-TESTCASE-CTX",
        prerequisites: &["G-03"],
        selectors: &[Story, Constraints],
    },
    GateDependencySpec {
        gate: "G-TASK-CTX",
        prerequisites: &["G-04"],
        selectors: &[Story, Constraints],
    },
];

pub struct GateRegistry;

impl GateRegistry {
    pub const fn all() -> &'static [GateSpec; GATE_COUNT] {
        &GATES
    }

    pub fn get(id: &str) -> Option<&'static GateSpec> {
        GATES.iter().find(|gate| gate.id == id)
    }

    /// Incremental dependency declarations for every registered Gate.
    pub const fn dependencies() -> &'static [GateDependencySpec; GATE_COUNT] {
        &GATE_DEPENDENCIES
    }

    /// Returns the incremental dependency declaration of one Gate.
    pub fn dependency_spec(id: &str) -> Option<&'static GateDependencySpec> {
        GATE_DEPENDENCIES.iter().find(|spec| spec.gate == id)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn registry_contains_exactly_36_unique_non_stub_rules() {
        assert_eq!(GateRegistry::all().len(), GATE_COUNT);
        assert_eq!(
            GateRegistry::all()
                .iter()
                .map(|gate| gate.id)
                .collect::<BTreeSet<_>>()
                .len(),
            GATE_COUNT
        );
        assert!(GateRegistry::all().iter().all(|gate| {
            !gate.scope.is_empty()
                && !gate.pass_condition.is_empty()
                && !gate.failure_action.is_empty()
        }));
    }
}
