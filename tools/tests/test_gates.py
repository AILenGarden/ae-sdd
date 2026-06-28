"""
test_gates.py — gates.py 单元测试（25 门禁：14 主 G-00~G-13 + 3 中段 G-14/G-CODEPLAN-SRC/G-DOC-STORAGE + 1 G-PATH + 4 G-RA + 1 G-RA-FLOW-VIOLATION + 1 G-CODE + 1 G-DOC-CONSISTENCY）

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

    def test_dr_in_subdir_passes_v3510(self):
        """🆕 v3.5.10 Gap-004：DR 文档在 design/ 子目录也应被识别（rglob 替代 glob）"""
        tmp = _setup_project({"design/story/be/DR-001.md": "# DR in subdir"})
        r = gates.check_g01(tmp, {}, "")
        self.assertTrue(r.pass_, msg=f"应识别子目录 DR：{r.message}")

    def test_excludes_report_docs_v3510(self):
        """🆕 v3.5.10 Gap-004：CodeReview/CodingReport 等报告类不应算 DR"""
        tmp = _setup_project({"design/STORY-001-CodeReview.md": "# CR",
                              "design/STORY-001-CodingReport.md": "# CR"})
        r = gates.check_g01(tmp, {}, "")
        self.assertFalse(r.pass_, msg="报告类文档不应被当作 DR")


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


# ─── G-CODE-1 ───────────────────────────────────────────────────────────────
class TestGCode1(unittest.TestCase):

    def test_clean_code_passes(self):
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        tmp = _setup_project({
            "src/main/java/com/example/SampleService.java": (
                "package com.example;\n"
                "public class SampleService {\n"
                "    public int sum(int a, int b) { return a + b; }\n"
                "}\n"
            ),
            "design/STORY-001-Coding-Report.md": (
                "# Coding Report\n"
                "`src/main/java/com/example/SampleService.java`\n"
            ),
        })
        r = gates.check_gcode1(tmp, {"phase": "coding"}, "STORY-001",
                               master_source=repo_source)
        self.assertTrue(r.pass_)
        self.assertEqual(r.details.get("n_blockers", -1), 0)

    def test_hardcoded_secret_blocks(self):
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        tmp = _setup_project({
            "src/main/java/com/example/BadService.java": (
                "package com.example;\n"
                "public class BadService {\n"
                "    private String token = \"abcdefg\";\n"
                "}\n"
            ),
        })
        r = gates.check_gcode1(tmp, {"phase": "coding"}, "STORY-001",
                               master_source=repo_source)
        self.assertFalse(r.pass_)
        self.assertGreater(r.details.get("n_blockers", 0), 0)

    def test_missing_code_file_in_report_blocks(self):
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        tmp = _setup_project({
            "src/main/java/com/example/SampleService.java": (
                "package com.example;\n"
                "public class SampleService {}\n"
            ),
            "design/STORY-001-Coding-Report.md": (
                "# Coding Report\n"
                "`src/main/java/com/example/MissingService.java`\n"
            ),
        })
        r = gates.check_gcode1(tmp, {"phase": "coding"}, "STORY-001",
                               master_source=repo_source)
        self.assertFalse(r.pass_)
        self.assertIn("coding-report-missing-code-file", r.details.get("blocker_rules", []))

    def test_no_master_source_skips(self):
        tmp = _setup_project({})
        r = gates.check_gcode1(tmp, {"phase": "coding"}, "STORY-001", master_source=None)
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


# ─── G-14 CodingPlan-Story 一致性（v3.4.0）──────────────────────────────────
class TestG14(unittest.TestCase):

    def _cp(self, body: str) -> str:
        return f"# STORY-001-CodingPlan\n{body}\n"

    def test_no_codingplan_blocks(self):
        tmp = _setup_project({"design/STORY-001.md": "# STORY-001 AC-001"})
        r = gates.check_g14(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_no_story_ref_blocks(self):
        tmp = _setup_project({
            "design/STORY-002-CodingPlan.md": "# Plan 无任何 Story 引用也无 AC",
        })
        r = gates.check_g14(tmp, {}, "STORY-002")
        self.assertFalse(r.pass_)

    def test_ac_misalign_blocks(self):
        # CodingPlan 无 AC 对齐，但 Story 含 AC-001/AC-002
        tmp = _setup_project({
            "design/STORY-001.md": "# STORY-001\nAC-001 ... \nAC-002 ...",
            "design/STORY-001-CodingPlan.md": "# Plan 引用 STORY-001 但测试章节无 AC 编号",
        })
        r = gates.check_g14(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_deviation_without_proposal_blocks(self):
        tmp = _setup_project({
            "design/STORY-001.md": "# STORY-001 AC-001",
            "design/STORY-001-CodingPlan.md": "# Plan STORY-001\n## 偏离声明\n接口路径偏离 Story\nAC-001 覆盖",
        })
        r = gates.check_g14(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_full_pass(self):
        tmp = _setup_project({
            "design/STORY-001.md": "# STORY-001 AC-001 AC-002",
            "design/STORY-001-CodingPlan.md": "# Plan STORY-001\n## 测试对应\nAC-001 ... \nAC-002 ...",
        })
        r = gates.check_g14(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)


# ─── G-CODEPLAN-SRC CodingPlan 源码核对（v3.4.0）─────────────────────────────
class TestGCodeplanSrc(unittest.TestCase):

    def _skeleton_cp(self, marks: list[str]) -> str:
        body = "# STORY-001-CodingPlan\n## §2 关键类骨架\n"
        for i, m in enumerate(marks, 1):
            body += f"\npublic class Foo{i} {{}}\n{m}\n"
        return body + "\n## §3 数据\n...\n"

    def test_no_codingplan_blocks(self):
        tmp = _setup_project({"design/STORY-001.md": "# STORY-001"})
        r = gates.check_g_codeplan_src(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_no_marks_blocks(self):
        tmp = _setup_project({
            "design/STORY-001-CodingPlan.md": self._skeleton_cp(["（无标记）"]),
        })
        r = gates.check_g_codeplan_src(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_pending_mark_blocks(self):
        tmp = _setup_project({
            "design/STORY-001-CodingPlan.md": self._skeleton_cp(["【已读源码：src/Foo.java】", "【待核实源码】"]),
            "src/Foo.java": "public class Foo {}",
        })
        r = gates.check_g_codeplan_src(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)
        self.assertGreater(r.details.get("n_pending", 0), 0)

    def test_missing_read_file_blocks(self):
        # 标已读但文件不存在 → 阻断
        tmp = _setup_project({
            "design/STORY-001-CodingPlan.md": self._skeleton_cp(["【已读源码：src/NotExist.java】"]),
        })
        r = gates.check_g_codeplan_src(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_all_read_passes(self):
        tmp = _setup_project({
            "design/STORY-001-CodingPlan.md": self._skeleton_cp(["【已读源码：src/Foo.java】", "【已读源码：src/Bar.java】"]),
            "src/Foo.java": "public class Foo {}",
            "src/Bar.java": "public class Bar {}",
        })
        r = gates.check_g_codeplan_src(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_no_skeleton_section_skips(self):
        # 无类骨架章节（微任务）→ 跳过不阻断
        tmp = _setup_project({
            "design/STORY-001-CodingPlan.md": "# Plan\n## §3 数据\n无类骨架\n",
        })
        r = gates.check_g_codeplan_src(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertTrue(r.details.get("skipped"))


# ─── G-DOC-STORAGE 文档落地存放合规（v3.4.0）─────────────────────────────────
class TestGDocStorage(unittest.TestCase):

    def test_clean_dir_passes(self):
        import tempfile
        tmp = Path(tempfile.mkdtemp())
        r = gates.check_g_doc_storage(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_stray_tmp_path_blocks(self):
        tmp = _setup_project({
            "tmp/STORY-001-CodingPlan.md": "# stray plan in tmp",
        })
        r = gates.check_g_doc_storage(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)
        self.assertGreater(len(r.details.get("stray_files", [])), 0)

    def test_compliant_design_root_passes(self):
        tmp = _setup_project({
            "design/STORY-001-CodingPlan.md": "# plan in design/",
            "ae-sdd-doc/Story/STORY-001.md": "# story in ae-sdd-doc/",
        })
        r = gates.check_g_doc_storage(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)


# ─── G-DOC-CONSISTENCY 项目侧记忆-配置路径一致性（🆕 v3.5.7）─────────────────
class TestGDocConsistency(unittest.TestCase):

    def test_consistent_memory_passes(self):
        """记忆文件文档根表述与 config 一致 → pass"""
        tmp = _setup_project({
            ".ae-sdd/config.yaml": "projectKey: life\nworkspaceKey: life\ngitPath: D:\\Item\\life\ndocWorkspacePath: D:\\Item\\life\n",
            ".ae-sdd/state.json": '{"phase": "initialized"}\n',
            ".ae-sdd/assets/life/life.assets.md": "# §A §B §C §D §E §F §G\n\n| docWorkspacePath | `D:\\Item\\life` |\n| gitPath | `D:\\Item\\life` |\n",
            "AGENTS.md": "- **项目文档工作区** = `D:\\Item\\life\\ae-sdd-doc\\`\n",
        })
        ade_sdd = tmp / ".ae-sdd"
        r = gates.check_g_doc_consistency(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_conflict_memory_blocks(self):
        """记忆文件文档根表述与 config 冲突 → blocker + 冲突详情"""
        tmp = _setup_project({
            ".ae-sdd/config.yaml": "projectKey: life\nworkspaceKey: life\ngitPath: D:\\Item\\life\ndocWorkspacePath: D:\\Item\\life\n",
            ".ae-sdd/state.json": '{"phase": "initialized"}\n',
            ".ae-sdd/assets/life/life.assets.md": "# §A §B §C §D §E §F §G\n\n| docWorkspacePath | `D:\\Item\\life` |\n| gitPath | `D:\\Item\\life` |\n",
            "AGENTS.md": "- **项目文档工作区** = `D:\\Item\\doc\\icec-cloud-boss\\...`\n",
        })
        r = gates.check_g_doc_consistency(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)
        conflicts = r.details.get("conflicts", [])
        self.assertGreater(len(conflicts), 0)
        self.assertEqual(conflicts[0]["file"], "AGENTS.md")
        self.assertIn("D:\\Item\\doc", conflicts[0]["path"])

    def test_no_config_skips_warn(self):
        """无 config.yaml → 降级 warn（pass=True，不阻断）"""
        tmp = _setup_project({
            "AGENTS.md": "- **项目文档工作区** = `D:\\Item\\doc\\`\n",
        })
        r = gates.check_g_doc_consistency(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertEqual(r.details.get("skipped"), "no_config")

    def test_generic_mention_not_blocked(self):
        """泛泛提及（无声明式线索词）不拦截，避免误伤历史引用"""
        tmp = _setup_project({
            ".ae-sdd/config.yaml": "projectKey: life\nworkspaceKey: life\ngitPath: D:\\Item\\life\ndocWorkspacePath: D:\\Item\\life\n",
            ".ae-sdd/state.json": '{"phase": "initialized"}\n',
            ".ae-sdd/assets/life/life.assets.md": "# §A §B §C §D §E §F §G\n\n| docWorkspacePath | `D:\\Item\\life` |\n| gitPath | `D:\\Item\\life` |\n",
            # 行内无"文档工作区/文档根"等声明式线索词，仅泛泛提及路径
            "AGENTS.md": "历史路径 `D:\\Item\\doc\\` 已作废，仅作参考\n",
        })
        r = gates.check_g_doc_consistency(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertEqual(len(r.details.get("conflicts", [])), 0)


# ─── check_all / summarize ───────────────────────────────────────────────────
class TestCheckAll(unittest.TestCase):

    def test_check_all_returns_all(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test")
        # v3.4.0：14 主门禁 + 3 中段门禁 + 1 G-PATH + 4 G-RA + 1 G-CODE = 23
        # 🆕 2026-06-27：+1 G-RA-FLOW-VIOLATION（建议书 §3.4）= 24
        # 🆕 v3.5.7：+1 G-DOC-CONSISTENCY（项目侧记忆-配置路径一致性）= 25
        # 🆕 v3.5.9：+1 G-RA-5（RA 机械派生深度，防「形式通过、内容空转」）= 26
        self.assertEqual(len(results), 26)

    def test_check_all_only_filter(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test", only="G-00")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].gate_id, "G-00")

    def test_check_all_only_filter_gra(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test", only="G-RA-1")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].gate_id, "G-RA-1")

    def test_check_all_unknown_gate(self):
        results = gates.check_all(None, None, "test", only="G-99")
        self.assertEqual(len(results), 1)
        self.assertFalse(results[0].pass_)

    def test_summarize(self):
        ade_sdd = _full_ade_sdd()
        results = gates.check_all(None, ade_sdd, "test")
        summary = gates.summarize(results)
        # v3.4.0：14 主门禁 + 3 中段门禁 + 1 G-PATH + 4 G-RA + 1 G-CODE = 23
        # 🆕 2026-06-27：+1 G-RA-FLOW-VIOLATION（建议书 §3.4）= 24
        # 🆕 v3.5.7：+1 G-DOC-CONSISTENCY（项目侧记忆-配置路径一致性）= 25
        # 🆕 v3.5.9：+1 G-RA-5（RA 机械派生深度，防「形式通过、内容空转」）= 26
        self.assertEqual(summary["total"], 26)
        self.assertEqual(summary["passed"] + summary["failed"], 26)
        self.assertIn("results", summary)


# ─── G-RA 需求分析准入门卫（v3.2）─────────────────────────────────────────────
def _ra_doc(content: str = "") -> str:
    """构造一份含 8 维度 + RAModel 12 维的合规 RA 文档内容。"""
    base = """# 需求分析报告：测试需求

## §0.5 RequirementAnalysisModel 决策记录
| 维度 | 结论 | 证据 | 风险等级 | 后续动作 |
|------|------|------|----------|----------|
| RA-01 输入保真 | {x} | PRD §1 | 🟢 | {x} |
| RA-02 目标与成功标准 | {x} | PRD §2 | 🟢 | {x} |
| RA-03 角色与权限边界 | {x} | PRD §3 | 🟢 | {x} |
| RA-04 场景拓扑 | {x} | PRD §4 | 🟢 | {x} |
| RA-05 状态与生命周期 | {x} | PRD §5 | 🟢 | {x} |
| RA-06 数据语义与所有权 | {x} | assets.table | 🟢 | {x} |
| RA-07 业务规则与衍生规则 | {x} | PRD §6 | 🟢 | {x} |
| RA-08 跨域级联 | {x} | PRD §7 | 🟢 | {x} |
| RA-09 现有能力复用 | {x} | assets.search | 🟢 | {x} |
| RA-10 非功能与约束 | {x} | PRD §8 | 🟢 | {x} |
| RA-11 AC 与测试可验证性 | {x} | PRD §9 | 🟢 | {x} |
| RA-12 规模与路由置信度 | {x} | §11 | 🟢 | {x} |

## §2 角色分析
角色枚举：C 端用户、运营人员。

## §3 场景分析
主流程场景、异常场景、边界场景。

## §4 业务流程
状态机、时序。

## §5 数据要素
实体、字段。

## §6 业务规则
R1：{规则}

## §7 设计方向
备选方案、方案对比。

## §8 验收标准
AC-001 Given When Then。

## §9 隐性假设
假设清单。
"""
    return base + content


class TestGRA1(unittest.TestCase):
    """G-RA-1 RA 文档存在"""

    def test_pre_ra_phase_no_doc_stub_passes(self):
        tmp = _setup_project({})
        # initialized 阶段无 RA 文档 → stub 通过
        r = gates.check_ra_required(tmp, {"phase": "initialized"}, "")
        self.assertTrue(r.pass_)
        self.assertTrue(r.details.get("stub"))

    def test_post_ra_phase_no_doc_blocks(self):
        tmp = _setup_project({})
        # 已进入下游节点（story-generated）却无 RA → 阻断
        r = gates.check_ra_required(tmp, {"phase": "story-generated"}, "STORY-001")
        self.assertFalse(r.pass_)
        self.assertIn("未找到 RA 文档", r.message)

    def test_with_ra_passes(self):
        tmp = _setup_project({"design/RA-001-v1.0.md": _ra_doc()})
        r = gates.check_ra_required(tmp, {"phase": "story-generated"}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertGreaterEqual(r.details.get("ra_files", 0), 1)

    def test_stale_ra_warns_but_passes(self):
        import os
        tmp = _setup_project({"design/RA-001-v1.0.md": _ra_doc()})
        # 把 mtime 改到 60 天前
        p = tmp / "design" / "RA-001-v1.0.md"
        old_time = p.stat().st_mtime - 60 * 86400
        os.utime(p, (old_time, old_time))
        r = gates.check_ra_required(tmp, {"phase": "story-generated"}, "STORY-001")
        self.assertTrue(r.pass_)  # warn 不阻断
        self.assertEqual(r.severity, "warn")


class TestGRA2(unittest.TestCase):
    """G-RA-2 RA 8 维度完整 + RAModel 12 维"""

    def test_no_ra_stub_passes(self):
        tmp = _setup_project({})
        r = gates.check_ra_dimensions(tmp, {}, "STORY-001")
        self.assertTrue(r.details.get("stub"))

    def test_full_ra_passes(self):
        tmp = _setup_project({"design/RA-001-v1.0.md": _ra_doc()})
        r = gates.check_ra_dimensions(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)

    def test_missing_dimension_blocks(self):
        # 缺 §9 假设维度（同时去掉所有 "§9"/"假设" 痕迹，避免 RAModel 证据列误命中）
        bad = _ra_doc().replace("## §9 隐性假设\n假设清单。", "")
        bad = bad.replace("假设", "推测").replace("PRD §9", "PRD §A")
        tmp = _setup_project({"design/RA-001-v1.0.md": bad})
        r = gates.check_ra_dimensions(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_missing_ramodel_blocks(self):
        bad = _ra_doc().replace("RA-12", "RA-XX")
        tmp = _setup_project({"design/RA-001-v1.0.md": bad})
        r = gates.check_ra_dimensions(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)


class TestGRA3(unittest.TestCase):
    """G-RA-3 RA 衍生章节完整"""

    def test_no_ra_stub_passes(self):
        tmp = _setup_project({})
        r = gates.check_ra_derivatives(tmp, {}, "STORY-001")
        self.assertTrue(r.details.get("stub"))

    def test_state_machine_missing_derivatives_blocks(self):
        # 含状态变更关键词但缺衍生章节
        bad = "# 需求分析\n账号锁定触发状态变更。\n## §2 角色\n## §3 场景\n## §4 流程\n状态机\n## §5 数据\n## §6 规则\n## §7 设计方向\n## §8 AC\n## §9 假设\n"
        tmp = _setup_project({"design/RA-001-v1.0.md": bad})
        r = gates.check_ra_derivatives(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)
        self.assertTrue(r.details.get("state_machine"))

    def test_non_state_machine_without_notapplicable_blocks(self):
        # 非状态机需求缺衍生章节且无"不适用"声明
        bad = "# 需求分析\n纯查询需求。\n## §2 角色\n## §3 场景\n## §4 流程\n## §5 数据\n## §6 规则\n## §7 设计方向\n## §8 AC\n## §9 假设\n"
        tmp = _setup_project({"design/RA-001-v1.0.md": bad})
        r = gates.check_ra_derivatives(tmp, {}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_non_state_machine_with_notapplicable_passes(self):
        good = "# 需求分析\n纯查询需求，不涉及状态变更。\n## §6.5 衍生规则登记表\n不适用：纯 CRUD 无衍生（PRD §3）。\n## §8.5 衍生 AC 登记表\n不适用。\n## §8.6 衍生覆盖率\n不适用。\n## §9-bis 业务模式匹配表\n不适用。\n## §9-ter 跨域级联效应表\n不适用。\n"
        tmp = _setup_project({"design/RA-001-v1.0.md": good})
        r = gates.check_ra_derivatives(tmp, {}, "STORY-001")
        self.assertTrue(r.pass_)


class TestGRA4(unittest.TestCase):
    """G-RA-4 RA 真实性扫描通过"""

    def test_no_master_source_skips(self):
        tmp = _setup_project({})
        r = gates.check_ra_authenticity(tmp, {}, "STORY-001", master_source=None)
        self.assertTrue(r.details.get("skipped", False))

    def test_pre_ra_no_doc_stub_passes(self):
        # master_source 指向本仓库 source/（其 parent 有 scripts/ra_authenticity_scan.py）
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        tmp = _setup_project({})
        r = gates.check_ra_authenticity(tmp, {"phase": "initialized"}, "STORY-001",
                                        master_source=repo_source)
        self.assertTrue(r.details.get("stub"))

    def test_clean_ra_passes(self):
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        clean = "# 需求分析\n纯 CRUD 查询，经 H.5 模式 1-6 全检后确认无衍生，理由：不涉及状态变更（PRD §3）。\n"
        tmp = _setup_project({"design/RA-001-v1.0.md": clean})
        r = gates.check_ra_authenticity(tmp, {"phase": "ra-generated"}, "STORY-001",
                                        master_source=repo_source)
        self.assertTrue(r.pass_)
        self.assertEqual(r.details.get("blockers", -1), 0)

    def test_fabricated_ra_blocks(self):
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        # 含 vague-ellipsis + missing-timeliness + masked-gap
        bad = "# 需求分析\n账号锁定等等。\n- 锁定后尽快下线。\n- 已解决。\n"
        tmp = _setup_project({"design/RA-001-v1.0.md": bad})
        r = gates.check_ra_authenticity(tmp, {"phase": "ra-generated"}, "STORY-001",
                                        master_source=repo_source)
        self.assertFalse(r.pass_)
        self.assertGreater(r.details.get("blockers", 0), 0)


class TestGRA5(unittest.TestCase):
    """G-RA-5 RA 机械派生深度通过（v3.5.9 — 防「形式通过、内容空转」）"""

    def test_no_master_source_skips(self):
        tmp = _setup_project({})
        r = gates.check_ra_depth(tmp, {}, "STORY-001", master_source=None)
        self.assertTrue(r.details.get("skipped", False))

    def test_pre_ra_no_doc_stub_passes(self):
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        tmp = _setup_project({})
        r = gates.check_ra_depth(tmp, {"phase": "initialized"}, "STORY-001",
                                 master_source=repo_source)
        self.assertTrue(r.details.get("stub"))

    def test_form_pass_empty_ra_blocks(self):
        """空转 RA（§6.5 仅表头无 R→R' 行 + §9-ter 时效含「尽快」+ §9-bis 缺模式）→ BLOCKER。"""
        repo_source = Path(__file__).resolve().parent.parent.parent / "source"
        empty_ra = (
            "# RA\n\n"
            "## §6.5 衍生规则登记表\n\n"
            "| 规则 # | 主规则 R | 衍生规则 R' | 衍生模式命中 | R' 优先级 |\n"
            "|--------|----------|-------------|--------------|-----------|\n"
            "| R1 | 锁定 |  |  | P0 |\n\n"
            "## §8.5 衍生 AC 登记表\n\n"
            "| AC # | 主场景 | 衍生动作 | 时效要求 | 对应规则 R' |\n"
            "|------|--------|----------|----------|-------------|\n\n"
            "## §9-bis 业务模式匹配表\n\n"
            "| 套用的模式 | 模式 # | 命中的衍生影响编号 | 备注 |\n"
            "|------------|--------|--------------------|------|\n"
            "| 账号状态变更 | 1 |  |  |\n\n"
            "## §9-ter 跨域级联效应表\n\n"
            "本需求涉及微服务聚合根与 MQ topic、WebSocket、Redis、CQRS。\n\n"
            "| 触发动作 | 受影响域 | 受影响状态机/事件/缓存/MQ | 触发方式 | 时效要求 | 反向影响 |\n"
            "|----------|----------|---------------------------|----------|----------|----------|\n"
            "| 账号锁定 | User | 状态变更 | 本域事务内 | 尽快 | — |\n"
        )
        tmp = _setup_project({"design/RA-empty-v1.0.md": empty_ra})
        r = gates.check_ra_depth(tmp, {"phase": "ra-generated"}, "STORY-001",
                                 master_source=repo_source)
        self.assertFalse(r.pass_, f"空转 RA 应被 G-RA-5 拦截，但 pass_={r.pass_}")
        self.assertGreater(r.details.get("blockers", 0), 0)


# ─── G-13 RA 层追溯（v3.2 升级）──────────────────────────────────────────────
class TestG13RaLayer(unittest.TestCase):

    def test_no_ra_layer_passes(self):
        # 无 RA 文档（可选层）→ 不阻断
        tmp = _setup_project({
            "design/DR-001.md": "# DR\n引用 RA-001",
            "design/STORY-001.md": "# Story\n引用 DR-001",
        })
        r = gates.check_g13(tmp, {"phase": "story-generated"}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertFalse(r.details.get("ra_layer", {}).get("present", True))

    def test_ra_layer_dr_not_ref_blocks(self):
        # RA 存在但 DR 未引用 RA-ID → issue
        tmp = _setup_project({
            "design/RA-001-v1.0.md": "# RA",
            "design/DR-001.md": "# DR\n没有引用任何 RA",
            "design/STORY-001.md": "# Story\nDR-001",
        })
        r = gates.check_g13(tmp, {"phase": "story-generated"}, "STORY-001")
        self.assertFalse(r.pass_)

    def test_ra_layer_full_trace_passes(self):
        tmp = _setup_project({
            "design/RA-001-v1.0.md": "# RA",
            "design/DR-001.md": "# DR\n基于 RA-001 编写",
            "design/STORY-001.md": "# Story\n参考 DR-001",
        })
        r = gates.check_g13(tmp, {"phase": "story-generated"}, "STORY-001")
        self.assertTrue(r.pass_)
        self.assertTrue(r.details.get("ra_layer", {}).get("present"))


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
