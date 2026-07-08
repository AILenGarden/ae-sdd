"""🆕 v3.9.3 CLI-level tests for work-item isolated state files.

BREAKING v3.9.3 变更：
  - 废除 v3.8.2 双段（{ID}--{name}）目录命名
  - 废除 --name 形参
  - 目录名统一走 R6 顶层名（PRD-/DR-/Story-/Task-）
  - session.json 与 state.json 共用 R6 顶层目录
  - cmd_state_new 强制 R2 向上归入（递归替父级建 state）

v3.8.2 旧测试已被替换为 v3.9.3 新行为测试。
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CLI = str(Path(__file__).resolve().parent.parent / "bin" / "ae-sdd")


def _setup_project() -> Path:
    tmp = Path(tempfile.mkdtemp())
    (tmp / ".ae-sdd" / "assets").mkdir(parents=True, exist_ok=True)
    (tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: life\n", encoding="utf-8")
    (tmp / ".ae-sdd" / "assets" / "life.assets.md").write_text(
        f"# §A §B §C §D §E §F §G\n\n| gitPath | `{tmp}` |\n| docWorkspacePath | `{tmp}` |\n",
        encoding="utf-8",
    )
    return tmp


def _run_cli(cwd: Path, *args: str) -> tuple[int, str, str]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(Path(__file__).resolve().parent.parent.parent)
    r = subprocess.run(
        [sys.executable, CLI, *args],
        capture_output=True, text=True, cwd=str(cwd), env=env, encoding="utf-8",
    )
    return r.returncode, r.stdout, r.stderr


# ─── 🆕 v3.9.3 新行为测试 ────────────────────────────────────────────────────
class TestCmdStateNewV393(unittest.TestCase):
    """🆕 v3.9.3 cmd_state_new 走 R6 顶层名 + R2 向上归入。"""

    def test_state_new_story_no_name_uses_r6(self):
        """v3.9.3 state new --id STORY-006-BE --entry-node STORY → 顶层 Story-006（无 --name）。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # R6 顶层名 = Story-006
        self.assertEqual(payload["workItemKey"], "Story-006")
        # state.json 落 .auto-engineering/Story-006/state.json
        sp = tmp / ".auto-engineering" / "Story-006" / "state.json"
        self.assertTrue(sp.is_file())

    def test_state_new_dr_no_name_uses_r6(self):
        """v3.9.3 state new --id DR-005 --entry-node DR → 顶层 DR-005。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "DR-005",
            "--entry-node", "DR",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        sp = tmp / ".auto-engineering" / "DR-005" / "state.json"
        self.assertTrue(sp.is_file())

    def test_state_new_task_bug_uses_task_prefix(self):
        """v3.9.3 state new --id BUG-LIFE-001 → 顶层 Task-BUG-LIFE-001（TASK 顶层名）。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "BUG-LIFE-001",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        sp = tmp / ".auto-engineering" / "Task-BUG-LIFE-001" / "state.json"
        self.assertTrue(sp.is_file())

    def test_state_new_legacy_name_flag_warns_and_ignores(self):
        """v3.9.3 --name 形参废除（传了被忽略，不报错）。"""
        tmp = _setup_project()
        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--name", "应被忽略",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        # 不应创建双段目录
        legacy = tmp / ".auto-engineering" / "STORY-006-BE--应被忽略" / "state.json"
        self.assertFalse(legacy.is_file())
        # 仍走 R6
        r6 = tmp / ".auto-engineering" / "Story-006" / "state.json"
        self.assertTrue(r6.is_file())

    def test_state_new_story_with_parent_dr_doc_absorbs(self):
        """v3.9.3 Story 有父级 DR + DR 文档存在关联性对 → Story 嵌进 DR 嵌套 state。"""
        tmp = _setup_project()
        # 创建设计文档
        design = tmp / "design"
        design.mkdir()
        (design / "DR-005-some-title.md").write_text(
            "# DR-005\n\n## Story 拆分\n\n- STORY-006-BE\n- STORY-007-BE\n",
            encoding="utf-8",
        )
        (design / "STORY-006-BE-Story.md").write_text(
            "# STORY-006-BE\n\n## 元信息\n\n- Story ID: STORY-006-BE\n- 来源 DR: DR-005\n",
            encoding="utf-8",
        )
        # 配置 docWorkspacePath
        cfg = (tmp / ".ae-sdd" / "config.yaml")
        cfg.write_text(f"projectKey: life\ndocWorkspacePath: {tmp}\n", encoding="utf-8")

        code, out, err = _run_cli(
            tmp, "state", "new",
            "--id", "STORY-006-BE",
            "--entry-node", "STORY",
            "--story-ids", "STORY-006-BE",
            "--json",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        # Story 嵌进 DR-005 嵌套 state
        self.assertIn("DR-005", payload["statePath"])


# ─── v3.8.2 旧测试全部 SKIPPED（BREAKING 变更）────────────────────────────────
class TestStateActiveMirrorRegression(unittest.TestCase):

    def test_state_read_uses_existing_legacy_active_work_item(self):
        """activeWorkItem may contain an existing v3.8 two-segment directory key."""
        tmp = _setup_project()
        work_item = "STORY-004-BE--车主端预约单操作-BE"
        state_file = tmp / ".auto-engineering" / work_item / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        nested = {
            "version": "2",
            "projectKey": "life",
            "stateModel": "nested",
            "entryNode": "STORY",
            "activeStory": "STORY-004-BE",
            "storyStates": {"STORY-004-BE": {"phase": "story-generated"}},
            "workItemKey": work_item,
        }
        state_file.write_text(json.dumps(nested, ensure_ascii=False), encoding="utf-8")
        mirror = dict(nested)
        mirror["activeWorkItem"] = work_item
        mirror["activeStatePath"] = str(state_file)
        (tmp / ".ae-sdd" / "state.json").write_text(
            json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(tmp, "state", "read", "--json")

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        payload = json.loads(out)
        self.assertEqual(payload["activeStory"], "STORY-004-BE")

    def test_state_write_without_story_updates_nested_active_story(self):
        """state write should advance activeStory in nested legacy work-item states."""
        tmp = _setup_project()
        work_item = "STORY-004-BE--车主端预约单操作-BE"
        story_id = "STORY-004-BE"
        state_file = tmp / ".auto-engineering" / work_item / "state.json"
        state_file.parent.mkdir(parents=True, exist_ok=True)
        nested = {
            "version": "2",
            "projectKey": "life",
            "stateModel": "nested",
            "entryNode": "STORY",
            "activeStory": story_id,
            "storyStates": {story_id: {"phase": "story-generated"}},
            "workItemKey": work_item,
            "scale": "小",
        }
        state_file.write_text(json.dumps(nested, ensure_ascii=False), encoding="utf-8")
        mirror = dict(nested)
        mirror["activeWorkItem"] = work_item
        mirror["activeStatePath"] = str(state_file)
        (tmp / ".ae-sdd" / "state.json").write_text(
            json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        code, out, err = _run_cli(
            tmp, "state", "write",
            "--phase", "story-reviewed",
            "--allow-empty-memory",
        )

        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        updated = json.loads(state_file.read_text(encoding="utf-8"))
        self.assertEqual(updated["storyStates"][story_id]["phase"], "story-reviewed")
        self.assertNotEqual(updated.get("phase"), "story-reviewed")


class TestStateWorkItemIsolationLegacy(unittest.TestCase):
    """🆕 v3.9.3 BREAKING：v3.8.2 双段 + --name 形参已废除。

    旧测试全部 SKIPPED，新行为见 TestCmdStateNewV393。
    """

    def test_legacy_double_segment(self):
        self.skipTest("v3.9.3 BREAKING: v3.8.2 双段目录已废除")

    def test_legacy_name_param(self):
        self.skipTest("v3.9.3 BREAKING: --name 形参已废除")

    def test_legacy_project_state_behavior(self):
        self.skipTest("v3.9.3 SKIPPED: 旧 v3.8.2 行为已废除，新行为见 TestCmdStateNewV393")

    def test_legacy_two_work_items(self):
        self.skipTest("v3.9.3 SKIPPED: 旧 v3.8.2 行为已废除")

    def test_legacy_state_read_work_item(self):
        self.skipTest("v3.9.3 SKIPPED: 旧 v3.8.2 行为已废除")


if __name__ == "__main__":
    unittest.main(verbosity=2)
