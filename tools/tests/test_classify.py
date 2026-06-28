"""
test_classify.py — classify.py 单元测试（4 维判定）

覆盖：
- 来源判定（PRD / Issue / 对话 / DR）
- 规模判定（微 / 小 / 中 / 大）
- AI 适配判定（高 / 中 / 低）
- 多 Agent 判定（中以上启用）
- next_action 建议
- 文件输入
"""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import classify  # noqa: E402


class TestClassifySource(unittest.TestCase):
    """来源判定测试"""

    def test_prd_keyword(self):
        c = classify.classify("请根据 PRD 的要求实现")
        self.assertEqual(c.source, "PRD")

    def test_issue_keyword(self):
        c = classify.classify("修复 Issue #123 的 bug")
        self.assertEqual(c.source, "Issue")

    def test_dialogue_keyword(self):
        c = classify.classify("我想要一个新的功能，希望...")
        self.assertEqual(c.source, "对话")

    def test_dr_keyword(self):
        c = classify.classify("基于 DR-001 的设计实现")
        self.assertEqual(c.source, "DR")

    def test_no_keyword_defaults_to_dialogue(self):
        c = classify.classify("做一些修改")
        self.assertEqual(c.source, "对话")
        # 置信度低（默认）
        self.assertLess(c.confidence["source"], 0.5)


class TestClassifyScale(unittest.TestCase):
    """规模判定测试"""

    def test_micro_keyword(self):
        c = classify.classify("fix typo in README")
        self.assertEqual(c.scale, "微")

    def test_small_keyword(self):
        c = classify.classify("小任务：改 1-3 个文件")
        self.assertEqual(c.scale, "小")

    def test_medium_keyword(self):
        c = classify.classify("中等任务：改 10 个文件")
        self.assertEqual(c.scale, "中")

    def test_large_keyword(self):
        c = classify.classify("大重构：整个模块需要重新设计")
        self.assertEqual(c.scale, "大")

    def test_no_keyword_inferred_from_length(self):
        # 200+ 行（按行数推断） → 大
        long_text = "\n".join(f"实现细节 {i}" for i in range(300))
        c = classify.classify(long_text)
        self.assertEqual(c.scale, "大")

    def test_short_text_inferred_micro(self):
        c = classify.classify("hello")
        self.assertEqual(c.scale, "微")


class TestClassifyAIFit(unittest.TestCase):
    """AI 适配判定测试"""

    def test_high_keyword(self):
        c = classify.classify("自动化的、标准化的任务")
        self.assertEqual(c.ai_fit, "高")

    def test_no_keyword_inferred_from_scale(self):
        # 微/小 → 高
        c = classify.classify("fix typo")
        self.assertEqual(c.ai_fit, "高")

    def test_large_inferred_low(self):
        c = classify.classify("大重构整个系统")
        # 大 + 无 AI 关键词 → 低
        self.assertEqual(c.ai_fit, "低")


class TestMultiAgent(unittest.TestCase):
    """多 Agent 判定测试"""

    def test_micro_no_multi_agent(self):
        c = classify.classify("fix typo")
        self.assertFalse(c.multi_agent)

    def test_small_no_multi_agent(self):
        c = classify.classify("小任务 改 1 个文件")
        self.assertFalse(c.multi_agent)

    def test_medium_uses_multi_agent(self):
        c = classify.classify("中等任务 改 10 个文件")
        self.assertTrue(c.multi_agent)

    def test_large_uses_multi_agent(self):
        c = classify.classify("大重构 整个模块")
        self.assertTrue(c.multi_agent)


class TestNextAction(unittest.TestCase):
    """next_action 建议测试"""

    def test_micro_goes_to_coding(self):
        c = classify.classify("fix typo in README")
        self.assertEqual(c.next_action, "coding")

    def test_small_dialogue_goes_to_requirement_analysis(self):
        # 小规模对话源 → 先做需求分析
        c = classify.classify("我想要修改用户头像的小任务")
        # "小任务" 命中 scale=小，source=对话 → next_action = requirement-analysis
        self.assertEqual(c.next_action, "requirement-analysis")

    def test_small_dr_keyword_goes_to_dr_generate(self):
        # DR 源 + 中等规模 → dr-generate
        c = classify.classify("基于 DR-001 设计实现的中等任务：10 个文件")
        # scale=中（命中"中等"），source=DR → next_action = dr-generate
        self.assertEqual(c.next_action, "dr-generate")

    def test_large_dialogue_goes_to_requirement_analysis(self):
        c = classify.classify("大重构整个用户域，重新设计")
        # scale=大，source=对话 → next_action = requirement-analysis
        self.assertEqual(c.next_action, "requirement-analysis")


class TestClassifyFromFile(unittest.TestCase):
    """从文件读取文本分类测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_read_file(self):
        f = self.tmp / "input.md"
        f.write_text("大重构：整个用户域重新设计", encoding="utf-8")
        c = classify.classify_from_file(f)
        self.assertEqual(c.source, "对话")
        self.assertEqual(c.scale, "大")

    def test_file_not_found_raises(self):
        f = self.tmp / "nonexistent.md"
        with self.assertRaises(FileNotFoundError):
            classify.classify_from_file(f)


class TestConfidenceScores(unittest.TestCase):
    """置信度范围测试"""

    def test_confidence_in_range(self):
        c = classify.classify("根据 PRD 实现一个标准的小任务")
        for key, conf in c.confidence.items():
            self.assertGreaterEqual(conf, 0.0)
            self.assertLessEqual(conf, 1.0)


class TestProjectContextScale(unittest.TestCase):
    """🆕 v3.5.10 Gap-014：project_context 参数覆盖行数推断的 scale 误判"""

    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.project = Path(self.tmp) / "proj"
        self.project.mkdir()

    def test_short_text_without_context_is_micro(self):
        """无 project_context 时，短文本仍判微（向后兼容）"""
        c = classify.classify("实现 IM 知识库匹配")
        self.assertEqual(c.scale, "微")

    def test_short_text_with_ra_context_is_large(self):
        """有 RA 产物时，短文本应被覆盖为'大'"""
        # 造一个 ≥100 行的 RA 文档
        ra_dir = self.project / "ae-sdd-doc" / "iterations" / "2026-06-28" / "RA"
        ra_dir.mkdir(parents=True)
        (ra_dir / "RA-TEST-v1.0.md").write_text("# RA\n" + "\n".join(f"line {i}" for i in range(120)))
        c = classify.classify("实现 IM 知识库匹配", project_context=self.project)
        self.assertEqual(c.scale, "大")

    def test_short_text_with_blocking_gaps_is_large(self):
        """有 blockingGaps ≥5 + RA 阶段完成时，判'大'"""
        ae_sdd = self.project / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "state.json").write_text('{"currentStory": "STORY-001"}')
        story_dir = self.project / ".auto-engineering" / "STORY-001"
        story_dir.mkdir(parents=True)
        (story_dir / "state.json").write_text(
            '{"blockingGaps": ["g1","g2","g3","g4","g5","g6"], '
            '"completedSteps": ["requirement-analysis-r1", "ra-generated"]}'
        )
        c = classify.classify("实现预约功能", project_context=self.project)
        self.assertEqual(c.scale, "大")

    def test_no_context_fallback_to_lines(self):
        """project_context 无任何产物时 fallback 到行数推断"""
        c = classify.classify("实现预约功能", project_context=self.project)
        # 无产物 → 行数推断 → 微
        self.assertEqual(c.scale, "微")

    def test_invalid_project_context_ignored(self):
        """无效 project_context 路径被忽略，不报错"""
        c = classify.classify("实现预约功能", project_context=Path("/nonexistent/path/xyz"))
        self.assertEqual(c.scale, "微")


if __name__ == "__main__":
    unittest.main(verbosity=2)
