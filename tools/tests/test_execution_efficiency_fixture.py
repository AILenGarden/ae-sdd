"""
test_execution_efficiency_fixture.py — golden execution fixture 校验

固定 PRD-AE-SDD-EXECUTION-EFFICIENCY-001 / STORY-AE-SDD-EXECUTION-CAPSULE-001
（plan P0 Task 1）的 approved-resume 基线：approved plan digest、四个必需
context refs、三个有序 slice（contract -> runtime-wiring -> process-test）、
focused verification ID 与 versioned 基线指标。标准库 only（unittest）。
"""
import hashlib
import json
import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIR = REPO_ROOT / "tests" / "fixtures" / "execution-efficiency"
RESUME_DIR = FIXTURE_DIR / "approved-resume"

STATE_PATH = RESUME_DIR / "state.json"
QUEUE_PATH = RESUME_DIR / "queue.json"
CONTEXT_PATH = RESUME_DIR / "context.json"
BASELINE_PATH = FIXTURE_DIR / "baseline.v1.json"
ALL_FIXTURES = (STATE_PATH, QUEUE_PATH, CONTEXT_PATH, BASELINE_PATH)

SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
VERIFICATION_ID_RE = re.compile(r"^V-EFF-[0-9]+[a-z]?$")
ABSOLUTE_PATH_RE = re.compile(r"^([A-Za-z]:[/\\]|/|\\\\)")

# capsule contract 的四个必需 context refs（plan §4.2 / Story 主流程 step 2）
REQUIRED_CONTEXT_REFS = (
    "storyRef",
    "constraintsRef",
    "thinkingEngineRef",
    "verificationRef",
)

# plan P0 Task 1 Step 3：三切片顺序固定
EXPECTED_SLICE_KINDS = ["contract", "runtime-wiring", "process-test"]

# plan §4.2 默认预算
EXPECTED_BUDGETS = {
    "maxCapsuleBytes": 16384,
    "maxToolOutputBytes": 65536,
    "maxSourceReadBytesPerBatch": 24576,
    "maxSourceFilesPerBatch": 12,
    "inspectionCallsPerBatch": 4,
    "maxNoProgressBatches": 3,
    "maxAuthorityRefreshesPerResume": 1,
}

# plan §5 P0 门槛指标集合
EXPECTED_BASELINE_METRICS = {
    "resumeToFirstPatchMs",
    "fullCapsuleBytes",
    "noChangeResponseBytes",
    "authorityRefreshesPerResume",
    "maxConsecutiveNoProgressBatches",
    "broadTestsBeforeFocusedGreen",
    "inPlanReapprovalRate",
    "gateFreshCacheHitRate",
    "repeatedSourceReadBytesReduction",
    "goldenTraceTokenReduction",
    "requestBodyTimeouts",
}


def load_fixture(path):
    """加载 fixture JSON；文件缺失时给出明确失败（RED 入口）。"""
    if not path.is_file():
        raise AssertionError(f"fixture missing: {path.relative_to(REPO_ROOT)}")
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_sha256(payload):
    """canonical JSON（排序键、紧凑分隔符）的 sha256，用于 locator/digest 绑定。"""
    blob = json.dumps(
        payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    )
    return "sha256:" + hashlib.sha256(blob.encode("utf-8")).hexdigest()


class TestFixturePresence(unittest.TestCase):
    def test_all_fixture_files_exist(self):
        for path in ALL_FIXTURES:
            with self.subTest(path=path.name):
                self.assertTrue(
                    path.is_file(),
                    f"missing fixture: {path.relative_to(REPO_ROOT)}",
                )


class TestFixtureMetadata(unittest.TestCase):
    """testing.md §四：稳定 ID、schema version、owner、来源与分类。"""

    def test_metadata_fields_present(self):
        for path in ALL_FIXTURES:
            data = load_fixture(path)
            with self.subTest(path=path.name):
                for key in ("fixtureId", "schemaVersion", "owner", "source"):
                    self.assertIsInstance(data.get(key), str)
                    self.assertTrue(data[key], key)
                self.assertEqual(data["classification"], "preserve")


class TestApprovedPlanDigest(unittest.TestCase):
    def setUp(self):
        self.state = load_fixture(STATE_PATH)
        self.queue = load_fixture(QUEUE_PATH)
        self.context = load_fixture(CONTEXT_PATH)

    def test_plan_is_user_approved(self):
        self.assertEqual(self.state["executionPlan"]["status"], "approved")

    def test_plan_digest_well_formed(self):
        digest = self.state["executionPlan"]["planDigest"]
        self.assertRegex(digest, SHA256_RE)

    def test_plan_digest_bound_across_fixtures(self):
        digest = self.state["executionPlan"]["planDigest"]
        self.assertEqual(self.queue["approvedPlanDigest"], digest)
        self.assertEqual(self.context["approvedPlanDigest"], digest)


class TestRequiredContextRefs(unittest.TestCase):
    def setUp(self):
        self.refs = load_fixture(CONTEXT_PATH)["requiredContextRefs"]

    def test_exactly_four_required_refs(self):
        self.assertEqual(set(self.refs.keys()), set(REQUIRED_CONTEXT_REFS))

    def test_each_ref_has_relative_artifact_and_digest(self):
        for name in REQUIRED_CONTEXT_REFS:
            with self.subTest(ref=name):
                ref = self.refs[name]
                self.assertIsInstance(ref["artifact"], str)
                self.assertTrue(ref["artifact"])
                self.assertIsNone(
                    ABSOLUTE_PATH_RE.match(ref["artifact"]),
                    "context ref artifact 必须是 project-relative",
                )
                self.assertRegex(ref["digest"], SHA256_RE)


class TestOrderedSlices(unittest.TestCase):
    def setUp(self):
        self.queue = load_fixture(QUEUE_PATH)
        self.slices = self.queue["slices"]

    def test_exactly_three_slices(self):
        self.assertEqual(len(self.slices), 3)
        self.assertEqual(self.queue["totalSlices"], 3)

    def test_ordinals_strictly_ordered_from_one(self):
        self.assertEqual([s["ordinal"] for s in self.slices], [1, 2, 3])

    def test_slice_kinds_in_contract_wiring_process_order(self):
        self.assertEqual(
            [s["kind"] for s in self.slices], EXPECTED_SLICE_KINDS
        )

    def test_slice_ids_unique_and_dependency_chain(self):
        ids = [s["sliceId"] for s in self.slices]
        self.assertEqual(len(set(ids)), 3)
        self.assertEqual(self.slices[0]["dependsOn"], [])
        self.assertEqual(self.slices[1]["dependsOn"], [ids[0]])
        self.assertEqual(self.slices[2]["dependsOn"], [ids[1]])

    def test_queue_starts_pending_on_first_slice(self):
        self.assertEqual(self.queue["activeOrdinal"], 1)
        self.assertEqual(self.queue["completedSlices"], 0)


class TestFocusedVerificationIds(unittest.TestCase):
    def setUp(self):
        self.slices = load_fixture(QUEUE_PATH)["slices"]

    def test_each_slice_has_focused_verification_id(self):
        for slc in self.slices:
            with self.subTest(slice=slc["sliceId"]):
                self.assertRegex(
                    slc["focusedVerificationId"], VERIFICATION_ID_RE
                )

    def test_broad_verification_ids_well_formed(self):
        for slc in self.slices:
            with self.subTest(slice=slc["sliceId"]):
                broad = slc["broadVerificationIds"]
                self.assertIsInstance(broad, list)
                for vid in broad:
                    self.assertRegex(vid, VERIFICATION_ID_RE)
                self.assertNotIn(slc["focusedVerificationId"], broad)


class TestExecutionBudgets(unittest.TestCase):
    def test_default_budgets_match_plan(self):
        budgets = load_fixture(QUEUE_PATH)["budgets"]
        self.assertEqual(budgets, EXPECTED_BUDGETS)


class TestBaselineMetrics(unittest.TestCase):
    def setUp(self):
        self.baseline = load_fixture(BASELINE_PATH)

    def test_baseline_is_versioned(self):
        self.assertEqual(
            self.baseline["schemaVersion"],
            "ae-sdd-execution-efficiency-baseline/v1",
        )

    def test_metric_set_complete(self):
        self.assertEqual(
            set(self.baseline["metrics"].keys()), EXPECTED_BASELINE_METRICS
        )

    def test_each_metric_has_threshold_direction_unit(self):
        for name, metric in self.baseline["metrics"].items():
            with self.subTest(metric=name):
                self.assertIsInstance(metric["threshold"], (int, float))
                self.assertIn(metric["direction"], ("lte", "gte", "eq"))
                self.assertIsInstance(metric["unit"], str)
                self.assertTrue(metric["unit"])


class TestStateRuntimeBinding(unittest.TestCase):
    """state 只保存 locator/digest/cursor（plan §4.1），并与 queue 绑定。"""

    def setUp(self):
        self.state = load_fixture(STATE_PATH)
        self.queue = load_fixture(QUEUE_PATH)
        self.runtime = self.state["executionRuntime"]

    def test_queue_digest_binds_canonical_queue(self):
        self.assertEqual(
            self.runtime["queueDigest"], canonical_sha256(self.queue)
        )

    def test_active_slice_ordinal_matches_queue(self):
        self.assertEqual(
            self.runtime["activeSliceOrdinal"], self.queue["activeOrdinal"]
        )

    def test_locators_project_relative_and_digests_well_formed(self):
        for key in ("capsuleRef", "queueRef", "ledgerRef"):
            self.assertTrue(
                self.runtime[key].startswith(".auto-engineering/"), key
            )
        for key in ("capsuleDigest", "ledgerDigest"):
            self.assertRegex(self.runtime[key], SHA256_RE)


class TestFixtureStaysMinimal(unittest.TestCase):
    """不得复制真实项目大 state：fixture 保持小而合成。"""

    def test_fixture_files_stay_small(self):
        for path in ALL_FIXTURES:
            with self.subTest(path=path.name):
                self.assertLess(path.stat().st_size, 8 * 1024)


if __name__ == "__main__":
    unittest.main(verbosity=2)
