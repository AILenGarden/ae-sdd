from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import memory_gate, memory_store  # noqa: E402
from lib.gate_intercept import check_intercept  # noqa: E402


def _project(tmp_path: Path, *, phase: str = "coding", story: str = "STORY-001") -> Path:
    ae_sdd = tmp_path / ".ae-sdd"
    ae_sdd.mkdir()
    (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
    (ae_sdd / "state.json").write_text(json.dumps({
        "version": "1",
        "projectKey": "test",
        "phase": phase,
        "currentStory": story,
        "currentTask": None,
        "history": [],
    }), encoding="utf-8")
    return ae_sdd


def test_memory_gate_blocks_missing_enter_write(tmp_path):
    ae_sdd = _project(tmp_path, phase="coding", story="STORY-001")
    result = memory_gate.check_state_transition(
        ade_sdd=ae_sdd,
        state_data={"phase": "coding", "currentStory": "STORY-001"},
        target_phase="test-running",
    )
    assert result["blocked"]
    assert result["memory_phase"] == "coding"
    assert "enter" in result["reason"]


def test_memory_gate_passes_after_enter_and_write(tmp_path):
    ae_sdd = _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="Coding finished", evidence=["src/Foo.java:1"], actor="test")

    result = memory_gate.check_state_transition(
        ade_sdd=ae_sdd,
        state_data={"phase": "coding", "currentStory": "STORY-001"},
        target_phase="test-running",
    )
    assert result["pass"]
    assert not result["blocked"]


def test_gate_intercept_blocks_state_write_before_memory(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    allowed, reason = check_intercept(
        "Bash",
        bash_command="ae-sdd state write --phase test-running",
        project_dir=tmp_path,
    )
    assert not allowed
    assert "Mandatory memory gate failed" in reason
    assert "memory phase: coding" in reason


def test_gate_intercept_reaches_entry_gates_after_memory_passes(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="Coding finished", evidence=["src/Foo.java:1"], actor="test")

    allowed, reason = check_intercept(
        "Bash",
        bash_command="ae-sdd state write --phase test-running",
        project_dir=tmp_path,
    )
    assert not allowed
    assert "Mandatory memory gate failed" not in reason
    assert "G-00" in reason


def test_cli_state_write_blocks_before_memory(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    repo = Path(__file__).resolve().parent.parent.parent
    result = subprocess.run(
        [sys.executable, str(repo / "tools" / "bin" / "ae-sdd"), "state", "write", "--phase", "test-running"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert "Mandatory memory gate failed" in (result.stdout + result.stderr)


def test_cli_state_write_allows_maintenance_override(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    repo = Path(__file__).resolve().parent.parent.parent
    result = subprocess.run(
        [
            sys.executable,
            str(repo / "tools" / "bin" / "ae-sdd"),
            "state",
            "write",
            "--phase",
            "test-running",
            "--allow-empty-memory",
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0
