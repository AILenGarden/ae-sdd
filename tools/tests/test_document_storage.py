import sys
import tempfile
import unittest
import hashlib
from pathlib import Path
from unittest.mock import patch

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
            tmp / ".ae-sdd", "test", "TRACE_MATRIX",
            story_id="STORY-001", version={"major": 2, "minor": 3},
        )
        self.assertTrue(r.full_path.endswith("STORY-001-追溯矩阵-v2-r3.md"))

    def test_dict_v_r_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "TRACE_MATRIX",
            story_id="STORY-001", version={"v": 2, "r": 4},
        )
        self.assertTrue(r.full_path.endswith("STORY-001-追溯矩阵-v2-r4.md"))

    def test_string_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "TRACE_MATRIX",
            story_id="STORY-001", version="v3-r5",
        )
        self.assertTrue(r.full_path.endswith("STORY-001-追溯矩阵-v3-r5.md"))

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

    def test_save_doc_no_changelog(self):
        """🆕 v3.10.1：save_doc 不再生成 ChangeLog 旁车文件。"""
        tmp = _setup_project()
        document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story",
            story_id="STORY-001-BE", changelog_note="首次创建",
        )
        cl = tmp / "ae-sdd-doc" / "Story" / "STORY-001-BE-changelog.md"
        self.assertFalse(cl.is_file(), "v3.10.1 不应生成 ChangeLog 旁车文件")

    def test_save_doc_updates_machine_index(self):
        """save_doc 更新 JSON 索引，不生成 STORING.md。"""
        tmp = _setup_project()
        document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story",
            story_id="STORY-001-BE",
        )
        index = tmp / "ae-sdd-doc" / "index.json"
        self.assertTrue(index.is_file())
        content = index.read_text(encoding="utf-8")
        self.assertIn("STORY-001-BE.md", content)
        self.assertIn("Story", content)
        self.assertFalse((tmp / "ae-sdd-doc" / "STORING.md").exists())

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

    def test_retired_process_intents_fail_closed(self):
        """过程 Markdown intent 停止新写入，历史文件仍由 resolve 兼容读取。"""
        tmp = _setup_project()
        for intent in ("TRACE_MATRIX", "CODING_PLAN", "CODE_REVIEW", "TEST_REPORT", "PROPOSAL"):
            result = document_storage.save_doc(
                tmp / ".ae-sdd", "test", intent, "# retired",
                story_id="STORY-001",
            )
            self.assertFalse(result.success, intent)
            self.assertIn("E012", result.error or "", intent)

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

    def test_explicit_work_item_is_canonical_even_when_story_is_present(self):
        """Retired CodingPlan no longer writes an execution Markdown artifact."""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "CODING_PLAN", "# Plan",
            work_item_id="BUG-LIFE-004", story_id="STORY-004-BE",
        )
        self.assertFalse(result.success)
        self.assertIn("E012", result.error or "")

    def test_explicit_work_item_priority_across_intents(self):
        """Only explicit optional TESTCASE/TASK retain Work Item identity."""
        tmp = _setup_project()
        for intent, suffix in [
            ("TESTCASE", "BUG-LIFE-004-testcase.md"),
            ("TASK", "BUG-LIFE-004.md"),
        ]:
            result = document_storage.save_doc(
                tmp / ".ae-sdd", "test", intent, "# doc",
                work_item_id="BUG-LIFE-004", story_id="STORY-004-BE",
            )
            self.assertTrue(result.success, msg=result.error)
            normalized = result.full_path.replace("\\", "/")
            self.assertIn(
                "/BUG-LIFE-004/", normalized,
                f"{intent} 目录名应为 work_item_id=BUG-LIFE-004，实际: {normalized}",
            )
            self.assertTrue(normalized.endswith(suffix), normalized)

    def test_story_is_legacy_bucket_when_work_item_is_absent(self):
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY", "# Story",
            story_id="STORY-004-BE",
        )
        self.assertTrue(result.success, msg=result.error)
        normalized = result.full_path.replace("\\", "/")
        self.assertTrue(
            normalized.endswith("ae-sdd-doc/Story/STORY-004-BE.md")
        )

    def test_bug_task_keeps_work_item_bucketing(self):
        """🆕 v3.10.4 回归：无 story_id 的 BUG/OPT 任务仍用 work_item_id 分桶。"""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "TASK", "# Task",
            work_item_id="BUG-LIFE-001", doc_id="TASK-001",
        )
        self.assertTrue(result.success, msg=result.error)
        normalized = result.full_path.replace("\\", "/")
        self.assertTrue(
            normalized.endswith("ae-sdd-doc/Task/BUG-LIFE-001/TASK-001.md"),
            f"无 story_id 时应回退 work_item_id 分桶，实际: {normalized}",
        )

    def test_non_story_scoped_intent_unaffected(self):
        """🆕 v3.10.4 回归：非 Story-scoped intent（如 PRD）不受 story_id 分桶影响。"""
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "PRD", "# PRD",
            work_item_id="Story-004", story_id="STORY-004-BE", doc_id="PRD-001",
        )
        self.assertTrue(result.success, msg=result.error)
        normalized = result.full_path.replace("\\", "/")
        # PRD 用 {docId} 不用 {workItem}，路径不受分桶逻辑影响
        self.assertTrue(normalized.endswith("ae-sdd-doc/PRD/PRD-001.md"))


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

    def test_finalize_no_changelog(self):
        """🆕 v3.10.1：finalize 不再生成 ChangeLog 旁车文件。"""
        tmp = _setup_project()
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-003-BE.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("# 内容", encoding="utf-8")

        document_storage.finalize_doc(
            tmp / ".ae-sdd", "test", "STORY", str(target),
            story_id="STORY-003-BE", changelog_note="补登记",
        )
        cl = target.parent / "STORY-003-BE-changelog.md"
        self.assertFalse(cl.is_file(), "v3.10.1 不应生成 ChangeLog 旁车文件")

    def test_finalize_updates_machine_index(self):
        """finalize 更新 JSON 索引，不生成 STORING.md。"""
        tmp = _setup_project()
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-004-BE.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("# 内容", encoding="utf-8")

        document_storage.finalize_doc(
            tmp / ".ae-sdd", "test", "STORY", str(target),
            story_id="STORY-004-BE",
        )
        index = tmp / "ae-sdd-doc" / "index.json"
        self.assertIn("STORY-004-BE.md", index.read_text(encoding="utf-8"))
        self.assertFalse((tmp / "ae-sdd-doc" / "STORING.md").exists())

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


class TestReadResources(unittest.TestCase):
    def test_story_template_uses_packaged_fallback_with_content_hash(self):
        tmp = _setup_project()
        result = document_storage.resolve_read_resource(
            tmp / ".ae-sdd", "test", "STORY_TEMPLATE"
        )

        self.assertEqual(result["source"], "packaged-default")
        self.assertEqual(result["path"], result["fullPath"])
        self.assertFalse(result["writable"])
        self.assertIn("ae-sdd:story-section", result["content"])
        self.assertEqual(
            result["sha256"],
            hashlib.sha256(result["content"].encode("utf-8")).hexdigest(),
        )

    def test_story_resource_prefers_project_override(self):
        tmp = _setup_project()
        override = tmp / "templates/design/story-template.md"
        override.parent.mkdir(parents=True)
        override.write_text("# project story template\n", encoding="utf-8")

        result = document_storage.resolve_read_resource(
            tmp / ".ae-sdd", "test", "STORY_TEMPLATE"
        )

        self.assertEqual(result["source"], "project-override")
        self.assertEqual(Path(result["path"]), override.resolve())
        self.assertEqual(result["content"], "# project story template\n")

    def test_ambiguous_project_resource_overrides_fail_closed(self):
        tmp = _setup_project()
        for override in (
            tmp / "templates/design/story-template.md",
            tmp / ".ae-sdd/templates/design/story-template.md",
        ):
            override.parent.mkdir(parents=True, exist_ok=True)
            override.write_text("# override\n", encoding="utf-8")

        with self.assertRaises(document_storage.DocStorageError) as ctx:
            document_storage.resolve_read_resource(
                tmp / ".ae-sdd", "test", "STORY_TEMPLATE"
            )
        self.assertIn("E014", str(ctx.exception))

    def test_read_resource_intents_reject_save(self):
        tmp = _setup_project()
        result = document_storage.save_doc(
            tmp / ".ae-sdd", "test", "STORY_TEMPLATE", "# overwrite"
        )
        self.assertFalse(result.success)
        self.assertIn("E013", result.error or "")


class TestStoryDocumentResolution(unittest.TestCase):
    """Story ID is logical identity; StoryName binds an exact native filename."""

    def _write_story(self, tmp: Path, relative: str, story_id: str) -> Path:
        target = tmp / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            f"# {story_id}-title\n\n## 元信息\n\n- Story ID：{story_id}\n",
            encoding="utf-8",
        )
        return target

    def test_exact_formal_story_name_resolves_and_validates_metadata(self):
        tmp = _setup_project()
        target = self._write_story(
            tmp,
            "document/project/design/story/be/"
            "cs-ai-story-006-门店推荐对接与列表接口-BE.md",
            "STORY-006-BE",
        )

        result = document_storage.resolve_story_document(
            tmp,
            story_id="STORY-006-BE",
            story_name="cs-ai-story-006-门店推荐对接与列表接口-BE.md",
        )

        self.assertEqual(result.path, target.resolve())
        self.assertEqual(result.story_name, "cs-ai-story-006-门店推荐对接与列表接口-BE")
        self.assertEqual(result.source, "story-name")
        self.assertEqual(result.rejected, ())

    def test_canonical_id_only_filename_remains_backward_compatible(self):
        tmp = _setup_project()
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-001.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("# legacy story without metadata\n", encoding="utf-8")

        result = document_storage.resolve_story_document(
            tmp, story_id="STORY-001"
        )

        self.assertEqual(result.path, target.resolve())
        self.assertEqual(result.source, "canonical-id")

    def test_formal_name_metadata_mismatch_is_rejected(self):
        tmp = _setup_project()
        target = self._write_story(
            tmp, "design/cs-ai-story-006-title-BE.md", "STORY-007-BE"
        )

        result = document_storage.resolve_story_document(
            tmp,
            story_id="STORY-006-BE",
            story_name="cs-ai-story-006-title-BE",
        )

        self.assertIsNone(result.path)
        self.assertEqual(result.source, "none")
        self.assertEqual(result.rejected[0]["code"], "STORY_DOC_ID_MISMATCH")
        self.assertEqual(Path(result.rejected[0]["path"]), target.resolve())

    def test_same_exact_story_name_in_multiple_locations_is_ambiguous(self):
        tmp = _setup_project()
        name = "cs-ai-story-006-title-BE.md"
        first = self._write_story(tmp, f"design/a/{name}", "STORY-006-BE")
        second = self._write_story(tmp, f"design/b/{name}", "STORY-006-BE")

        with self.assertRaises(document_storage.StoryDocumentAmbiguousError) as ctx:
            document_storage.resolve_story_document(
                tmp,
                story_id="STORY-006-BE",
                story_name=name,
            )

        self.assertEqual(ctx.exception.code, "STORY_DOC_AMBIGUOUS")
        self.assertEqual(
            {Path(path) for path in ctx.exception.candidates},
            {first.resolve(), second.resolve()},
        )

    def test_bound_path_has_priority_and_is_revalidated(self):
        tmp = _setup_project()
        bound = self._write_story(
            tmp, "design/current/cs-ai-story-006-title-BE.md", "STORY-006-BE"
        )
        self._write_story(
            tmp, "design/archive/cs-ai-story-006-title-BE.md", "STORY-006-BE"
        )

        result = document_storage.resolve_story_document(
            tmp,
            story_id="STORY-006-BE",
            story_name="cs-ai-story-006-title-BE",
            bound_path=str(bound),
        )

        self.assertEqual(result.path, bound.resolve())
        self.assertEqual(result.source, "bound-path")

    def test_bound_path_outside_project_and_doc_workspace_is_rejected(self):
        tmp = _setup_project()
        outside = Path(tempfile.mkdtemp()) / "cs-ai-story-006-title-BE.md"
        outside.write_text(
            "# story\n\n- Story ID：STORY-006-BE\n", encoding="utf-8"
        )

        result = document_storage.resolve_story_document(
            tmp,
            story_id="STORY-006-BE",
            story_name="cs-ai-story-006-title-BE",
            bound_path=str(outside),
        )

        self.assertIsNone(result.path)
        self.assertEqual(result.rejected[0]["code"], "STORY_DOC_OUTSIDE_ROOTS")

    def test_story_name_symlink_outside_search_roots_is_rejected(self):
        tmp = _setup_project()
        outside = Path(tempfile.mkdtemp()) / "cs-ai-story-006-title-BE.md"
        outside.write_text(
            "# story\n\n- Story ID：STORY-006-BE\n", encoding="utf-8"
        )
        link = tmp / "design" / outside.name
        link.parent.mkdir(parents=True, exist_ok=True)
        try:
            link.symlink_to(outside)
        except OSError:
            # Windows may deny symlink creation. Inject only the enumerated
            # candidate and prove the resolver rejects it before reading it.
            with patch.object(
                document_storage, "_exact_story_name_candidates", return_value=[outside]
            ), patch.object(
                document_storage,
                "_story_candidate_rejection",
                side_effect=AssertionError("outside candidate was read"),
            ):
                result = document_storage.resolve_story_document(
                    tmp,
                    story_id="STORY-006-BE",
                    story_name="cs-ai-story-006-title-BE",
                )
        else:
            result = document_storage.resolve_story_document(
                tmp,
                story_id="STORY-006-BE",
                story_name="cs-ai-story-006-title-BE",
            )

        self.assertIsNone(result.path)
        self.assertEqual(result.rejected[0]["code"], "STORY_DOC_OUTSIDE_ROOTS")

    def test_bound_path_basename_drift_is_rejected(self):
        tmp = _setup_project()
        bound = self._write_story(
            tmp, "design/a-different-name.md", "STORY-006-BE"
        )

        result = document_storage.resolve_story_document(
            tmp,
            story_id="STORY-006-BE",
            story_name="cs-ai-story-006-title-BE",
            bound_path=str(bound),
        )

        self.assertIsNone(result.path)
        self.assertEqual(result.rejected[0]["code"], "STORY_DOC_NAME_MISMATCH")

    def test_story_name_rejects_path_fragments(self):
        tmp = _setup_project()
        for invalid in ("../story", "folder/story", r"folder\story"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(document_storage.StoryDocumentNameInvalidError) as ctx:
                    document_storage.resolve_story_document(
                        tmp,
                        story_id="STORY-006-BE",
                        story_name=invalid,
                    )
                self.assertEqual(ctx.exception.code, "STORY_DOC_NAME_INVALID")

    def test_story_name_rejects_glob_metacharacters_before_search(self):
        tmp = _setup_project()
        for invalid in ("*", "a?", "[ab]", "x]"):
            with self.subTest(invalid=invalid), patch.object(
                document_storage,
                "_exact_story_name_candidates",
                side_effect=AssertionError("glob-capable search must not run"),
            ):
                with self.assertRaises(document_storage.StoryDocumentNameInvalidError) as ctx:
                    document_storage.resolve_story_document(
                        tmp, story_id="STORY-006-BE", story_name=invalid
                    )
                self.assertEqual(ctx.exception.code, "STORY_DOC_NAME_INVALID")

    def test_id_only_fallback_never_selects_task_or_coding_artifacts(self):
        tmp = _setup_project()
        for relative in (
            "ae-sdd-doc/Task/STORY-001/STORY-001.md",
            "ae-sdd-doc/Coding/STORY-001/STORY-001.md",
        ):
            target = tmp / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("# not a Story document\n", encoding="utf-8")

        result = document_storage.resolve_story_document(tmp, story_id="STORY-001")

        self.assertIsNone(result.path)
        self.assertEqual(result.source, "none")

    def test_id_only_story_candidates_across_search_roots_are_ambiguous(self):
        tmp = _setup_project()
        doc_workspace = Path(tempfile.mkdtemp())
        (tmp / ".ae-sdd" / "assets" / "test.assets.md").write_text(
            f"# assets\n\n| gitPath | `{tmp}` |\n"
            f"| docWorkspacePath | `{doc_workspace}` |\n",
            encoding="utf-8",
        )
        first = tmp / "ae-sdd-doc" / "Story" / "STORY-001.md"
        second = doc_workspace / "ae-sdd-doc" / "Story" / "STORY-001.md"
        for target in (first, second):
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("# legacy Story\n", encoding="utf-8")

        with self.assertRaises(document_storage.StoryDocumentAmbiguousError) as ctx:
            document_storage.resolve_story_document(tmp, story_id="STORY-001")

        self.assertEqual(
            {Path(value) for value in ctx.exception.candidates},
            {first.resolve(), second.resolve()},
        )


class TestRaPrerequisites(unittest.TestCase):
    """G-RA-COMPLETE 章节锚校验（C4 修复：标题行正则匹配，非松散子串）。"""

    def _full_ra_content(self) -> str:
        """构造含全部 12 必填章节标题 + 5问的合法 RA 文档。"""
        return (
            "5问自检 通过率100%\n\n"
            "## 0.5 RequirementAnalysisModel 决策记录\n\n"
            "## 0.6 需求风险预判\n\n"
            "## 2. 角色分析\n\n"
            "## 3. 场景分析\n\n"
            "## 4. 业务流程\n\n"
            "## 5. 数据要素\n\n"
            "## 6. 业务规则与约束\n\n"
            "## 7. 设计方向论证\n\n"
            "## 8. 验收标准雏形\n\n"
            "## 9. 隐性假设与验证\n\n"
            "## 10. 缺口与未决问题\n\n"
            "## 11. 规模裁定\n\n"
            + "\n".join(f"RA-G0{i}: PASS" for i in range(1, 9))
        )

    def test_full_ra_passes_all_gates(self):
        """含全部章节的 RA 文档应通过 G-RA-COMPLETE（标题行匹配）。"""
        document_storage.check_ra_prerequisites(self._full_ra_content())  # 不抛即通过

    def test_missing_section_blocked(self):
        """缺章节标题的文档应被 G-RA-COMPLETE 拦截。"""
        content = self._full_ra_content().replace("## 2. 角色分析\n\n", "")
        with self.assertRaises(document_storage.DocStorageError) as ctx:
            document_storage.check_ra_prerequisites(content)
        self.assertIn("G-RA-COMPLETE", str(ctx.exception))
        self.assertIn("§2 角色", str(ctx.exception))

    def test_section_in_body_prose_does_not_pass(self):
        """章节名出现在正文而非标题行时不应算作章节存在（收紧子串误判）。"""
        # 把所有 ## 标题降级成普通正文行（不以 # 开头）
        content = self._full_ra_content().replace("## ", "章节：")
        with self.assertRaises(document_storage.DocStorageError):
            document_storage.check_ra_prerequisites(content)

    def test_template_itself_passes(self):
        """ra-template.md 自身应通过结构门禁。"""
        import sys as _sys
        from pathlib import Path as _Path
        tmpl = (_Path(__file__).resolve().parent.parent.parent
                / "source" / "templates" / "design" / "ra-template.md")
        if not tmpl.is_file():
            self.skipTest("ra-template.md not found")
        text = tmpl.read_text(encoding="utf-8")
        document_storage.check_ra_prerequisites(text)  # 不抛即通过


if __name__ == "__main__":
    unittest.main(verbosity=2)
