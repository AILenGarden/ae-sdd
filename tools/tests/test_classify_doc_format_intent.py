"""🆕 v3.11.6 tests for micro-doc-format intent routing (DOC_FORMAT entry_node).

覆盖 micro 意图分流的第三个 entry_node：
  - DOC_FORMAT：仅调整已有设计文档排版/格式、语义不变 → scale="微" + entry_node="DOC_FORMAT"

消歧优先级（最关键，与 v3.10.2 OPTIMIZE/CODE_REVIEW 同一范式）：
  1. self-update 上下文（ae-sdd/SKILL/流程）> 文档上下文
     —— 但"根据 ae-sdd 的模板格式调整文档"中的 ae-sdd 是引用型前缀（援引标准），
        不计入 self-update 信号；真正的 self-update 词（skill/流程/门禁/runtime 等）仍生效。
  2. 内容变更信号（新增字段/新增接口等）> 格式关键词
     —— 防止"调整格式的同时改语义"被误判为零语义变更的轻量任务。
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import classify  # noqa: E402


class TestDocFormatIntent(unittest.TestCase):
    """micro-doc-format 意图判定：仅调整文档排版/格式 → DOC_FORMAT 微链"""

    def test_adjust_story_format_per_ae_sdd_template(self):
        """真实触发场景：'请根据 ae-sdd 的 Story 模板格式调整 document 目录下的
        Story003 和 004。本次任务仅调整格式，不允许丢失原文档语义。'"""
        c = classify.classify(
            "请根据ae-sdd的Story模板格式调整document目录下的Story003和004。"
            "本次任务仅调整格式，不允许丢失原文档语义。"
        )
        self.assertEqual(c.entry_node, "DOC_FORMAT")
        self.assertEqual(c.scale, "微")

    def test_adjust_dr_layout(self):
        c = classify.classify("按照 ae-sdd 的模板格式排版一下这份 DR 文档")
        self.assertEqual(c.entry_node, "DOC_FORMAT")
        self.assertEqual(c.scale, "微")

    def test_unify_prd_format_no_ae_sdd_mention(self):
        """完全不提 ae-sdd 也应命中——DOC_FORMAT 不要求引用 ae-sdd 标准"""
        c = classify.classify("把这份 PRD 文档排版统一一下")
        self.assertEqual(c.entry_node, "DOC_FORMAT")
        self.assertEqual(c.scale, "微")

    def test_testcase_format_adjustment(self):
        c = classify.classify("这份 TestCase 文档格式调整一下，套模板")
        self.assertEqual(c.entry_node, "DOC_FORMAT")
        self.assertEqual(c.scale, "微")


class TestDisambiguationSelfUpdatePriority(unittest.TestCase):
    """消歧 1：self-update 上下文（真实 self-update 词）压倒文档上下文"""

    def test_optimize_ae_sdd_story_template_not_doc_format(self):
        """'优化' 命中 _OPTIMIZE_KEYWORDS 分支在前，不会进入 DOC_FORMAT 分支；
        但即使假设改用格式类关键词，含'优化'时应先被 OPTIMIZE 分支截获（分支顺序验证）。"""
        c = classify.classify("优化 ae-sdd 的 Story 模板格式")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")

    def test_ae_sdd_skill_keyword_blocks_doc_format(self):
        """引用前缀'根据 ae-sdd' + 真实 self-update 词'skill' 同时出现时，
        真实 self-update 词仍生效，不应判 DOC_FORMAT。"""
        c = classify.classify("根据 ae-sdd 的 skill 定义调整这个 Story 文档格式")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")

    def test_adjust_runtime_format_not_doc_format(self):
        c = classify.classify("调整一下 runtime 切片的格式")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")


class TestDisambiguationContentChangePriority(unittest.TestCase):
    """消歧 2：内容变更信号压倒格式关键词——防止语义变更被误判为纯格式任务"""

    def test_format_with_new_field_not_doc_format(self):
        c = classify.classify("调整这个 Story 文档格式，同时新增字段")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")

    def test_format_with_new_interface_not_doc_format(self):
        c = classify.classify("把这份 DR 文档排版一下，并新增接口章节")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")

    def test_format_with_requirement_change_not_doc_format(self):
        c = classify.classify("这份 PRD 格式化的同时变更需求范围")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")


class TestNoDocContextExclusion(unittest.TestCase):
    """无文档上下文时不应触发 DOC_FORMAT（如"格式化代码"应走别的路由）"""

    def test_format_code_not_doc_format(self):
        c = classify.classify("格式化一下这段代码")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")

    def test_format_java_file_not_doc_format(self):
        c = classify.classify("把 OrderService.java 排版一下")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")


class TestHelperFunctions(unittest.TestCase):
    """_is_doc_context / _has_content_change_signal / _has_doc_format_selfupdate_context 单测"""

    def test_doc_context_detected(self):
        self.assertTrue(classify._is_doc_context("调整这个 story 文档格式"))
        self.assertTrue(classify._is_doc_context("这份 dr 排版一下"))
        self.assertTrue(classify._is_doc_context("prd.md 格式化"))

    def test_doc_context_not_detected(self):
        self.assertFalse(classify._is_doc_context("格式化这段代码"))

    def test_content_change_signal_detected(self):
        self.assertTrue(classify._has_content_change_signal("调整格式，新增字段"))
        self.assertTrue(classify._has_content_change_signal("排版的同时新增接口"))

    def test_content_change_signal_not_detected(self):
        self.assertFalse(classify._has_content_change_signal("仅调整格式，不改语义"))

    def test_doc_format_selfupdate_context_reference_prefix_excludes_bare_ae_sdd(self):
        # 引用前缀命中 + 无其他 self-update 词 → 不算 self-update 上下文
        self.assertFalse(
            classify._has_doc_format_selfupdate_context("根据 ae-sdd 的模板格式调整文档")
        )

    def test_doc_format_selfupdate_context_reference_prefix_keeps_real_selfupdate_word(self):
        # 引用前缀命中 + 真实 self-update 词 'skill' → 仍算 self-update 上下文
        self.assertTrue(
            classify._has_doc_format_selfupdate_context("根据 ae-sdd 的 skill 定义调整文档格式")
        )

    def test_doc_format_selfupdate_context_no_reference_prefix_falls_back_to_generic(self):
        # 无引用前缀，裸 "ae-sdd" 走通用检测 → 仍算 self-update 上下文
        self.assertTrue(
            classify._has_doc_format_selfupdate_context("优化 ae-sdd 的文档格式")
        )


class TestBackwardCompatibility(unittest.TestCase):
    """原有 entry_node 行为不被 DOC_FORMAT 分支破坏（分支顺序在 OPTIMIZE/CODE_REVIEW 之后）"""

    def test_optimize_still_optimize(self):
        c = classify.classify("优化这段代码的性能")
        self.assertEqual(c.entry_node, "OPTIMIZE")
        self.assertEqual(c.scale, "微")

    def test_code_review_still_code_review(self):
        c = classify.classify("审查这段实现")
        self.assertEqual(c.entry_node, "CODE_REVIEW")
        self.assertEqual(c.scale, "微")

    def test_bug_still_bug(self):
        c = classify.classify("修复登录 bug")
        self.assertEqual(c.entry_node, "BUG")
        self.assertEqual(c.scale, "微")

    def test_config_still_config(self):
        c = classify.classify("改个常量配置")
        self.assertEqual(c.entry_node, "CONFIG")
        self.assertEqual(c.scale, "微")

    def test_plain_dialogue_not_doc_format(self):
        c = classify.classify("做一些修改")
        self.assertNotEqual(c.entry_node, "DOC_FORMAT")


if __name__ == "__main__":
    unittest.main()
