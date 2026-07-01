"""CLI-level tests for work-item isolated state files."""
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


class TestStateWorkItemIsolation(unittest.TestCase):
    def test_state_write_with_story_uses_isolated_work_item_state(self):
        """--story keeps a separate .auto-engineering/{id}/state.json file."""
        tmp = _setup_project()

        code, out, err = _run_cli(
            tmp, "state", "write",
            "--phase", "task-generated",
            "--story", "BUG-LIFE-001",
            "--allow-empty-memory",
        )
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")

        isolated = tmp / ".auto-engineering" / "BUG-LIFE-001" / "state.json"
        mirror = tmp / ".ae-sdd" / "state.json"
        self.assertTrue(isolated.is_file())
        self.assertTrue(mirror.is_file())

        isolated_state = json.loads(isolated.read_text(encoding="utf-8"))
        mirror_state = json.loads(mirror.read_text(encoding="utf-8"))
        self.assertEqual(isolated_state["currentStory"], "BUG-LIFE-001")
        self.assertEqual(isolated_state["currentWorkItem"], "BUG-LIFE-001")
        self.assertEqual(isolated_state["scale"], "微")
        self.assertEqual(isolated_state["entryNode"], "BUG")
        self.assertEqual(mirror_state["activeWorkItem"], "BUG-LIFE-001")

    def test_two_work_items_do_not_overwrite_each_other(self):
        """切换独立任务只更新 active mirror，不覆盖旧任务 state。"""
        tmp = _setup_project()
        for item in ("BUG-LIFE-001", "OPT-LIFE-002"):
            code, out, err = _run_cli(
                tmp, "state", "write",
                "--phase", "task-generated",
                "--work-item", item,
                "--allow-empty-memory",
            )
            self.assertEqual(code, 0, msg=f"{item} stdout={out}\nstderr={err}")

        bug_state_path = tmp / ".auto-engineering" / "BUG-LIFE-001" / "state.json"
        opt_state_path = tmp / ".auto-engineering" / "OPT-LIFE-002" / "state.json"
        bug_state = json.loads(bug_state_path.read_text(encoding="utf-8"))
        opt_state = json.loads(opt_state_path.read_text(encoding="utf-8"))
        mirror_state = json.loads((tmp / ".ae-sdd" / "state.json").read_text(encoding="utf-8"))

        self.assertEqual(bug_state["currentWorkItem"], "BUG-LIFE-001")
        self.assertEqual(opt_state["currentWorkItem"], "OPT-LIFE-002")
        self.assertEqual(bug_state["scale"], "微")
        self.assertEqual(opt_state["scale"], "微")
        self.assertEqual(opt_state["entryNode"], "OPT")
        self.assertEqual(mirror_state["activeWorkItem"], "OPT-LIFE-002")

        code, out, err = _run_cli(tmp, "state", "read", "--work-item", "BUG-LIFE-001", "--json")
        self.assertEqual(code, 0, msg=f"stdout={out}\nstderr={err}")
        read_back = json.loads(out)
        self.assertEqual(read_back["currentWorkItem"], "BUG-LIFE-001")


if __name__ == "__main__":
    unittest.main(verbosity=2)
