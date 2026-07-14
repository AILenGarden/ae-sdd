import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


CLI = str(Path(__file__).resolve().parent.parent / "bin" / "ae-sdd")


def _project() -> tuple[Path, Path, Path]:
    root = Path(tempfile.mkdtemp())
    ade = root / ".ae-sdd"
    ade.mkdir()
    (ade / "config.yaml").write_text("projectKey: demo\n", encoding="utf-8")
    state_dir = root / ".auto-engineering" / "Story-004"
    state_dir.mkdir(parents=True)
    state_path = state_dir / "state.json"
    state_path.write_text(json.dumps({
        "version": "1", "projectKey": "demo", "currentStory": "STORY-004-BE",
        "currentWorkItem": "Story-004", "workItemKey": "Story-004",
        "stateMachineId": "Story-004", "phase": "test-running",
    }), encoding="utf-8")
    changed = root / "module" / "src" / "test" / "ExampleTest.java"
    changed.parent.mkdir(parents=True)
    changed.write_text("class ExampleTest {}\n", encoding="utf-8")
    return root, state_path, changed


def _run(root: Path, *args: str):
    env = os.environ.copy()
    env["PYTHONPATH"] = str(Path(__file__).resolve().parent.parent.parent)
    return subprocess.run([sys.executable, CLI, *args], cwd=root, env=env,
                          capture_output=True, text=True, encoding="utf-8")


def test_verify_plan_persist_writes_canonical_work_item_bound_plan():
    root, state_path, _ = _project()
    result = _run(root, "verify", "plan", "--story", "STORY-004-BE",
                  "--work-item", "Story-004", "--changed", "module/src/test/ExampleTest.java",
                  "--persist", "--json")
    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    saved = json.loads(state_path.read_text(encoding="utf-8"))["verificationPlan"]
    assert saved == payload
    assert saved["workItem"] == "Story-004"
    assert saved["changedPaths"] == ["module/src/test/ExampleTest.java"]
    assert saved["planFingerprint"].startswith("sha256:")
    assert saved["inputFingerprint"].startswith("sha256:")


def test_verify_plan_without_persist_does_not_write_state_and_is_stable():
    root, state_path, _ = _project()
    before = state_path.read_text(encoding="utf-8")
    args = ("verify", "plan", "--story", "STORY-004-BE", "--work-item", "Story-004",
            "--changed", "module/src/test/ExampleTest.java", "--json")
    first, second = _run(root, *args), _run(root, *args)
    assert first.returncode == second.returncode == 0
    assert json.loads(first.stdout) == json.loads(second.stdout)
    assert state_path.read_text(encoding="utf-8") == before


def test_verify_plan_rejects_outside_path_and_wrong_work_item_is_isolated():
    root, state_path, _ = _project()
    outside = root.parent / "outside.java"
    outside.write_text("class Outside {}\n", encoding="utf-8")
    escaped = _run(root, "verify", "plan", "--story", "STORY-004-BE",
                   "--work-item", "Story-004", "--changed", "../outside.java", "--persist")
    wrong = _run(root, "verify", "plan", "--story", "STORY-004-BE",
                 "--work-item", "Story-999", "--changed", "module/src/test/ExampleTest.java", "--persist")
    assert escaped.returncode != 0
    assert wrong.returncode != 0
    assert "verificationPlan" not in json.loads(state_path.read_text(encoding="utf-8"))


def test_evidence_finalize_upgrades_hashes_and_tampering_fails():
    root, _, _ = _project()
    artifact = root / ".auto-engineering" / "STORY-004-BE" / "evidence" / "g09.json"
    artifact.parent.mkdir(parents=True, exist_ok=True)
    artifact.write_text('{"status":"PASS"}\n', encoding="utf-8")
    manifest = artifact.parent / "manifest.json"
    manifest.write_text(json.dumps({
        "schemaVersion": 1, "storyId": "STORY-004-BE", "entries": [{
            "kind": "test-authenticity", "artifacts": [{"path": str(artifact)}]
        }]
    }), encoding="utf-8")
    finalized = _run(root, "evidence", "finalize", "--story", "STORY-004-BE", "--json")
    assert finalized.returncode == 0, finalized.stderr
    saved = json.loads(manifest.read_text(encoding="utf-8"))
    assert saved["contentHash"].startswith("sha256:")
    assert saved["entries"][0]["artifacts"][0]["sha256"].startswith("sha256:")
    artifact.write_text("tampered\n", encoding="utf-8")
    rejected = _run(root, "evidence", "finalize", "--story", "STORY-004-BE")
    assert rejected.returncode != 0
