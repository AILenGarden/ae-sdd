"""
test_gates.py — gates.py 单元测试（14 门禁）

覆盖每个 check_gXX 函数的核心场景：缺失、通过、反例。
"""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import gates, paths  # noqa: E402


def _setup_project(structure: dict) -> Path:
    """构造临时项目目录。structure 是 {relpath: content} 字典"""
    tmp = Path(tempfile.mkdtemp())
    for rel, content in structure.items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    return tmp


def _full_ade_sdd(project_key: str = "test", phase: str = "initialized",
                   current_story: str = "") -> Path:
    """构造最小可用的 .ae-sdd/ + 项目结构"""
    return _setup_project({
        ".ae-sdd/config.yaml": f"projectKey: {project_key}\n",
        ".ae-sdd/state.json": f'{{"version": "1", "projectKey": "{project_key}", "phase": "{phase}", "currentStory": {"\"" + current_story + "\"" if current_story else "null"}}}\n',
    })


# ─── G-00 ───────────────────────────────────────────────────────────────────
class TestG00(unittest.TestCase):

    def test_no_ae_sdd_blocks(self):
        r = gates.check_g00(None, None, "test")
        self.assertFalse(r.pass_)
        self.assertIn("init", r.action)

    def test_complete_passes(self):
        tmp = _setup_project({
            ".ae-sdd/config.yaml": "projectKey: test\n",
            ".ae-sdd/state.json": '{"phase": "initialized"}\n',
            ".ae-sdd/assets/test.assets.md": "# §A §B §C §D §E §F §G\n",
        })
        ade_sdd = tmp / ".ae-sdd"
        r = gates.check_g00(None, ade_sdd, "test")
        self.assertTrue(r.pass_)

    def test_missing_assets_blocks(self):
        tmp = _setup_project({
            ".ae-sdd/config.yaml": "projectKey: test\n",
            ".ae-sdd/state.json": "{}",
        })
        ade_sdd = tmp / ".ae-sdd"
        r = gates.check_g00(None, ade_sdd, "test")
        self.assertFalse(r.pass_)
        self.assertIn("assets", r.message)

    def test_missing_7_layers_blocks(self):
        tmp = _setup_project({
            ".ae-sdd/config.yaml": "projectKey: test\n",
            ".ae-sdd/state.json": "{}",
            ".ae-sdd/assets/test.assets.md": "# empty\n",
        })
        ade_sdd = tmp / ".ae-sdd"
        r = gates.check_g00(None, ade_sdd, "test")
        self.assertFalse(r.pass_)
        self.assertIn("缺索引层", r.message)


# ─── G-01 ───────────────────────────────────────────────────────────────────
class TestG01(unittest.TestCase):

    def test_no_design_dir_blocks(self):
        tmp = _setup_project({})
        r = gates.check_g01(tmp, {}, "")
        self.assertFalse(r.pass_)

    def test_with_dr_passes(self):
        tmp = _setup_project({"design/DR-001.md": "# DR"})
        r = gates.check_g01(tmp, {}, "")
        self.assertTrue(r.pass_)

    def test_with_lowercase_dr_passes(self):
        tmp = _setup_project({"design/dr-001.md": "# dr"})
        r = gates.check_g01(tmp, {}, "")
        self.assertTrue(r.pass_)


# ─── G-02 ───────────────────────────────────────────────────────────────────
class TestG02(unittest.TestCase):

    def test_no_current_story_blocks(self):
        tmp = _setup_project({})
        r = gates.check_g02(tmp, {}, "")
        self.assertFalse(r.pass_)

    def test_with_story_passes(self):
        tmp = _setup_project({"design/STORY-001.md": "# Story"})
        r = gates.check_g02(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_missing_story_file_blocks(self):
        tmp = _setup_project({})
        r = gates.check_g02(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


# ─── G-03 ───────────────────────────────────────────────────────────────────
class TestG03(unittest.TestCase):

    def test_past_phase_passes(self):
        for phase in ["story-reviewed", "task-generated", "task-reviewed",
                      "coding", "test-running", "code-reviewed", "completed"]:
            with self.subTest(phase=phase):
                r = gates.check_g03(Path("."), {"phase": phase}, "STORY-001")
                self.assertTrue(r.pass_)

    def test_initialized_blocks(self):
        r = gates.check_g03(Path("."), {"phase": "initialized"}, "")
        self.assertFalse(r.pass_)

    def test_default_phase_blocks(self):
        # 没 phase 字段 → 默认 "initialized" → 失败
        r = gates.check_g03(Path("."), {}, "")
        self.assertFalse(r.pass_)


# ─── G-04 / G-05 / G-06 / G-07 ──────────────────────────────────────────────
class TestG04(unittest.TestCase):

    def test_with_testcase_passes(self):
        tmp = _setup_project({"design/STORY-001-testcase.md": "# TC"})
        r = gates.check_g04(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_no_story_blocks(self):
        r = gates.check_g04(Path("."), {}, "")
        self.assertFalse(r.pass_)

    def test_missing_testcase_blocks(self):
        tmp = _setup_project({})
        r = gates.check_g04(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


class TestG05(unittest.TestCase):

    def test_with_task_passes(self):
        tmp = _setup_project({"task/STORY-001-task-001.md": "# T1"})
        r = gates.check_g05(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_multiple_tasks(self):
        tmp = _setup_project({
            "task/STORY-001-task-001.md": "# T1",
            "task/STORY-001-task-002.md": "# T2",
            "task/OTHER-task-001.md": "# other",
        })
        r = gates.check_g05(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertEqual(len(r.details["files"]), 2)

    def test_no_task_dir_blocks(self):
        tmp = _setup_project({})
        r = gates.check_g05(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


class TestG06(unittest.TestCase):

    def test_past_phase_passes(self):
        for phase in ["task-reviewed", "coding", "test-running", "code-reviewed", "completed"]:
            with self.subTest(phase=phase):
                r = gates.check_g06(Path("."), {"phase": phase}, "STORY-001")
                self.assertTrue(r.pass_)

    def test_early_phase_blocks(self):
        for phase in ["initialized", "dr-generated", "story-generated", "story-reviewed", "task-generated"]:
            with self.subTest(phase=phase):
                r = gates.check_g06(Path("."), {"phase": phase}, "STORY-001")
                self.assertFalse(r.pass_)


class TestG07(unittest.TestCase):

    def test_with_codingplan_7_sections_passes(self):
        sections = "文件顺序 类骨架 数据 Mapper SQL 测试对应 验证点 调试回滚"
        tmp = _setup_project({f"design/STORY-001-CodingPlan.md": f"# CP\n{sections}"})
        r = gates.check_g07(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_missing_sections_blocks(self):
        tmp = _setup_project({"design/STORY-001-CodingPlan.md": "# CP\n只有文件顺序"})
        r = gates.check_g07(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_no_codingplan_blocks(self):
        tmp = _setup_project({})
        r = gates.check_g07(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


# ─── G-08 ───────────────────────────────────────────────────────────────────
class TestG08(unittest.TestCase):

    def _codingplan_md(self, keywords: list[str], marks: list[str]) -> str:
        """构造 CodingPlan 文档：14 关键词 + 14 标记"""
        lines = ["# CodingPlan"] + keywords
        lines.append("\n| # | 门禁 | 状态 |")
        lines.append("|---|------|------|")
        for i, m in enumerate(marks, 1):
            lines.append(f"| {i} | 门禁{i} | {m} |")
        return "\n".join(lines) + "\n"

    def test_full_pass(self):
        keywords = [
            "DR-Story-Task", "AC 100%", "文件顺序", "类骨架", "数据",
            "Mapper SQL", "测试对应", "验证点", "调试回滚", "资源隔离",
            "核心链路", "CodingModel", "混合压测", "测试真实性",
        ]
        marks = ["✅"] * 14
        tmp = _setup_project({"design/STORY-001-CodingPlan.md": self._codingplan_md(keywords, marks)})
        r = gates.check_g08(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_missing_keywords_blocks(self):
        # 缺 1 个关键词
        keywords = ["DR-Story-Task", "AC 100%"]  # 缺 12 个
        marks = ["✅"] * 14
        tmp = _setup_project({"design/STORY-001-CodingPlan.md": self._codingplan_md(keywords, marks)})
        r = gates.check_g08(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_with_failure_marks_blocks(self):
        keywords = [
            "DR-Story-Task", "AC 100%", "文件顺序", "类骨架", "数据",
            "Mapper SQL", "测试对应", "验证点", "调试回滚", "资源隔离",
            "核心链路", "CodingModel", "混合压测", "测试真实性",
        ]
        marks = ["✅"] * 13 + ["❌"]  # 1 条失败
        tmp = _setup_project({"design/STORY-001-CodingPlan.md": self._codingplan_md(keywords, marks)})
        r = gates.check_g08(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_insufficient_marks_blocks(self):
        keywords = [
            "DR-Story-Task", "AC 100%", "文件顺序", "类骨架", "数据",
            "Mapper SQL", "测试对应", "验证点", "调试回滚", "资源隔离",
            "核心链路", "CodingModel", "混合压测", "测试真实性",
        ]
        marks = ["✅"] * 10  # 只 10 条
        tmp = _setup_project({"design/STORY-001-CodingPlan.md": self._codingplan_md(keywords, marks)})
        r = gates.check_g08(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


# ─── G-09 ───────────────────────────────────────────────────────────────────
class TestG09(unittest.TestCase):

    def test_clean_java_passes(self):
        tmp = _setup_project({
            "src/test/java/SampleTest.java": (
                "package com.example;\n"
                "import org.junit.Test;\n"
                "import static org.junit.Assert.assertEquals;\n"
                "public class SampleTest {\n"
                "    @Test\n"
                "    public void t() { assertEquals(2, 1+1); }\n"
                "}\n"
            ),
        })
        r = gates.check_g09(tmp, {}, "STORY-001", master_source=paths.locate_master_source())
        self.assertTrue(r.pass_)

    def test_disabled_test_blocks(self):
        tmp = _setup_project({
            "src/test/java/BadTest.java": (
                "package com.example;\n"
                "import org.junit.Test;\n"
                "import org.junit.Ignore;\n"
                "public class BadTest {\n"
                "    @Test @Ignore\n"
                "    public void t() {}\n"
                "}\n"
            ),
        })
        r = gates.check_g09(tmp, {}, "STORY-001", master_source=paths.locate_master_source())
        self.assertFalse(r.pass_)
        self.assertGreater(r.details.get("n_blockers", 0), 0)

    def test_no_master_source_skips(self):
        tmp = _setup_project({})
        r = gates.check_g09(tmp, {}, "STORY-001", master_source=None)
        # 找不到母版 → 跳过
        self.assertTrue(r.details.get("skipped", False))


# ─── G-10 / G-11 / G-12 ─────────────────────────────────────────────────────
class TestG10(unittest.TestCase):

    def test_with_report_passes(self):
        tmp = _setup_project({"design/STORY-001-Report.md": "# R"})
        r = gates.check_g10(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_no_story_blocks(self):
        r = gates.check_g10(Path("."), {}, "")
        self.assertFalse(r.pass_)


class TestG11(unittest.TestCase):

    def test_with_coding_report_passes(self):
        tmp = _setup_project({"design/STORY-001-Coding-Report.md": "# CR"})
        r = gates.check_g11(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)


class TestG12(unittest.TestCase):

    def test_with_codereview_passes(self):
        tmp = _setup_project({"design/STORY-001-CodeReview.md": "# CV"})
        r = gates.check_g12(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)


# ─── G-13 ───────────────────────────────────────────────────────────────────
class TestG13(unittest.TestCase):

    def test_full_chain_passes(self):
        tmp = _setup_project({
            "design/DR-001.md": "# DR-001",
            "design/STORY-001.md": "# STORY-001 (引用 DR-001)",
            "task/STORY-001-task-001.md": "# Task 1 (实现 STORY-001)",
            "design/STORY-001-Coding-Report.md": "# Coding Report (完成 STORY-001-task-001)",
            "design/STORY-001-CodeReview.md": "# CodeReview (审核 STORY-001)",
        })
        r = gates.check_g13(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_broken_chain_blocks(self):
        tmp = _setup_project({
            "design/DR-001.md": "# DR-001",
            "design/STORY-001.md": "# STORY-001 (引用 DR-001)",
            "task/STORY-001-task-001.md": "# Task 1 无任何引用",  # 断链：不引用 STORY-001
        })
        r = gates.check_g13(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


# ─── check_all / summarize ───────────────────────────────────────────────────
class TestCheckAll(unittest.TestCase):

    def test_check_all_returns_14(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test")
        self.assertEqual(len(results), 14)

    def test_check_all_only_filter(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test", only="G-00")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].gate_id, "G-00")

    def test_check_all_unknown_gate(self):
        results = gates.check_all(None, None, "test", only="G-99")
        self.assertEqual(len(results), 1)
        self.assertFalse(results[0].pass_)

    def test_summarize(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test")
        summary = gates.summarize(results)
        self.assertEqual(summary["total"], 14)
        self.assertEqual(summary["passed"] + summary["failed"], 14)
        self.assertIn("results", summary)


# ─── GateResult 数据类 ─────────────────────────────────────────────────────
class TestGateResult(unittest.TestCase):

    def test_dataclass_fields(self):
        r = gates.GateResult(
            gate_id="G-XX", name="test", severity="blocker",
            pass_=True, message="ok",
        )
        self.assertEqual(r.gate_id, "G-XX")
        self.assertEqual(r.severity, "blocker")
        self.assertTrue(r.pass_)
        self.assertEqual(r.action, None)
        self.assertEqual(r.details, {})


if __name__ == "__main__":
    unittest.main(verbosity=2)
