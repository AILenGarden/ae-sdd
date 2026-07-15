from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


CLI = str(Path(__file__).resolve().parent.parent / "bin" / "ae-sdd")


def test_legacy_state_write_delegates_to_state_store_and_increments_revision(tmp_path: Path) -> None:
    (tmp_path / ".ae-sdd").mkdir()
    (tmp_path / ".ae-sdd" / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
    state_dir = tmp_path / ".auto-engineering" / "BUG-001"
    state_dir.mkdir(parents=True)
    state_path = state_dir / "state.json"
    state_path.write_text(json.dumps({
        "version": "1", "projectKey": "test", "currentWorkItem": "BUG-001",
        "workItemKey": "BUG-001", "stateMachineId": "BUG-001", "currentStory": "STORY-001",
        "phase": "initialized", "scale": "微", "entryNode": "BUG", "history": [],
    }), encoding="utf-8")
    env = os.environ.copy()
    env["PYTHONPATH"] = str(Path(__file__).resolve().parent.parent)

    result = subprocess.run(
        [sys.executable, CLI, "state", "write", "--phase", "coding-process",
         "--work-item", "BUG-001", "--json"],
        cwd=tmp_path, env=env, capture_output=True, text=True, encoding="utf-8",
    )

    assert result.returncode == 0, result.stderr
    saved = json.loads(state_path.read_text(encoding="utf-8"))
    assert saved["phase"] == "coding-process"
    assert saved["revision"] == 1
    assert saved["lastMutation"]["operation"] == "legacy.state.write"
    lease = json.loads((state_dir / "state.lease.json").read_text(encoding="utf-8"))
    assert lease["status"] == "released"
