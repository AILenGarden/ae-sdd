"""Contract tests for the bounded, risk-driven TestCase strategy."""
from __future__ import annotations

import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
STRATEGY = REPO_ROOT / "source" / "standards" / "testing" / "be-testcase-strategy.md"
GENERATE = (
    REPO_ROOT
    / "source"
    / "skill-fallbacks"
    / "skills"
    / "phase1-design"
    / "testcase-generate-skill.full.md"
)
REVIEW = (
    REPO_ROOT
    / "source"
    / "skill-fallbacks"
    / "skills"
    / "phase1-design"
    / "testcase-review-skill.full.md"
)
TEMPLATE = REPO_ROOT / "source" / "templates" / "testcase" / "be-testcase-template.md"
THINKING_ENGINE = (
    REPO_ROOT
    / "source"
    / "standards"
    / "thinking"
    / "be-coding-thinking-engine.md"
)
DESIGN = REPO_ROOT / "source" / "docs" / "ae-sdd-design.md"


def _read(path: Path) -> str:
    if not path.is_file():
        raise AssertionError(f"required bounded-test artifact is missing: {path}")
    return path.read_text(encoding="utf-8")


class TestBoundedTestStrategy(unittest.TestCase):
    def test_strategy_defines_admission_local_budgets_and_a_stop_rule(self):
        text = _read(STRATEGY)
        for contract in (
            "有限风险登记",
            "行为等价类",
            "边界测试准入",
            "局部数量上限",
            "最低充分层级",
            "停止条件",
            "预算例外",
            "笛卡尔积",
        ):
            self.assertIn(contract, text)
        for protected_risk in ("安全", "权限", "金额", "数据丢失", "事务", "并发", "幂等", "不可逆"):
            self.assertIn(protected_risk, text)

    def test_strategy_removes_unbounded_count_and_ratio_requirements(self):
        text = _read(STRATEGY)
        for obsolete in (
            "N×(N-1) 条单步路径",
            "每个字段至少 1 条有效值用例 + 1 条无效值用例",
            "实际用例数 ≥ 公式计算的最少用例数",
            "证伪用例占比 ≥ 成熟度阈值",
            "总用例数 = 第一层 + 第二层",
        ):
            self.assertNotIn(obsolete, text)

    def test_generate_and_review_enforce_value_instead_of_quotas(self):
        generate = _read(GENERATE)
        review = _read(REVIEW)
        for text in (generate, review):
            for contract in ("独立失败机制", "选择决策", "停止条件", "预算例外"):
                self.assertIn(contract, text)
        for obsolete in (
            "实际用例数不得低于策略公式",
            "证伪用例占比 =",
            "坑库成熟度 ≥ 50% → 阈值 40%",
            "坑库成熟度 < 50% → 阈值 25%",
        ):
            self.assertNotIn(obsolete, generate + "\n" + review)

    def test_template_records_selection_and_boundedness_evidence(self):
        text = _read(TEMPLATE)
        for field in (
            "风险等级",
            "证据来源",
            "行为分区",
            "独立失败机制",
            "选择决策",
            "合并至",
            "停止条件证据",
            "预算例外",
        ):
            self.assertIn(field, text)

    def test_coding_model_and_design_use_the_same_bounded_contract(self):
        thinking = _read(THINKING_ENGINE)
        design = _read(DESIGN)
        for contract in ("有限风险登记", "独立失败机制", "停止条件"):
            self.assertIn(contract, thinking)
        self.assertNotIn("一个分支对应一个用例", thinking)
        self.assertNotIn("6类场景全部覆盖", thinking)
        self.assertNotIn("所有边界条件和业务规则都覆盖了吗", thinking)
        self.assertIn("D-025", design)
        self.assertIn("风险驱动的有界测试策略", design)

    def test_remaining_examples_do_not_reintroduce_mechanical_expansion(self):
        strategy = _read(STRATEGY)
        generate = _read(GENERATE)
        template = _read(TEMPLATE)
        self.assertNotIn("if/else 每个分支都走到", strategy)
        self.assertNotIn("核心 AC 至少一个负向验证", generate)
        self.assertNotIn("覆盖兜底用例", template)


if __name__ == "__main__":
    unittest.main(verbosity=2)
