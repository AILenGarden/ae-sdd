import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import document_storage  # noqa: E402


def _setup_project() -> Path:
    tmp = Path(tempfile.mkdtemp())
    (tmp / ".ae-sdd" / "assets").mkdir(parents=True, exist_ok=True)
    (tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
    (tmp / ".ae-sdd" / "assets" / "test.assets.md").write_text(
        f"# §A §B §C §D §E §F §G\n\n| gitPath | `{tmp}` |\n| docWorkspacePath | `{tmp}` |\n",
        encoding="utf-8",
    )
    return tmp


class TestResolvePathVersion(unittest.TestCase):

    def test_dict_major_minor_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "TEST_REPORT",
            story_id="STORY-001", version={"major": 2, "minor": 3},
        )
        self.assertTrue(r.full_path.endswith("STORY-001-Report-v2-r3.md"))

    def test_dict_v_r_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "TEST_REPORT",
            story_id="STORY-001", version={"v": 2, "r": 4},
        )
        self.assertTrue(r.full_path.endswith("STORY-001-Report-v2-r4.md"))

    def test_string_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "CODING_REPORT",
            story_id="STORY-001", version="v3-r5",
        )
        self.assertTrue(r.full_path.endswith("STORY-001-CodingReport-v3-r5.md"))

    def test_compare_version_string(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "REVIEW_COMPARE",
            work_item_id="STORY-001", version="v1-to-v2",
        )
        self.assertTrue(r.full_path.endswith("STORY-001-ReviewCompare-v1-to-v2.md"))


class TestSaveDoc(unittest.TestCase):
    """save_doc 端到端：写文件 + ChangeLog + STORING + .gitignore + 版本号自增。"""

    def test_save_doc_writes_design_doc_in_place(self):
        """设计类文档（STORY）原地写入，不带版本号。"""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story 内容",
            story_id="STORY-001-BE", changelog_note="首次创建",
        )
        self.assertTrue(result.success, msg=result.error)
        # 文件落在 ae-sdd-doc/Story/，不带版本号
        self.assertTrue(result.full_path.replace("\\", "/").endswith("ae-sdd-doc/Story/STORY-001-BE.md"))
        self.assertTrue(Path(result.full_path).is_file())
        # 设计类无版本号
        self.assertIsNone(result.new_version)

    def test_save_doc_appends_changelog(self):
        """save_doc 追加 ChangeLog 到文档同级目录。"""
        tmp = _setup_project()
        document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story",
            story_id="STORY-001-BE", changelog_note="首次创建",
        )
        cl = tmp / "ae-sdd-doc" / "Story" / "STORY-001-BE-changelog.md"
        self.assertTrue(cl.is_file())
        self.assertIn("首次创建", cl.read_text(encoding="utf-8"))

    def test_save_doc_updates_storing_index(self):
        """save_doc 更新单一 ae-sdd-doc/STORING.md 索引。"""
        tmp = _setup_project()
        document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story",
            story_id="STORY-001-BE",
        )
        storing = tmp / "ae-sdd-doc" / "STORING.md"
        self.assertTrue(storing.is_file())
        content = storing.read_text(encoding="utf-8")
        self.assertIn("STORY-001-BE.md", content)
        self.assertIn("Story", content)

    def test_save_doc_maintains_gitignore(self):
        """save_doc 首次写入时维护 .gitignore（幂等追加 ae-sdd-doc/）。"""
        tmp = _setup_project()
        document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story",
            story_id="STORY-001-BE",
        )
        gi = tmp / ".gitignore"
        self.assertTrue(gi.is_file())
        self.assertIn("ae-sdd-doc/", gi.read_text(encoding="utf-8"))

    def test_save_doc_version_increment_for_report(self):
        """事件类报告（CODING_REPORT）未显式传 version 时 r 自增。"""
        tmp = _setup_project()
        # 第一份：应为 v1-r1
        r1 = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "CODING_REPORT", "# Coding 报告 r1",
            story_id="STORY-001", changelog_note="首轮",
        )
        self.assertTrue(r1.success, msg=r1.error)
        # 第二份：未传 version，应自增到 r2
        r2 = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "CODING_REPORT", "# Coding 报告 r2",
            story_id="STORY-001", changelog_note="次轮",
        )
        self.assertTrue(r2.success, msg=r2.error)
        # 两份报告都保留（旧版本不删）
        self.assertTrue(Path(r1.full_path).is_file())
        self.assertTrue(Path(r2.full_path).is_file())
        self.assertNotEqual(r1.full_path, r2.full_path)

    def test_save_doc_unknown_intent_e000(self):
        """未知 intent 返回失败，错误码含 E000。"""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "BOGUS_INTENT", "# 内容",
            story_id="X",
        )
        self.assertFalse(result.success)
        self.assertIn("E000", result.error)

    def test_issue_intent_is_implemented(self):
        """ISSUE 已登记的 intent 必须有真实路径实现。"""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "ISSUE", "# Bug issue",
            doc_id="BUG-LIFE-001",
        )
        self.assertTrue(result.success, msg=result.error)
        self.assertTrue(result.full_path.replace("\\", "/").endswith("ae-sdd-doc/Issue/BUG-LIFE-001.md"))
        self.assertTrue(Path(result.full_path).is_file())

    def test_work_item_id_buckets_task_without_story_id(self):
        """BUG/OPT 等独立编码任务可用 work_item_id 分桶，不必伪造成 Story。"""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "TASK", "# Task",
            work_item_id="BUG-LIFE-001", doc_id="TASK-001",
        )
        self.assertTrue(result.success, msg=result.error)
        normalized = result.full_path.replace("\\", "/")
        self.assertTrue(normalized.endswith("ae-sdd-doc/Task/BUG-LIFE-001/TASK-001.md"))

    def test_r_only_report_version_increment(self):
        """r-only 报告（如 TESTCASE_REVIEW）重入时 r 自增，不覆盖 r1。"""
        tmp = _setup_project()
        r1 = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "TESTCASE_REVIEW", "# review r1",
            work_item_id="BUG-LIFE-001",
        )
        r2 = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "TESTCASE_REVIEW", "# review r2",
            work_item_id="BUG-LIFE-001",
        )
        self.assertTrue(r1.success, msg=r1.error)
        self.assertTrue(r2.success, msg=r2.error)
        self.assertNotEqual(r1.full_path, r2.full_path)
        self.assertTrue(r1.full_path.endswith("TestCaseReview-r1.md"))
        self.assertTrue(r2.full_path.endswith("TestCaseReview-r2.md"))


class TestFinalizeDoc(unittest.TestCase):
    """finalize_doc：对已手写文件补 ChangeLog/STORING，不覆盖内容。"""

    def test_finalize_does_not_overwrite_content(self):
        """finalize 不覆盖已写文件的内容。"""
        tmp = _setup_project()
        # 先手写一个文件到最终路径
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-002-BE.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        original = "# 这是手写的原始内容，finalize 不能覆盖"
        target.write_text(original, encoding="utf-8")

        result = document_storage.finalize_doc(
            tmp / ".ae-sdd", "test", "STORY", str(target),
            story_id="STORY-002-BE", changelog_note="finalize 登记",
        )
        self.assertTrue(result.success, msg=result.error)
        # 内容未被覆盖
        self.assertEqual(target.read_text(encoding="utf-8"), original)

    def test_finalize_appends_changelog(self):
        """finalize 追加 ChangeLog 到文件同级目录。"""
        tmp = _setup_project()
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-003-BE.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("# 内容", encoding="utf-8")

        document_storage.finalize_doc(
            tmp / ".ae-sdd", "test", "STORY", str(target),
            story_id="STORY-003-BE", changelog_note="补登记",
        )
        cl = target.parent / "STORY-003-BE-changelog.md"
        self.assertTrue(cl.is_file())
        self.assertIn("补登记", cl.read_text(encoding="utf-8"))

    def test_finalize_updates_storing(self):
        """finalize 更新 STORING.md 索引（用已写文件路径）。"""
        tmp = _setup_project()
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-004-BE.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("# 内容", encoding="utf-8")

        document_storage.finalize_doc(
            tmp / ".ae-sdd", "test", "STORY", str(target),
            story_id="STORY-004-BE",
        )
        storing = tmp / "ae-sdd-doc" / "STORING.md"
        self.assertIn("STORY-004-BE.md", storing.read_text(encoding="utf-8"))

    def test_finalize_nonexistent_file_e009(self):
        """finalize 不存在的文件抛 E009。"""
        tmp = _setup_project()
        with self.assertRaises(document_storage.DocStorageError) as ctx:
            document_storage.finalize_doc(
                tmp / ".ae-sdd", "test", "STORY", str(tmp / "nope.md"),
                story_id="X",
            )
        self.assertIn("E009", str(ctx.exception))


class TestThinkingEngine(unittest.TestCase):
    def test_get_thinking_engine_uses_packaged_fallback(self):
        tmp = _setup_project()
        result = document_storage.get_thinking_engine(tmp / ".ae-sdd", "test")
        self.assertTrue(result, "expected packaged thinking engine fallback")
        self.assertTrue(result["path"].endswith("be-coding-thinking-engine.md"))
        self.assertIn("sha256", result)
        self.assertGreater(len(result["content"]), 100)

    def test_get_thinking_engine_prefers_project_override(self):
        tmp = _setup_project()
        override = tmp / "standards" / "thinking" / "be-coding-thinking-engine.md"
        override.parent.mkdir(parents=True)
        override.write_text("# project override\n\ncustom thinking engine", encoding="utf-8")

        result = document_storage.get_thinking_engine(tmp / ".ae-sdd", "test")
        self.assertEqual(Path(result["path"]), override.resolve())
        self.assertIn("custom thinking engine", result["content"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
