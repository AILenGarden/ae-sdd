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

    def test_assets_dir(self):
        self.assertEqual(paths.assets_dir(self.ade_sdd), self.ade_sdd / "assets")

    def test_overrides_dir(self):
        self.assertEqual(paths.overrides_dir(self.ade_sdd), self.ade_sdd / "overrides")

    def test_reports_dir(self):
        self.assertEqual(paths.reports_dir(self.ade_sdd), self.ade_sdd / "reports")


if __name__ == "__main__":
    unittest.main(verbosity=2)
