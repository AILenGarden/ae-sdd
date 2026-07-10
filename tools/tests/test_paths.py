"""
test_paths.py — paths.py 单元测试

覆盖：
- locate_master_source（4 个候选路径）
- locate_project_ae_sdd（向上 5 级查找）
- read_config（YAML 极简解析）
- find_doc / list_docs（多位置查找）
"""
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

# 让 import lib 找得到
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import paths  # noqa: E402


class TestLocateMasterSource(unittest.TestCase):
    """locate_master_source 测试"""

    def setUp(self):
        self.tmp = tempfile.mkdtemp()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_env_var_priority(self):
        """环境变量优先级最高"""
        # 构造一个"假母版"
        fake_master = Path(self.tmp) / "fake_master"
        fake_master.mkdir(parents=True)  # 关键：先建父目录
        (fake_master / "SKILL.md").write_text("# Fake", encoding="utf-8")
        try:
            import os
            old = os.environ.get("AE_SDD_MASTER")
            os.environ["AE_SDD_MASTER"] = str(fake_master)
            result = paths.locate_master_source()
            self.assertEqual(result, fake_master)
        finally:
            if old:
                os.environ["AE_SDD_MASTER"] = old
            else:
                os.environ.pop("AE_SDD_MASTER", None)

    @unittest.skip("__file__ 总指向 ae-sdd 母版，cli_path 候选无法被移除")
    def test_skips_dirs_without_skill_md(self):
        """没 SKILL.md 的目录不算母版（验证 locate_master_source 不返回无 SKILL.md 的目录）

        注：locate_master_source() 始终包含 cli_path.parent.parent.parent（即 ae-sdd 母版）
        作为候选，无法在测试环境让它返回 None。
        """
        import os
        from pathlib import Path
        old_cwd = os.getcwd()
        tmp_path = Path(self.tmp)
        try:
            os.chdir(self.tmp)
            os.environ["AE_SDD_MASTER"] = str(tmp_path / "no_such")
            result = paths.locate_master_source()
            self.assertIsNone(result)
        finally:
            os.chdir(old_cwd)
            os.environ.pop("AE_SDD_MASTER", None)


class TestLocateProjectAeSdd(unittest.TestCase):
    """locate_project_ae_sdd 测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_finds_in_current_dir(self):
        (self.tmp / ".ae-sdd").mkdir()
        (self.tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test", encoding="utf-8")
        result = paths.locate_project_ae_sdd(self.tmp)
        self.assertEqual(result, self.tmp / ".ae-sdd")

    def test_finds_in_parent(self):
        (self.tmp / ".ae-sdd").mkdir()
        (self.tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test", encoding="utf-8")
        sub = self.tmp / "sub" / "deeper"
        sub.mkdir(parents=True)
        result = paths.locate_project_ae_sdd(sub)
        self.assertEqual(result, self.tmp / ".ae-sdd")

    def test_returns_none_when_no_ae_sdd(self):
        result = paths.locate_project_ae_sdd(self.tmp)
        self.assertIsNone(result)


class TestReadConfig(unittest.TestCase):
    """read_config YAML 极简解析测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ade_sdd = self.tmp / ".ae-sdd"
        self.ade_sdd.mkdir()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_simple_keys(self):
        (self.ade_sdd / "config.yaml").write_text(textwrap.dedent("""
            version: 1
            projectKey: icec-cloud-boss
            gitPath: D:/Item/icec-cloud-boss
        """).strip(), encoding="utf-8")
        cfg = paths.read_config(self.ade_sdd)
        # 极简解析器：所有值都是 str
        self.assertEqual(cfg["version"], "1")
        self.assertEqual(cfg["projectKey"], "icec-cloud-boss")
        self.assertEqual(cfg["gitPath"], "D:/Item/icec-cloud-boss")

    def test_nested_section(self):
        (self.ade_sdd / "config.yaml").write_text(textwrap.dedent("""
            version: 1
            projectKey: test
            master:
              source: ../ae-sdd
              version: "3.0.0"
        """).strip(), encoding="utf-8")
        cfg = paths.read_config(self.ade_sdd)
        self.assertIn("master", cfg)
        self.assertEqual(cfg["master"]["source"], "../ae-sdd")
        self.assertEqual(cfg["master"]["version"], "3.0.0")

    def test_comments_ignored(self):
        (self.ade_sdd / "config.yaml").write_text(textwrap.dedent("""
            # This is a comment
            version: 1  # inline comment
            projectKey: test
        """).strip(), encoding="utf-8")
        cfg = paths.read_config(self.ade_sdd)
        self.assertEqual(cfg["projectKey"], "test")

    def test_hash_inside_quotes_preserved(self):
        """引号内的 # 不应被当作注释剥离（C7 修复）。"""
        (self.ade_sdd / "config.yaml").write_text(
            'desc: "see #123 for details"  # real comment\n',
            encoding="utf-8")
        cfg = paths.read_config(self.ade_sdd)
        self.assertEqual(cfg["desc"], "see #123 for details")

    def test_empty_config(self):
        # config.yaml 不存在 → 返回空 dict
        cfg = paths.read_config(self.ade_sdd)
        self.assertEqual(cfg, {})


class TestFindDoc(unittest.TestCase):
    """find_doc / list_docs 测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.design = self.tmp / "design"
        self.task = self.tmp / "task"
        self.design.mkdir()
        self.task.mkdir()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_find_in_design_dir(self):
        (self.design / "STORY-001.md").write_text("# Story", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", ".md")
        self.assertEqual(result, self.design / "STORY-001.md")

    def test_find_in_project_root(self):
        # 不在 design/，在项目根
        (self.tmp / "STORY-001.md").write_text("# Story", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", ".md")
        self.assertEqual(result, self.tmp / "STORY-001.md")

    def test_find_returns_none_when_missing(self):
        result = paths.find_doc(self.tmp, "STORY-001", ".md")
        self.assertIsNone(result)

    def test_list_docs_finds_all_tasks(self):
        (self.task / "STORY-001-task-001.md").write_text("# t1", encoding="utf-8")
        (self.task / "STORY-001-task-002.md").write_text("# t2", encoding="utf-8")
        (self.task / "OTHER-001-task-001.md").write_text("# other", encoding="utf-8")
        result = paths.list_docs(self.tmp, "STORY-001", "-task-*.md")
        self.assertEqual(len(result), 2)
        self.assertIn(self.task / "STORY-001-task-001.md", result)
        self.assertIn(self.task / "STORY-001-task-002.md", result)

    def test_list_docs_empty_when_no_task_dir(self):
        import shutil
        shutil.rmtree(self.task)
        result = paths.list_docs(self.tmp, "STORY-001", "-task-*.md")
        self.assertEqual(result, [])

    # ─── 🆕 v3.9.10：document-storage 新布局 ae-sdd-doc/ 覆盖 ──────────────────
    def test_find_in_ae_sdd_doc_story(self):
        """新布局：ae-sdd-doc/Story/{docId}.md 可命中（STORY intent）。"""
        doc = self.tmp / "ae-sdd-doc" / "Story" / "STORY-001.md"
        doc.parent.mkdir(parents=True, exist_ok=True)
        doc.write_text("# Story", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", ".md")
        self.assertEqual(result, doc)

    def test_find_in_ae_sdd_doc_testcase(self):
        """新布局：ae-sdd-doc/Test/{workItem}/{workItem}-testcase.md 可命中（TESTCASE intent）。"""
        doc = self.tmp / "ae-sdd-doc" / "Test" / "STORY-001" / "STORY-001-testcase.md"
        doc.parent.mkdir(parents=True, exist_ok=True)
        doc.write_text("# TC", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", "-testcase.md")
        self.assertEqual(result, doc)

    def test_find_in_ae_sdd_doc_codingplan(self):
        """新布局：ae-sdd-doc/Coding/{workItem}/{workItem}-CodingPlan.md 可命中（CODING_PLAN intent）。"""
        doc = self.tmp / "ae-sdd-doc" / "Coding" / "STORY-001" / "STORY-001-CodingPlan.md"
        doc.parent.mkdir(parents=True, exist_ok=True)
        doc.write_text("# CP", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", "-CodingPlan.md")
        self.assertEqual(result, doc)

    def test_find_in_ae_sdd_doc_iterations(self):
        """新布局：iterations/ 迭代目录下文档可命中（TASK_SMALL/PLAN_MICRO 路径）。"""
        doc = (self.tmp / "ae-sdd-doc" / "iterations" / "2026-07-08"
               / "Coding" / "STORY-001" / "STORY-001-CodingPlan.md")
        doc.parent.mkdir(parents=True, exist_ok=True)
        doc.write_text("# CP iter", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", "-CodingPlan.md")
        self.assertEqual(result, doc)

    def test_find_legacy_design_preferred_over_new(self):
        """旧 design/ 与新 ae-sdd-doc/ 同时存在时优先返回旧路径（保持历史项目行为）。"""
        (self.design / "STORY-001.md").write_text("# Story legacy", encoding="utf-8")
        new_doc = self.tmp / "ae-sdd-doc" / "Story" / "STORY-001.md"
        new_doc.parent.mkdir(parents=True, exist_ok=True)
        new_doc.write_text("# Story new", encoding="utf-8")
        result = paths.find_doc(self.tmp, "STORY-001", ".md")
        self.assertEqual(result, self.design / "STORY-001.md")

    def test_list_docs_finds_tasks_in_ae_sdd_doc(self):
        """新布局：ae-sdd-doc/Task/{story_id}/ 下 Task 文档可被 list_docs 列出。"""
        new_task_dir = self.tmp / "ae-sdd-doc" / "Task" / "STORY-001"
        new_task_dir.mkdir(parents=True, exist_ok=True)
        (new_task_dir / "STORY-001-task-001.md").write_text("# t1 new", encoding="utf-8")
        (new_task_dir / "STORY-001-task-002.md").write_text("# t2 new", encoding="utf-8")
        result = paths.list_docs(self.tmp, "STORY-001", "-task-*.md")
        names = sorted(p.name for p in result)
        self.assertEqual(names, ["STORY-001-task-001.md", "STORY-001-task-002.md"])

    def test_list_docs_dedupes_when_docworkspace_equals_project_root(self):
        """docWorkspacePath 回退 gitPath（= 项目根）时搜索根重合，list_docs 不应重复返回同一文件。"""
        # 构造 .ae-sdd/assets 声明 gitPath=项目根、无 docWorkspacePath（回退 gitPath）
        assets = self.tmp / ".ae-sdd" / "assets"
        assets.mkdir(parents=True, exist_ok=True)
        (assets / "test.assets.md").write_text(
            f"# §A §B §C §D §E §F §G\n\n| gitPath | `{self.tmp}` |\n",
            encoding="utf-8",
        )
        (self.tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        (self.task / "STORY-001-task-001.md").write_text("# t1", encoding="utf-8")
        result = paths.list_docs(self.tmp, "STORY-001", "-task-*.md")
        # doc_search_roots 去重后仅 1 个根，task-001 只被遍历一次
        self.assertEqual(len(result), 1)

    def test_doc_search_roots_returns_project_only_without_ae_sdd(self):
        """无 .ae-sdd/config.yaml 时仅返回 [project_dir]（向后兼容）。"""
        roots = paths.doc_search_roots(self.tmp)
        self.assertEqual(roots, [self.tmp])

    def test_doc_search_roots_dedupes_when_docworkspace_equals_gitpath(self):
        """docWorkspacePath 缺省回退 gitPath 时，搜索根去重为 1 个。"""
        assets = self.tmp / ".ae-sdd" / "assets"
        assets.mkdir(parents=True, exist_ok=True)
        (assets / "test.assets.md").write_text(
            f"# §A §B §C §D §E §F §G\n\n| gitPath | `{self.tmp}` |\n",
            encoding="utf-8",
        )
        (self.tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        roots = paths.doc_search_roots(self.tmp)
        self.assertEqual(roots, [self.tmp])


class TestPathHelpers(unittest.TestCase):
    """路径辅助函数测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ade_sdd = self.tmp / ".ae-sdd"
        self.ade_sdd.mkdir()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_project_root(self):
        self.assertEqual(paths.project_root(self.ade_sdd), self.tmp)

    def test_project_design_dir(self):
        self.assertEqual(paths.project_design_dir(self.tmp), self.tmp / "design")

    def test_project_task_dir(self):
        self.assertEqual(paths.project_task_dir(self.tmp), self.tmp / "task")

    def test_state_path(self):
        self.assertEqual(paths.state_path(self.ade_sdd), self.ade_sdd / "state.json")

    def test_work_item_dir_name_v393_r6_only(self):
        """🆕 v3.9.3 废除 v3.8.2 双段：work_item_dir_name 只接受 (top_node, features) 走 R6 顶层名。"""
        # 4 种顶层节点
        self.assertEqual(paths.work_item_dir_name("PRD", {"prd_feature": "IM-CS"}), "PRD-IM-CS")
        self.assertEqual(paths.work_item_dir_name("DR", {"dr_feature": "CS"}), "DR-CS")
        self.assertEqual(paths.work_item_dir_name("STORY", {"story_ids": ["STORY-003-BE", "STORY-004-BE"]}), "Story-003-004")
        self.assertEqual(paths.work_item_dir_name("TASK", {"task_id": "BUG-LIFE-001"}), "Task-BUG-LIFE-001")

    def test_work_item_dir_name_v393_no_legacy_args(self):
        """v3.9.3 旧 (id, name) 双参调用 → 走新签名但参数语义变了，等价 (top_node, features) 不传 name。"""
        # 旧 v3.8.2 调用方式: work_item_dir_name("STORY-004-BE", "登录超时")
        # v3.9.3 后第 1 个参数被当作 top_node="STORY-004-BE"（非法），抛 ValueError
        with self.assertRaises(ValueError):
            paths.work_item_dir_name("STORY-004-BE", {"story_ids": ["STORY-004-BE"]})

    def test_work_item_state_path_v393_r6(self):
        """v3.9.3 state.json 走 R6 顶层目录。"""
        expected = self.tmp / ".auto-engineering" / "Story-003-004" / "state.json"
        actual = paths.work_item_state_path(self.ade_sdd, "STORY", {"story_ids": ["STORY-003-BE", "STORY-004-BE"]})
        self.assertEqual(actual, expected)

    def test_find_work_item_state_path_accepts_existing_legacy_key(self):
        legacy_key = "STORY-004-BE--车主端预约单操作-BE"
        state_file = self.tmp / ".auto-engineering" / legacy_key / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        state_file.write_text('{"workItemKey":"%s"}\n' % legacy_key, encoding="utf-8")

        self.assertEqual(paths.find_work_item_state_path(self.ade_sdd, legacy_key), state_file)

    def test_find_work_item_state_path_matches_nested_active_story(self):
        """Nested state lookup must not depend on legacy currentStory being present."""
        state_file = self.tmp / ".auto-engineering" / "custom-state" / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        state_file.write_text(
            '{"stateModel":"nested","activeStory":"STORY-004-BE",'
            '"storyStates":{"STORY-004-BE":{"phase":"story-generated"}}}\n',
            encoding="utf-8",
        )

        self.assertEqual(paths.find_work_item_state_path(self.ade_sdd, "STORY-004-BE"), state_file)

    def test_assets_dir(self):
        self.assertEqual(paths.assets_dir(self.ade_sdd), self.ade_sdd / "assets")

    def test_overrides_dir(self):
        self.assertEqual(paths.overrides_dir(self.ade_sdd), self.ade_sdd / "overrides")

    def test_reports_dir(self):
        self.assertEqual(paths.reports_dir(self.ade_sdd), self.ade_sdd / "reports")


class TestAssetFieldAndModuleFiles(unittest.TestCase):
    """🆕 v4.0：read_asset_field / resolve_doc_workspace / find_module_asset_files 测试。"""

    def setUp(self):
        import tempfile
        self.tmp = Path(tempfile.mkdtemp(prefix="paths-v4-"))
        self.ade_sdd = self.tmp / ".ae-sdd"
        self.ade_sdd.mkdir()
        self.assets = self.ade_sdd / "assets"
        self.assets.mkdir()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_overview(self, project_key, git_path=None, doc_ws=None):
        """写一个含 §1 字段的总览资产。"""
        fields = [f"| projectKey | `{project_key}` |"]
        if git_path:
            fields.append(f"| gitPath | `{git_path}` |")
        if doc_ws:
            fields.append(f"| docWorkspacePath | `{doc_ws}` |")
        content = "# §A\n## §1\n" + "\n".join(fields) + "\n## §B\n## §C\n## §D\n## §E\n## §F\n## §G\n"
        (self.assets / f"{project_key}.assets.md").write_text(content, encoding="utf-8")

    def test_read_asset_field_markdown_table(self):
        self._write_overview("proj1", git_path=r"d:\proj1")
        val = paths.read_asset_field(self.ade_sdd, "proj1", "gitPath")
        self.assertEqual(val, r"d:\proj1")

    def test_read_asset_field_missing_returns_none(self):
        self._write_overview("proj1")
        # 无 docWorkspacePath
        val = paths.read_asset_field(self.ade_sdd, "proj1", "docWorkspacePath")
        self.assertIsNone(val)

    def test_read_asset_field_no_asset_file(self):
        val = paths.read_asset_field(self.ade_sdd, "nonexistent", "gitPath")
        self.assertIsNone(val)

    def test_resolve_doc_workspace_fallback_to_gitpath(self):
        self._write_overview("proj1", git_path=r"d:\proj1")
        ws = paths.resolve_doc_workspace(self.ade_sdd, "proj1")
        self.assertEqual(ws, Path(r"d:\proj1"))

    def test_resolve_doc_workspace_prefers_docws(self):
        self._write_overview("proj1", git_path=r"d:\proj1", doc_ws=r"d:\docs")
        ws = paths.resolve_doc_workspace(self.ade_sdd, "proj1")
        self.assertEqual(ws, Path(r"d:\docs"))

    def test_find_module_asset_files_flat_compat(self):
        """旧扁平位置（.ae-sdd/assets/{key}.{module}.assets.md）能被发现。"""
        self._write_overview("proj1")
        (self.assets / "proj1.module-a.assets.md").write_text("# module a\n", encoding="utf-8")
        (self.assets / "proj1.module-b.assets.md").write_text("# module b\n", encoding="utf-8")
        result = paths.find_module_asset_files(self.ade_sdd, "proj1")
        names = [p.name for p in result]
        self.assertIn("proj1.assets.md", names)  # 总览
        self.assertIn("proj1.module-a.assets.md", names)
        self.assertIn("proj1.module-b.assets.md", names)

    def test_find_module_asset_files_empty_when_no_overview(self):
        """无总览时返回空列表。"""
        result = paths.find_module_asset_files(self.ade_sdd, "nonexistent")
        self.assertEqual(result, [])

    def test_find_module_asset_files_only_overview(self):
        """只有总览无子文件时，返回 [总览]。"""
        self._write_overview("proj1")
        result = paths.find_module_asset_files(self.ade_sdd, "proj1")
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].name, "proj1.assets.md")

    # ─── 🆕 v4.1：line 分组发现 + 三阶段共存 ──────────────────────────────────

    def test_resolve_assets_base_prefers_docws(self):
        """resolve_assets_base 优先用 docWorkspacePath/assets/{key}。"""
        self._write_overview("proj1", doc_ws=str(self.tmp / "docs"))
        base = paths.resolve_assets_base(self.ade_sdd, "proj1")
        self.assertEqual(base, self.tmp / "docs" / ".ae-sdd" / "assets" / "proj1")

    def test_resolve_assets_base_fallback_gitpath(self):
        """无 docWorkspacePath 时回退 gitPath/assets/{key}。"""
        self._write_overview("proj1", git_path=str(self.tmp / "repo"))
        base = paths.resolve_assets_base(self.ade_sdd, "proj1")
        self.assertEqual(base, self.tmp / "repo" / ".ae-sdd" / "assets" / "proj1")

    def test_resolve_assets_base_none_when_no_overview(self):
        """无总览时 resolve_assets_base 返回 None。"""
        self.assertIsNone(paths.resolve_assets_base(self.ade_sdd, "ghost"))

    def test_find_module_asset_files_line_group_discovery(self):
        """阶段①：line 分组 {key}/{line}/{module}/{module}.assets.md 能被发现。"""
        doc_ws = self.tmp / "docs"
        base = doc_ws / ".ae-sdd" / "assets" / "proj1"
        # 构造 2c/admin 两条 line，各含 1 个 module
        cs_mod = base / "2c" / "icec-cloud-life-cs"
        user_mod = base / "admin" / "icec-cloud-boss-user"
        cs_mod.mkdir(parents=True)
        user_mod.mkdir(parents=True)
        (cs_mod / "icec-cloud-life-cs.assets.md").write_text("# cs\n", encoding="utf-8")
        (user_mod / "icec-cloud-boss-user.assets.md").write_text("# user\n", encoding="utf-8")
        self._write_overview("proj1", doc_ws=str(doc_ws))

        result = paths.find_module_asset_files(self.ade_sdd, "proj1")
        names = [p.name for p in result]
        self.assertIn("proj1.assets.md", names)  # 总览在前
        self.assertIn("icec-cloud-life-cs.assets.md", names)
        self.assertIn("icec-cloud-boss-user.assets.md", names)
        # line 按名字典序排序：'2c' < 'admin'（'2' ASCII 50 < 'a' 97），故 2c 的 module 在前
        self.assertLess(names.index("icec-cloud-life-cs.assets.md"),
                        names.index("icec-cloud-boss-user.assets.md"))

    def test_find_module_asset_files_flat_module_still_works(self):
        """阶段②：单层 module {key}/{module}/{module}.assets.md 仍被发现（v4.0 兼容）。"""
        doc_ws = self.tmp / "docs"
        base = doc_ws / ".ae-sdd" / "assets" / "proj1"
        mod = base / "svc-a"
        mod.mkdir(parents=True)
        (mod / "svc-a.assets.md").write_text("# svc a\n", encoding="utf-8")
        self._write_overview("proj1", doc_ws=str(doc_ws))

        result = paths.find_module_asset_files(self.ade_sdd, "proj1")
        names = [p.name for p in result]
        self.assertIn("proj1.assets.md", names)
        self.assertIn("svc-a.assets.md", names)

    def test_find_module_asset_files_mixed_line_and_flat(self):
        """混合：同一 base 下既有 line 分组又有单层 module（两者都发现）。"""
        doc_ws = self.tmp / "docs"
        base = doc_ws / ".ae-sdd" / "assets" / "proj1"
        # 单层 module
        flat_mod = base / "standalone-svc"
        flat_mod.mkdir(parents=True)
        (flat_mod / "standalone-svc.assets.md").write_text("# flat\n", encoding="utf-8")
        # line 分组 module
        line_mod = base / "2c" / "life-cs"
        line_mod.mkdir(parents=True)
        (line_mod / "life-cs.assets.md").write_text("# line\n", encoding="utf-8")
        self._write_overview("proj1", doc_ws=str(doc_ws))

        result = paths.find_module_asset_files(self.ade_sdd, "proj1")
        names = [p.name for p in result]
        self.assertIn("proj1.assets.md", names)
        self.assertIn("standalone-svc.assets.md", names)
        self.assertIn("life-cs.assets.md", names)
        # 单层 module 在阶段②，line 在阶段①之后；二者都应被发现（顺序不重叠即可）

    def test_find_module_asset_files_three_stages_coexist(self):
        """阶段①②③ 三者共存：line 分组 + 单层 module + 旧扁平，全部被发现且不重复。"""
        doc_ws = self.tmp / "docs"
        base = doc_ws / ".ae-sdd" / "assets" / "proj1"
        # 阶段① line 分组
        line_mod = base / "2c" / "life-cs"
        line_mod.mkdir(parents=True)
        (line_mod / "life-cs.assets.md").write_text("# line\n", encoding="utf-8")
        # 阶段② 单层 module
        flat_mod = base / "standalone"
        flat_mod.mkdir(parents=True)
        (flat_mod / "standalone.assets.md").write_text("# flat\n", encoding="utf-8")
        # 阶段③ 旧扁平
        self._write_overview("proj1", doc_ws=str(doc_ws))
        (self.assets / "proj1.legacy-mod.assets.md").write_text("# legacy\n", encoding="utf-8")

        result = paths.find_module_asset_files(self.ade_sdd, "proj1")
        names = [p.name for p in result]
        self.assertEqual(names[0], "proj1.assets.md")  # 总览恒首位
        self.assertIn("life-cs.assets.md", names)
        self.assertIn("standalone.assets.md", names)
        self.assertIn("proj1.legacy-mod.assets.md", names)
        # 无重复
        self.assertEqual(len(names), len(set(names)))

    def test_discover_line_groups_classifies_correctly(self):
        """discover_line_groups 正确区分 module 目录与 line 目录。"""
        base = self.tmp / "docs" / ".ae-sdd" / "assets" / "proj1"
        # module 目录（含同名 .md）
        m1 = base / "svc-a"
        m1.mkdir(parents=True)
        (m1 / "svc-a.assets.md").write_text("x", encoding="utf-8")
        # line 目录（孙级含 module）
        m2 = base / "2c" / "life-cs"
        m2.mkdir(parents=True)
        (m2 / "life-cs.assets.md").write_text("y", encoding="utf-8")
        # 无关空目录（不应被识别为任何一类）
        (base / "empty-dir").mkdir()

        discovered = paths.discover_line_groups(base)
        flat_names = [p.parent.name for p in discovered["flat_modules"]]
        self.assertEqual(flat_names, ["svc-a"])
        self.assertIn("2c", discovered["line_groups"])
        self.assertEqual([p.parent.name for p in discovered["line_groups"]["2c"]], ["life-cs"])


# ─── 🆕 v3.9.3 父级文档字段抽取 + 关联性验证 ───────────────────────────────────
class TestExtractParentClaim(unittest.TestCase):

    def _tmp_doc(self, content: str) -> Path:
        import tempfile
        p = Path(tempfile.mkdtemp(prefix="story-")) / "STORY-006-BE-Story.md"
        p.write_text(content, encoding="utf-8")
        return p

    def test_story_chinese_claim(self):
        """Story 模板中文字段：'- 来源 PRD: PRD-001' / '- 来源 DR: DR-005'"""
        doc = self._tmp_doc(
            "# STORY-006\n\n## 元信息\n\n- Story ID：STORY-006-BE\n"
            "- 来源 PRD：PRD-001\n- 来源 DR：DR-005\n- 优先级：P1\n"
        )
        prd, dr = paths.extract_parent_claim(doc, doc_kind="story")
        self.assertEqual(prd, "PRD-001")
        self.assertEqual(dr, "DR-005")

    def test_dr_chinese_claim(self):
        """DR 模板字段：'- PRD: PRD-001' / '- DR ID: DR-005'"""
        doc = self._tmp_doc(
            "# DR-005\n\n## 元信息\n\n- DR ID：DR-005\n- PRD：PRD-001\n- 状态：Draft\n"
        )
        prd, dr = paths.extract_parent_claim(doc, doc_kind="dr")
        self.assertEqual(prd, "PRD-001")
        # DR 文档不应返回自己的 DR ID 作为 parent_dr
        self.assertIsNone(dr)

    def test_missing_fields(self):
        """字段缺失 → 返回 (None, None)"""
        doc = self._tmp_doc("# STORY-X\n\n## 元信息\n\n- Story ID: STORY-X\n- 优先级: P1\n")
        prd, dr = paths.extract_parent_claim(doc, doc_kind="story")
        self.assertIsNone(prd)
        self.assertIsNone(dr)

    def test_nonexistent_file(self):
        """文档不存在 → (None, None)"""
        import tempfile
        nonexistent = Path(tempfile.gettempdir()) / "does-not-exist-XYZ.md"
        if nonexistent.exists():
            nonexistent.unlink()
        prd, dr = paths.extract_parent_claim(nonexistent, doc_kind="story")
        self.assertIsNone(prd)
        self.assertIsNone(dr)


class TestVerifyParentClaim(unittest.TestCase):

    def setUp(self):
        import tempfile
        self.tmp = Path(tempfile.mkdtemp(prefix="vpc-"))
        self.design = self.tmp / "design"
        self.design.mkdir()

    def _make_dr(self, dr_id: str, story_ids: list) -> Path:
        body = f"# {dr_id}\n\n## Story 拆分\n\n" + "\n".join(f"- {s}" for s in story_ids) + "\n"
        p = self.design / f"{dr_id}-some-title.md"
        p.write_text(body, encoding="utf-8")
        return p

    def test_dr_doc_exists_relation_ok(self):
        self._make_dr("DR-005", ["STORY-006-BE", "STORY-007-BE"])
        ok, reason = paths.verify_parent_claim("DR", "DR-005", self.design, child_id="STORY-006-BE")
        self.assertTrue(ok)
        self.assertEqual(reason, "ok")

    def test_dr_doc_not_found(self):
        ok, reason = paths.verify_parent_claim("DR", "DR-999", self.design, child_id="STORY-006-BE")
        self.assertFalse(ok)
        self.assertEqual(reason, "doc_not_found")

    def test_dr_doc_exists_relation_mismatch(self):
        self._make_dr("DR-005", ["STORY-999-BE"])
        ok, reason = paths.verify_parent_claim("DR", "DR-005", self.design, child_id="STORY-006-BE")
        self.assertFalse(ok)
        self.assertEqual(reason, "relation_mismatch")

    def test_short_id_fallback_match(self):
        """弱关联：去掉 -BE/-FE 后缀再查"""
        self._make_dr("DR-005", ["STORY-006"])
        ok, reason = paths.verify_parent_claim("DR", "DR-005", self.design, child_id="STORY-006-BE")
        self.assertTrue(ok)

    def test_invalid_args(self):
        ok, reason = paths.verify_parent_claim("XYZ", "DR-005", self.design, child_id="STORY-006")
        self.assertFalse(ok)
        self.assertEqual(reason, "invalid_args")


if __name__ == "__main__":
    unittest.main(verbosity=2)
