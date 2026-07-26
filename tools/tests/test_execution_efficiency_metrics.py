"""
test_execution_efficiency_metrics.py — P0 性能门槛指标 pin

固定 PRD-AE-SDD-EXECUTION-EFFICIENCY-001（plan §5 / Task 13 Step 3）的 P0 门槛：
指标名集合、阈值、方向与单位必须逐一对齐 golden baseline fixture
（tests/fixtures/execution-efficiency/baseline.v1.json），预算派生合同
（4 次调查调用一批 x 连续 3 批 -> 第 13 次连续调查调用被拒）必须与 queue
fixture 一致，且指标载体不得携带 prompt、源码正文或 secret。标准库 only。
"""
import json
import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIR = REPO_ROOT / "tests" / "fixtures" / "execution-efficiency"
RESUME_DIR = FIXTURE_DIR / "approved-resume"

BASELINE_PATH = FIXTURE_DIR / "baseline.v1.json"
QUEUE_PATH = RESUME_DIR / "queue.json"
STATE_PATH = RESUME_DIR / "state.json"
CONTEXT_PATH = RESUME_DIR / "context.json"
ALL_FIXTURES = (STATE_PATH, QUEUE_PATH, CONTEXT_PATH, BASELINE_PATH)

# plan §5 P0 门槛：指标名 -> (threshold, direction, unit)
P0_GATES = {
    "resumeToFirstPatchMs": (300000, "lte", "ms"),
    "fullCapsuleBytes": (16384, "lte", "bytes"),
    "noChangeResponseBytes": (1024, "lte", "bytes"),
    "authorityRefreshesPerResume": (1, "lte", "count"),
    "maxConsecutiveNoProgressBatches": (3, "lte", "count"),
    "broadTestsBeforeFocusedGreen": (0, "eq", "count"),
}

# plan §4.2 默认预算：4 次调查调用一批，连续 3 批无进展后停止调查。
INSPECTION_CALLS_PER_BATCH = 4
MAX_NO_PROGRESS_BATCHES = 3
FIRST_DENIED_INVESTIGATION_CALL = (
    INSPECTION_CALLS_PER_BATCH * MAX_NO_PROGRESS_BATCHES + 1
)

# secret/凭据正文 denylist（值扫描；键名是 schema 字段，不扫描）。
SECRET_PATTERNS = (
    re.compile(r"BEGIN [A-Z ]*PRIVATE KEY"),
    re.compile(r"AKIA[0-9A-Z]{16}"),
    re.compile(r"ghp_[A-Za-z0-9]{20,}"),
    re.compile(r"github_pat_[A-Za-z0-9_]{20,}"),
    re.compile(r"sk-[A-Za-z0-9]{20,}"),
    re.compile(r"xox[baprs]-[A-Za-z0-9-]{10,}"),
    re.compile(r"(?i)bearer [A-Za-z0-9._~+/=-]{20,}"),
    re.compile(r"(?i)(password|passwd|api[_-]?key|secret|token)\s*[:=]\s*\S+"),
)

# 单个标量字符串的有界长度：指标/fixture 载体不允许塞入 prompt 或源码正文。
MAX_SCALAR_CHARS = 512


def load_fixture(path):
    if not path.is_file():
        raise AssertionError(f"fixture missing: {path.relative_to(REPO_ROOT)}")
    return json.loads(path.read_text(encoding="utf-8"))


def evaluate_metric(threshold, direction, value):
    """按 baseline direction 语义判定一个实测值是否过门槛。"""
    if direction == "lte":
        return value <= threshold
    if direction == "gte":
        return value >= threshold
    if direction == "eq":
        return value == threshold
    raise AssertionError(f"unknown metric direction: {direction}")


def iter_strings(node):
    """递归产出 JSON 文档中的全部字符串标量（键与值）。"""
    if isinstance(node, dict):
        for key, value in node.items():
            yield key
            yield from iter_strings(value)
    elif isinstance(node, list):
        for value in node:
            yield from iter_strings(value)
    elif isinstance(node, str):
        yield node


class TestP0GatePin(unittest.TestCase):
    """P0 门槛指标名/阈值/方向/单位与 plan §5 逐一冻结对齐。"""

    def setUp(self):
        self.metrics = load_fixture(BASELINE_PATH)["metrics"]

    def test_p0_gate_names_exact(self):
        for name in P0_GATES:
            with self.subTest(metric=name):
                self.assertIn(name, self.metrics)

    def test_p0_gate_thresholds_directions_units(self):
        for name, (threshold, direction, unit) in P0_GATES.items():
            with self.subTest(metric=name):
                metric = self.metrics[name]
                self.assertEqual(metric["threshold"], threshold, name)
                self.assertEqual(metric["direction"], direction, name)
                self.assertEqual(metric["unit"], unit, name)

    def test_capsule_and_no_change_thresholds_are_the_hard_limits(self):
        self.assertEqual(
            self.metrics["fullCapsuleBytes"]["threshold"], 16 * 1024
        )
        self.assertEqual(
            self.metrics["noChangeResponseBytes"]["threshold"], 1024
        )
        self.assertEqual(
            self.metrics["authorityRefreshesPerResume"]["threshold"], 1
        )
        self.assertEqual(
            self.metrics["broadTestsBeforeFocusedGreen"]["threshold"], 0
        )


class TestNoProgressBudgetContract(unittest.TestCase):
    """queue fixture 预算必须兑现“第 13 次连续调查调用被拒”。"""

    def test_queue_budgets_match_plan_defaults(self):
        budgets = load_fixture(QUEUE_PATH)["budgets"]
        self.assertEqual(
            budgets["inspectionCallsPerBatch"], INSPECTION_CALLS_PER_BATCH
        )
        self.assertEqual(
            budgets["maxNoProgressBatches"], MAX_NO_PROGRESS_BATCHES
        )

    def test_first_denied_call_is_the_thirteenth(self):
        self.assertEqual(FIRST_DENIED_INVESTIGATION_CALL, 13)

    def test_baseline_batch_gate_matches_queue_budget(self):
        baseline = load_fixture(BASELINE_PATH)["metrics"]
        queue = load_fixture(QUEUE_PATH)["budgets"]
        self.assertEqual(
            baseline["maxConsecutiveNoProgressBatches"]["threshold"],
            queue["maxNoProgressBatches"],
        )


class TestMetricEvaluationSemantics(unittest.TestCase):
    """direction 求值语义：过门槛与回归样本都必须被正确判定。"""

    def test_p0_representative_sample_passes_every_gate(self):
        sample = {
            "resumeToFirstPatchMs": 2500,
            "fullCapsuleBytes": 4096,
            "noChangeResponseBytes": 512,
            "authorityRefreshesPerResume": 1,
            "maxConsecutiveNoProgressBatches": 3,
            "broadTestsBeforeFocusedGreen": 0,
        }
        for name, (threshold, direction, _unit) in P0_GATES.items():
            with self.subTest(metric=name):
                self.assertTrue(
                    evaluate_metric(threshold, direction, sample[name]), name
                )

    def test_regressions_are_caught_per_gate(self):
        regressions = {
            "resumeToFirstPatchMs": 300001,
            "fullCapsuleBytes": 16385,
            "noChangeResponseBytes": 1025,
            "authorityRefreshesPerResume": 2,
            "maxConsecutiveNoProgressBatches": 4,
            "broadTestsBeforeFocusedGreen": 1,
        }
        for name, (threshold, direction, _unit) in P0_GATES.items():
            with self.subTest(metric=name):
                self.assertFalse(
                    evaluate_metric(threshold, direction, regressions[name]),
                    name,
                )


class TestMetricsCarryNoSensitiveContent(unittest.TestCase):
    """指标与 fixture 载体不得携带 prompt、源码正文或 secret。"""

    def test_no_secret_patterns_in_any_fixture_string(self):
        for path in ALL_FIXTURES:
            document = load_fixture(path)
            for scalar in iter_strings(document):
                with self.subTest(path=path.name, scalar=scalar[:32]):
                    for pattern in SECRET_PATTERNS:
                        self.assertIsNone(
                            pattern.search(scalar),
                            f"{path.name} 命中 secret 模式 {pattern.pattern!r}",
                        )

    def test_string_scalars_stay_bounded_and_single_line(self):
        for path in ALL_FIXTURES:
            document = load_fixture(path)
            for scalar in iter_strings(document):
                with self.subTest(path=path.name, scalar=scalar[:32]):
                    self.assertLessEqual(len(scalar), MAX_SCALAR_CHARS)
                    self.assertNotIn("\n", scalar)
                    self.assertNotIn("\r", scalar)

    def test_baseline_values_are_plain_numbers(self):
        metrics = load_fixture(BASELINE_PATH)["metrics"]
        for name, metric in metrics.items():
            with self.subTest(metric=name):
                self.assertIsInstance(metric["threshold"], (int, float))
                self.assertNotIsInstance(metric["threshold"], bool)


if __name__ == "__main__":
    unittest.main(verbosity=2)
