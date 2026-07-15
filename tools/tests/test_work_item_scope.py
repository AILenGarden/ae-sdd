from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from lib import document_storage, gates


CLI = Path(__file__).resolve().parent.parent / "bin" / "ae-sdd"


def _write(path: Path, content: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


def test_scoped_artifact_prefers_work_item_over_story_report(tmp_path: Path) -> None:
    work_item = _write(
        tmp_path / "ae-sdd-doc" / "CR" / "BUG-001" / "BUG-001-CodeReview.md",
        "# BUG-001 review",
    )
    _write(
        tmp_path / "ae-sdd-doc" / "CR" / "STORY-001" / "STORY-001-CodeReview.md",
        "# STORY-001 review",
    )

    resolved = document_storage.resolve_scoped_artifact(
        tmp_path,
        category="CR",
        work_item_id="BUG-001",
        story_id="STORY-001",
        suffixes=["-CodeReview.md", "-CodeReview-v*-r*.md"],
    )

    assert resolved.path == work_item
    assert resolved.scope_source == "work-item"


def test_scoped_artifact_uses_unique_legacy_story_fallback(tmp_path: Path) -> None:
    legacy = _write(
        tmp_path / "ae-sdd-doc" / "CR" / "STORY-001" / "STORY-001-CodeReview.md",
        "# STORY-001 review",
    )

    resolved = document_storage.resolve_scoped_artifact(
        tmp_path,
        category="CR",
        work_item_id="BUG-001",
        story_id="STORY-001",
        suffixes=["-CodeReview.md", "-CodeReview-v*-r*.md"],
    )

    assert resolved.path == legacy
    assert resolved.scope_source == "legacy-story"


def test_scoped_artifact_rejects_ambiguous_legacy_fallback(tmp_path: Path) -> None:
    _write(
        tmp_path / "ae-sdd-doc" / "CR" / "STORY-001" / "STORY-001-CodeReview.md",
        "# current",
    )
    _write(
        tmp_path
        / "ae-sdd-doc"
        / "iterations"
        / "2026-07-14"
        / "CR"
        / "STORY-001"
        / "STORY-001-CodeReview.md",
        "# historical duplicate",
    )

    with pytest.raises(document_storage.ScopeAmbiguousError) as exc:
        document_storage.resolve_scoped_artifact(
            tmp_path,
            category="CR",
            work_item_id="BUG-001",
            story_id="STORY-001",
            suffixes=["-CodeReview.md"],
        )

    assert exc.value.code == "SCOPE_AMBIGUOUS"
    assert len(exc.value.candidates) == 2


def test_g12_uses_work_item_report_and_exposes_scope_source(tmp_path: Path) -> None:
    report = _write(
        tmp_path / "ae-sdd-doc" / "CR" / "BUG-001" / "BUG-001-CodeReview.md",
        "# BUG-001 review\n\nParent STORY-001",
    )
    _write(
        tmp_path / "ae-sdd-doc" / "CR" / "STORY-001" / "STORY-001-CodeReview.md",
        "# STORY-001 main review",
    )
    state = {
        "entryNode": "BUG",
        "scale": "微",
        "phase": "code-reviewed",
        "currentWorkItem": "BUG-001",
        "currentStory": "STORY-001",
    }

    result = gates.check_g12(tmp_path, state, "STORY-001")

    assert result.pass_ is True
    assert Path(result.details["file"]) == report
    assert result.details["scopeSource"] == "work-item"


def test_g13_bug_followup_uses_inherited_parent_story_trace(tmp_path: Path) -> None:
    state = {
        "entryNode": "BUG",
        "scale": "微",
        "phase": "coding",
        "currentWorkItem": "BUG-001",
        "parentStory": "STORY-001",
        "currentStory": "STORY-001",
    }

    result = gates.check_g13(tmp_path, state, "STORY-001")

    assert result.pass_ is True
    assert result.details["traceMode"] == "inherited-parent-story"


def test_g13_standalone_micro_bug_uses_minimal_trace(tmp_path: Path) -> None:
    state = {
        "entryNode": "BUG",
        "scale": "微",
        "phase": "coding",
        "currentWorkItem": "BUG-001",
        "currentStory": "BUG-001",
    }

    result = gates.check_g13(tmp_path, state, "BUG-001")

    assert result.pass_ is True
    assert result.details["traceMode"] == "minimal-bug-trace"


def test_g13_large_dr_route_remains_strict(tmp_path: Path) -> None:
    state = {
        "entryNode": "DR",
        "scale": "大",
        "phase": "coding",
        "currentWorkItem": "DR-001",
        "currentStory": "STORY-001",
    }

    result = gates.check_g13(tmp_path, state, "STORY-001")

    assert result.pass_ is False
    assert result.details.get("traceMode", "strict-dr") == "strict-dr"


def test_g13_strict_route_reads_canonical_ae_sdd_doc_tree(tmp_path: Path) -> None:
    _write(tmp_path / "ae-sdd-doc" / "DR" / "DR-001.md", "# DR-001\n")
    _write(
        tmp_path / "ae-sdd-doc" / "docs" / "guide-lra-workflow.md",
        "# Not an RA document\n",
    )
    _write(
        tmp_path / "ae-sdd-doc" / "Story" / "STORY-001.md",
        "# STORY-001\n\nSource: DR-001\n",
    )
    state = {
        "entryNode": "DR",
        "scale": "大",
        "phase": "coding",
        "currentWorkItem": "DR-001",
        "currentStory": "STORY-001",
    }

    result = gates.check_g13(tmp_path, state, "STORY-001")

    assert result.pass_ is True
    assert result.details["traceMode"] == "strict-dr"
    assert result.details["n_drs"] == 1


def test_cli_gates_check_accepts_explicit_work_item(tmp_path: Path) -> None:
    (tmp_path / ".ae-sdd").mkdir()
    (tmp_path / ".ae-sdd" / "config.yaml").write_text(
        "projectKey: test\n", encoding="utf-8"
    )
    for key, story in [("BUG-001", "STORY-001"), ("OTHER-001", "STORY-OTHER")]:
        _write(
            tmp_path / ".auto-engineering" / key / "state.json",
            json.dumps(
                {
                    "version": "1",
                    "entryNode": "BUG",
                    "scale": "微",
                    "phase": "code-reviewed",
                    "currentWorkItem": key,
                    "currentStory": story,
                    "history": [],
                }
            ),
        )
    _write(
        tmp_path / "ae-sdd-doc" / "CR" / "BUG-001" / "BUG-001-CodeReview.md",
        "# BUG review\n\nSTORY-001",
    )
    env = os.environ.copy()
    env["PYTHONPATH"] = str(CLI.parent.parent)
    env["PYTHONIOENCODING"] = "utf-8"

    result = subprocess.run(
        [
            sys.executable,
            str(CLI),
            "gates",
            "check",
            "--work-item",
            "BUG-001",
            "--only",
            "G-12",
            "--json",
        ],
        cwd=str(tmp_path),
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["all_pass"] is True
    assert payload["workItem"] == "BUG-001"
    assert payload["results"][0]["details"]["scopeSource"] == "work-item"
