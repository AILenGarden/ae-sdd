"""🆕 v3.10.2 tests for micro intent routing (OPTIMIZE / CODE_REVIEW entry_node).

覆盖 micro 意图分流的两个新 entry_node：
  - OPTIMIZE：优化/重构/改进代码 → scale="微" + entry_node="OPTIMIZE"
  - CODE_REVIEW：审查/CodeReview/评审代码 → scale="微" + entry_node="CODE_REVIEW"

消歧优先级（最关键）：
  self-update 上下文（ae-sdd/SKILL/流程）> 代码上下文
  - "优化这部分实现" → OPTIMIZE（micro）
  - "优化 ae-sdd" → 不进 micro（self-update，entry_node 非 OPTIMIZE）
  - "优化 ae-sdd 的实现" → 不进 micro（self-update 上下文压倒代码上下文）
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import classify  # noqa: E402


class TestMicroOptimizeIntent(unittest.TestCase):
    """micro-optimize 意图判定：优化/重构/改进代码 → OPTIMIZE 微链"""

    def test_optimize_implementation(self):
        c = classify.classify("请帮我优化这部分实现")
        self.assertEqual(c.entry_node, "OPTIMIZE")
        self.assertEqual(c.scale, "微")

    def test_optimize_code(self):
        c = classify.classify("优化这段代码的性能")
        self.assertEqual(c.entry_node, "OPTIMIZE")
        self.assertEqual(c.scale, "微")

    def test_refactor(self):
        c = classify.classify("帮我重构这个方法")
        self.assertEqual(c.entry_node, "OPTIMIZE")
        self.assertEqual(c.scale, "微")

    def test_improve(self):
        c = classify.classify("改进一下这个 service 的逻辑")
        self.assertEqual(c.entry_node, "OPTIMIZE")
        self.assertEqual(c.scale, "微")

    def test_optimize_with_explicit_code_context(self):
        c = classify.classify("优化 OrderService.java 的实现逻辑")
        self.assertEqual(c.entry_node, "OPTIMIZE")
        self.assertEqual(c.scale, "微")


class TestMicroReviewIntent(unittest.TestCase):
    """micro-review 意图判定：审查/CodeReview → CODE_REVIEW 微链"""

    def test_codereview_english(self):
        c = classify.classify("帮我 CodeReview 这段代码")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")

    def test_review_code(self):
        c = classify.classify("code review 一下这个文件")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")

    def test_shencha(self):
        c = classify.classify("审查这段实现")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")

    def test_ping_shen_daima(self):
        c = classify.classify("评审代码：OrderController")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")

    def test_daima_shencha(self):
        c = classify.classify("这段逻辑做一下代码审查")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")

    def test_cr_baogao(self):
        c = classify.classify("出 CR 报告")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")


class TestDisambiguationSelfUpdatePriority(unittest.TestCase):
    """消歧：self-update 上下文压倒代码上下文（最关键的防误路由测试）

    原痛点：`优化` 同时匹配 micro-optimize 和 self-update。
    当文本含 ae-sdd/SKILL/流程 等词时，"优化"指向 ae-sdd 自身，
    应走 self-update 路由（entry_node 不是 OPTIMIZE），不进 micro。
    """

    def test_optimize_ae_sdd_not_micro(self):
        c = classify.classify("优化 ae-sdd 的路由逻辑")
        self.assertNotEqual(c.entry_node, "OPTIMIZE")

    def test_optimize_skill_not_micro(self):
        c = classify.classify("优化 SKILL.md 的路由表")
        self.assertNotEqual(c.entry_node, "OPTIMIZE")

    def test_optimize_runtime_not_micro(self):
        c = classify.classify("优化 runtime 切片")
        self.assertNotEqual(c.entry_node, "OPTIMIZE")

    def test_optimize_flow_not_micro(self):
        c = classify.classify("优化流程编排")
        self.assertNotEqual(c.entry_node, "OPTIMIZE")

    def test_optimize_ae_sdd_even_with_code_word(self):
        """即使含"代码"一词，ae-sdd 上下文仍压倒——属 self-update"""
        c = classify.classify("优化 ae-sdd 的代码实现")
        self.assertNotEqual(c.entry_node, "OPTIMIZE")


class TestReviewNotTriggeredBySelfUpdateContext(unittest.TestCase):
    """审查类在 self-update 上下文下也不进 micro-review"""

    def test_review_skill_not_micro(self):
        c = classify.classify("审查 ae-sdd 的 skill 设计")
        self.assertNotEqual(c.entry_node, "CODE_REVIEW")


class TestHelperFunctions(unittest.TestCase):
    """_has_selfupdate_context / _is_code_context 辅助函数单测"""

    def test_selfupdate_context_detected(self):
        self.assertTrue(classify._has_selfupdate_context("优化 ae-sdd"))
        self.assertTrue(classify._has_selfupdate_context("改 skill"))
        self.assertTrue(classify._has_selfupdate_context("优化流程"))
        self.assertTrue(classify._has_selfupdate_context("改 tools/lib/gates.py"))

    def test_selfupdate_context_not_detected(self):
        self.assertFalse(classify._has_selfupdate_context("优化这段实现"))
        self.assertFalse(classify._has_selfupdate_context("重构 OrderService"))

    def test_code_context_detected(self):
        self.assertTrue(classify._is_code_context("优化这段代码"))
        self.assertTrue(classify._is_code_context("改这个方法"))
        self.assertTrue(classify._is_code_context("重构 OrderService.java"))
        self.assertTrue(classify._is_code_context("优化 service 逻辑"))

    def test_code_context_not_detected(self):
        self.assertFalse(classify._is_code_context("优化 ae-sdd 流程"))


class TestBackwardCompatibility(unittest.TestCase):
    """原有 entry_node 行为不被 micro 意图分流破坏"""

    def test_bug_still_bug_entry(self):
        c = classify.classify("修复登录 bug")
        self.assertEqual(c.entry_node, "BUG")
        self.assertEqual(c.scale, "微")

    def test_config_still_config_entry(self):
        c = classify.classify("改个常量配置")
        self.assertEqual(c.entry_node, "CONFIG")
        self.assertEqual(c.scale, "微")

    def test_prd_still_prd_entry(self):
        c = classify.classify("# PRD: 电商系统\n\n实现订单模块")
        self.assertEqual(c.entry_node, "PRD")

    def test_plain_dialogue_not_micro_intent(self):
        """普通对话不误判为 OPTIMIZE/CODE_REVIEW"""
        c = classify.classify("做一些修改")
        self.assertNotIn(c.entry_node, ("OPTIMIZE", "CODE_REVIEW"))


if __name__ == "__main__":
    unittest.main()
