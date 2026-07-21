"""
test_context_gates.py — v3.9.1 上下文加载准入门禁单测

覆盖 G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX 四个门禁：
  - 齐备 pass
  - 各项缺失 block
  - scale 豁免（小/微链对应阶段）
  - pre-phase stub 通过
  - 已过 phase skipped

注册表驱动（CONTEXT_GATE_REGISTRY），复用 document-storage-skill API：
  get_constraints / get_assets / paths.find_doc / _iter_ra_files / _find_prd_files
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import gates, paths  # noqa: E402


def _setup_project(structure: dict) -> Path:
    """构造临时项目目录。structure 是 {relpath: content} 字典。"""
    tmp = Path(tempfile.mkdtemp())
    for rel, content in structure.items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    return tmp


def _make_state(phase: str = "dr-generated", scale: str = "大",
                current_story: str = "") -> dict:
    """构造 state dict。"""
    st = {"phase": phase, "scale": scale}
    if current_story:
        st["currentStory"] = current_story
    return st


# assets.md gitPath 占位符（_full_context_project 在 _setup_project 后回填真实 tmp 路径）
tmp_drive_placeholder = "__TMP_PATH__"


def _full_context_project(phase: str = "dr-generated", scale: str = "大",
                          current_story: str = "STORY-001") -> Path:
    """构造一个上下文齐备的项目（constraints + assets + RA + PRD + DR + Story + TestCase 全有）。

    assets.md 含 gitPath 字段（get_constraints/get_assets 依赖它定位 constraints/ 和资产文件）。
    """
    tmp = _setup_project({
        ".ae-sdd/config.yaml": "projectKey: test\nworkspaceKey: test\n",
        ".ae-sdd/state.json": json.dumps(_make_state(phase, scale, current_story),
                                          ensure_ascii=False) + "\n",
        # assets.md §1 含 gitPath + 7 层索引（G-00 也要求 §A~§G）
        ".ae-sdd/assets/test.assets.md": (
            f"# test assets\n\n"
            f"| gitPath | `{tmp_drive_placeholder}` |\n\n"
            f"# §A §B §C §D §E §F §G\n"
        ),
        "constraints/code-style.md": "# code style\n",
        "constraints/database.md": "# database\n",
        "ae-sdd-doc/RA/RA-001.md": "# RA\n## §2 角色\n## §3 场景\n## §4 流程\n## §5 数据\n## §6 规则\n## §7 设计方向\n## §8 AC\n## §9 假设\n",
        "PRD-001.md": "# PRD\n业务背景\n",
        "design/DR-001.md": "# DR\n业务背景 + 业务规则 + 验收标准\n",
        f"design/{current_story}.md": (
            "# Story\n"
            "AC + 接口契约 + 数据模型\n"
            # 🆕 v3.9.20 G-STORY-CTX standardsRef 门禁：大/中链正文须引用 ≥3 个约束类别
            # （_STANDARDS_KEYWORDS 见 gates.py:2653）。此处引用 layered-arch/database/
            # security/testing 四类关键词，满足大/中链 ≥3、小/微链 ≥1 的阈值。
            "遵循分层架构（Controller/Service/Repository），禁止跨层调用。\n"
            "数据库设计含 DDL 约束与索引规范，表名/字段名遵循命名约定。\n"
            "数据访问一律参数化查询，防 SQL 注入；敏感数据需脱敏。\n"
            "交付物须含单元测试，覆盖率满足测试策略要求。\n"
        ),
        f"design/{current_story}-testcase.md": "# TestCase\n场景清单 + 测试分层\n",
    })
    # assets.md 的 gitPath 需指向实际 tmp 目录（_setup_project 先建文件，此处回填真实路径）
    asset_file = tmp / ".ae-sdd" / "assets" / "test.assets.md"
    content = asset_file.read_text(encoding="utf-8").replace(
        tmp_drive_placeholder, str(tmp).replace("\\", "/"))
    asset_file.write_text(content, encoding="utf-8")
    return tmp


# ─── G-DR-CTX ───────────────────────────────────────────────────────────────
class TestGDrCtx(unittest.TestCase):

    def test_full_context_passes(self):
        """大链 dr-generated，constraints/assets/RA/PRD 齐备 → pass。"""
        tmp = _full_context_project(phase="dr-generated", scale="大")
        r = gates.check_g_dr_ctx(tmp, _make_state("dr-generated", "大"), "")
        self.assertTrue(r.pass_, msg=f"expected pass, got: {r.message}")

    def test_missing_constraints_blocks(self):
        """缺 constraints → block。"""
        tmp = _full_context_project(phase="dr-generated", scale="大")
        for f in (tmp / "constraints").glob("*.md"):
            f.unlink()
        r = gates.check_g_dr_ctx(tmp, _make_state("dr-generated", "大"), "")
        self.assertFalse(r.pass_)
        self.assertIn("项目约束", r.message)

    def test_missing_ra_blocks(self):
        """缺 RA → block。"""
        tmp = _full_context_project(phase="dr-generated", scale="大")
        (tmp / "ae-sdd-doc" / "RA" / "RA-001.md").unlink()
        r = gates.check_g_dr_ctx(tmp, _make_state("dr-generated", "大"), "")
        self.assertFalse(r.pass_)
        self.assertIn("RA", r.message)

    def test_missing_prd_blocks(self):
        """缺 PRD → block。"""
        tmp = _full_context_project(phase="dr-generated", scale="大")
        (tmp / "PRD-001.md").unlink()
        r = gates.check_g_dr_ctx(tmp, _make_state("dr-generated", "大"), "")
        self.assertFalse(r.pass_)
        self.assertIn("PRD", r.message)

    def test_small_scale_exempt(self):
        """小链豁免 G-DR-CTX（小链跳过 DR）。"""
        tmp = _full_context_project(phase="dr-generated", scale="小")
        r = gates.check_g_dr_ctx(tmp, _make_state("dr-generated", "小"), "")
        self.assertTrue(r.pass_)
        self.assertTrue(r.details.get("skipped"))

    def test_pre_phase_stub(self):
        """phase=initialized（pre-phase）→ stub 通过。"""
        tmp = _full_context_project(phase="initialized", scale="大")
        r = gates.check_g_dr_ctx(tmp, _make_state("initialized", "大"), "")
        self.assertTrue(r.pass_)
        self.assertTrue(r.details.get("skipped"))


# ─── G-STORY-CTX ────────────────────────────────────────────────────────────
class TestGStoryCtx(unittest.TestCase):

    def test_full_context_passes(self):
        """大链 story-generated，constraints/assets/DR/PRD 齐备 → pass。"""
        tmp = _full_context_project(phase="story-generated", scale="大",
                                     current_story="STORY-001")
        r = gates.check_g_story_ctx(tmp, _make_state("story-generated", "大", "STORY-001"), "STORY-001")
        self.assertTrue(r.pass_, msg=f"expected pass, got: {r.message}")

    def test_missing_dr_blocks(self):
        """缺 DR → block。"""
        tmp = _full_context_project(phase="story-generated", scale="大")
        (tmp / "design" / "DR-001.md").unlink()
        r = gates.check_g_story_ctx(tmp, _make_state("story-generated", "大", "STORY-001"), "STORY-001")
        self.assertFalse(r.pass_)
        self.assertIn("DR", r.message)
        self.assertIn("不得在 Story 任务中运行 dr-generate-skill", r.action)

    def test_process_heading_blocks(self):
        """Story 正文出现过程型章节 → outputBoundary 阻断。"""
        tmp = _full_context_project(phase="story-generated", scale="大")
        story = tmp / "design" / "STORY-001.md"
        story.write_text(
            story.read_text(encoding="utf-8") + "\n## 生成过程\n已完成门禁检查。\n",
            encoding="utf-8",
        )
        r = gates.check_g_story_ctx(
            tmp, _make_state("story-generated", "大", "STORY-001"), "STORY-001"
        )
        self.assertFalse(r.pass_)
        self.assertIn("Story 输出边界", r.message)
        self.assertIn("生成过程", r.message)

    def test_process_artifact_reference_blocks(self):
        """Story 正文引用内部 Plan/Report → outputBoundary 阻断。"""
        tmp = _full_context_project(phase="story-generated", scale="大")
        story = tmp / "design" / "STORY-001.md"
        story.write_text(
            story.read_text(encoding="utf-8") + "\n详见 StoryGeneratePlan。\n",
            encoding="utf-8",
        )
        r = gates.check_g_story_ctx(
            tmp, _make_state("story-generated", "大", "STORY-001"), "STORY-001"
        )
        self.assertFalse(r.pass_)
        self.assertIn("STORYGENERATEPLAN", r.message)

    def test_small_scale_exempt(self):
        """🆕 v3.9.20：小/微链豁免已取消，改走 standardsRef 轻量阈值（≥1 类别）。

        原 v3.9.1 语义"小链跳过 Story（skipped=True）"随 G-STORY-CTX scales 扩到
        {大,中,小,微} 而废弃；现小链与大链同样需加载上下文，仅 standardsRef 阈值
        从 3 降到 1。本例 Story 正文仅含 1 个约束类别关键词（分层架构），应通过。
        """
        tmp = _full_context_project(phase="story-generated", scale="小",
                                     current_story="STORY-001")
        # 覆盖 Story 正文为仅 1 个标准类别引用，验证小链阈值=1（非豁免、非 stub）
        (tmp / "design" / "STORY-001.md").write_text(
            "# Story\nAC + 数据模型\n遵循分层架构（Controller/Service）。\n",
            encoding="utf-8")
        r = gates.check_g_story_ctx(tmp, _make_state("story-generated", "小", "STORY-001"),
                                    "STORY-001")
        self.assertTrue(r.pass_, msg=f"small scale should pass with 1 standard ref, got: {r.message}")
        # 非 skipped：standardsRef 真实执行并命中 1 类（证明走的是阈值=1 而非豁免）
        self.assertFalse(r.details.get("skipped"),
                         msg="small scale no longer exempt; should run the gate, not skip")
        self.assertIn("standardsRef", r.details.get("status", {}))
        self.assertTrue(r.details["status"]["standardsRef"])

    def test_passed_phase_skipped(self):
        """phase=task-generated（已过 story 阶段）→ skipped。"""
        tmp = _full_context_project(phase="task-generated", scale="大")
        r = gates.check_g_story_ctx(tmp, _make_state("task-generated", "大", "STORY-001"), "STORY-001")
        self.assertTrue(r.pass_)
        self.assertTrue(r.details.get("skipped"))


# ─── G-TESTCASE-CTX ─────────────────────────────────────────────────────────
class TestGTestcaseCtx(unittest.TestCase):

    def test_full_context_passes(self):
        """大链 testcase-generated，constraints/assets/Story 齐备 → pass。"""
        tmp = _full_context_project(phase="testcase-generated", scale="大",
                                     current_story="STORY-001")
        r = gates.check_g_testcase_ctx(tmp, _make_state("testcase-generated", "大", "STORY-001"), "STORY-001")
        self.assertTrue(r.pass_, msg=f"expected pass, got: {r.message}")

    def test_missing_story_blocks(self):
        """缺 Story 文档 → block。"""
        tmp = _full_context_project(phase="testcase-generated", scale="大")
        (tmp / "design" / "STORY-001.md").unlink()
        r = gates.check_g_testcase_ctx(tmp, _make_state("testcase-generated", "大", "STORY-001"), "STORY-001")
        self.assertFalse(r.pass_)
        self.assertIn("Story", r.message)

    def test_missing_current_story_blocks(self):
        """current_story 为空 → block。"""
        tmp = _full_context_project(phase="testcase-generated", scale="大")
        r = gates.check_g_testcase_ctx(tmp, _make_state("testcase-generated", "大", ""), "")
        self.assertFalse(r.pass_)
        self.assertIn("Story", r.message)


# ─── G-TASK-CTX ─────────────────────────────────────────────────────────────
class TestGTaskCtx(unittest.TestCase):

    def test_full_context_passes(self):
        """大链 task-generated，constraints/assets/Story/TestCase 齐备 → pass。"""
        tmp = _full_context_project(phase="task-generated", scale="大",
                                     current_story="STORY-001")
        r = gates.check_g_task_ctx(tmp, _make_state("task-generated", "大", "STORY-001"), "STORY-001")
        self.assertTrue(r.pass_, msg=f"expected pass, got: {r.message}")

    def test_missing_testcase_blocks(self):
        """大链缺 TestCase → block。"""
        tmp = _full_context_project(phase="task-generated", scale="大")
        (tmp / "design" / "STORY-001-testcase.md").unlink()
        r = gates.check_g_task_ctx(tmp, _make_state("task-generated", "大", "STORY-001"), "STORY-001")
        self.assertFalse(r.pass_)
        self.assertIn("TestCase", r.message)

    def test_micro_scale_exempt_story_testcase(self):
        """微链豁免 Story/TestCase（微链无 Story/TestCase 产物）。"""
        tmp = _full_context_project(phase="task-generated", scale="微")
        # 微链即使删掉 Story/TestCase 也应 pass（required_micro 只查 constraints+assets）
        (tmp / "design" / "STORY-001.md").unlink()
        (tmp / "design" / "STORY-001-testcase.md").unlink()
        r = gates.check_g_task_ctx(tmp, _make_state("task-generated", "微", ""), "")
        self.assertTrue(r.pass_, msg=f"micro should pass with constraints+assets only, got: {r.message}")

    def test_micro_missing_constraints_blocks(self):
        """微链缺 constraints 仍 block（required_micro 含 constraints）。"""
        tmp = _full_context_project(phase="task-generated", scale="微")
        for f in (tmp / "constraints").glob("*.md"):
            f.unlink()
        r = gates.check_g_task_ctx(tmp, _make_state("task-generated", "微", ""), "")
        self.assertFalse(r.pass_)
        self.assertIn("项目约束", r.message)


if __name__ == "__main__":
    unittest.main()
