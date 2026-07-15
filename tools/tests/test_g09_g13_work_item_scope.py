"""Regression contracts for Story-entry traceability and scoped G-09 scans."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import baseline, evidence, gates, verification_plan  # noqa: E402


STORY_ID = "STORY-004-BE"
MASTER_SOURCE = Path(__file__).resolve().parents[2] / "source"


def _project(files: dict[str, str]) -> Path:
    root = Path(tempfile.mkdtemp())
    for relative, content in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    return root


def _clean_test() -> str:
    return (
        "package example;\n"
        "import org.junit.Test;\n"
        "import static org.junit.Assert.assertEquals;\n"
        "public class CurrentTest {\n"
        "  @Test public void value() { assertEquals(2, 1 + 1); }\n"
        "}\n"
    )


def _bad_test(class_name: str = "BadTest") -> str:
    return (
        "package example;\n"
        "import org.junit.Test;\n"
        "import org.junit.Ignore;\n"
        f"public class {class_name} {{\n"
        "  @Test @Ignore public void disabled() {}\n"
        "}\n"
    )


def _state_with_verified_scope(project: Path, changed_paths: list[str]) -> tuple[dict, Path]:
    plan = verification_plan.build_plan(project, STORY_ID, changed_paths)
    command = "test_authenticity_scan.py"
    toolchain = "test-authenticity:v1"
    report = project / ".auto-engineering" / STORY_ID / "evidence" / "g09-report.json"
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps({"status": "PASS", "storyId": STORY_ID,
                                  "scope": sorted(changed_paths),
                                  "commandHash": evidence.command_hash(command),
                                  "toolchainFingerprint": toolchain}), encoding="utf-8")
    evidence.record(
        project,
        STORY_ID,
        kind="test-authenticity",
        command=command,
        input_fingerprint=plan["planFingerprint"],
        toolchain_fingerprint=toolchain,
        exit_code=0,
        artifacts=[{"path": report.relative_to(project).as_posix(),
                    "sha256": evidence.artifact_hash(report)}],
        summary={"gate": "G-09", "storyId": STORY_ID, "status": "PASS",
                 "changedPaths": sorted(changed_paths), "scope": sorted(changed_paths),
                 "commandHash": evidence.command_hash(command),
                 "toolchainFingerprint": toolchain,
                 "report": report.relative_to(project).as_posix()},
    )
    return {"phase": "test-running", "entryNode": "STORY", "verificationPlan": plan}, report


class TestG13StoryEntry(unittest.TestCase):
    def _complete_story_chain(self) -> dict[str, str]:
        return {
            f"design/{STORY_ID}.md": f"# {STORY_ID}\n",
            f"task/{STORY_ID}-task-001.md": f"# Task\nimplements {STORY_ID}\n",
            f"design/{STORY_ID}-Coding-Report.md": (
                f"# Coding Report\ncompleted {STORY_ID}-task-001\n"
            ),
            f"design/{STORY_ID}-CodeReview.md": f"# Code Review\nreviewed {STORY_ID}\n",
        }

    def test_story_entry_without_dr_passes_with_explicit_dr_exemption(self):
        project = _project(self._complete_story_chain())
        result = gates.check_g13(
            project,
            {"entryNode": "STORY", "scale": "\u4e2d", "phase": "code-reviewed"},
            STORY_ID,
        )

        self.assertTrue(result.pass_, result.message)
        self.assertEqual(result.details.get("entryNode"), "STORY")
        self.assertEqual(result.details.get("dr_layer", {}).get("status"), "EXEMPT_STORY_ENTRY")
        self.assertTrue(result.details.get("dr_layer", {}).get("exempt"))

    def test_non_story_or_legacy_entry_without_dr_remains_strict(self):
        cases = (("DR", "\u5927"), ("PRD", "\u5927"), (None, None), ("STORY", "\u5927"))
        for entry_node, scale in cases:
            with self.subTest(entry_node=entry_node, scale=scale):
                project = _project(self._complete_story_chain())
                state = {"phase": "code-reviewed"}
                if entry_node is not None:
                    state["entryNode"] = entry_node
                if scale is not None:
                    state["scale"] = scale
                result = gates.check_g13(project, state, STORY_ID)
                self.assertFalse(result.pass_)

    def test_story_entry_keeps_story_task_coding_and_review_links_strict(self):
        cases = {
            "missing-story": f"design/{STORY_ID}.md",
            "task-does-not-reference-story": f"task/{STORY_ID}-task-001.md",
            "coding-does-not-reference-task": f"design/{STORY_ID}-Coding-Report.md",
            "review-does-not-reference-story": f"design/{STORY_ID}-CodeReview.md",
        }
        for label, path in cases.items():
            with self.subTest(case=label):
                files = self._complete_story_chain()
                if label == "missing-story":
                    files.pop(path)
                else:
                    files[path] = "# Present but deliberately unlinked\n"
                project = _project(files)
                result = gates.check_g13(project, {"entryNode": "STORY"}, STORY_ID)
                self.assertFalse(result.pass_, label)

    def test_story_entry_code_reviewed_requires_downstream_documents(self):
        required = (
            f"task/{STORY_ID}-task-001.md",
            f"design/{STORY_ID}-Coding-Report.md",
            f"design/{STORY_ID}-CodeReview.md",
        )
        for missing_path in required:
            with self.subTest(missing_path=missing_path):
                files = self._complete_story_chain()
                files.pop(missing_path)
                project = _project(files)
                result = gates.check_g13(
                    project,
                    {"entryNode": "STORY", "scale": "\u4e2d", "phase": "code-reviewed"},
                    STORY_ID,
                )
                self.assertFalse(result.pass_, missing_path)


class TestG09WorkItemScope(unittest.TestCase):
    def _check(self, project: Path, state: dict) -> gates.GateResult:
        return gates.check_g09(project, state, STORY_ID, master_source=MASTER_SOURCE)

    def test_verified_scope_clean_ignores_untouched_repository_debt(self):
        current = "feature/src/test/java/example/CurrentTest.java"
        project = _project({
            current: _clean_test(),
            "legacy/src/test/java/example/LegacyBadTest.java": _bad_test("LegacyBadTest"),
        })
        state, _ = _state_with_verified_scope(project, [current])

        result = self._check(project, state)

        self.assertTrue(result.pass_, result.message)
        self.assertEqual(result.details.get("scopeMode"), "work-item")
        self.assertEqual(result.details.get("scopeSource"), "verificationPlan.changedPaths")
        self.assertEqual(result.details.get("scopePaths"), [current])

    def test_legacy_state_scope_fields_remain_compatible(self):
        current = "feature/src/test/java/example/CurrentTest.java"
        for field in ("changedPaths", "changedFiles"):
            with self.subTest(field=field):
                project = _project({
                    current: _clean_test(),
                    "legacy/src/test/java/example/LegacyBadTest.java": _bad_test("LegacyBadTest"),
                })
                result = self._check(project, {"phase": "test-running", field: [current]})
                self.assertTrue(result.pass_, result.message)
                self.assertEqual(result.details.get("scopeSource"), f"state.{field}")

    def test_verification_plan_scope_precedes_legacy_state_fields(self):
        current = "feature/src/test/java/example/CurrentTest.java"
        legacy = "legacy/src/test/java/example/LegacyBadTest.java"
        project = _project({current: _clean_test(), legacy: _bad_test("LegacyBadTest")})
        state, _ = _state_with_verified_scope(project, [current])
        state["changedPaths"] = [legacy]
        state["changedFiles"] = [legacy]

        result = self._check(project, state)

        self.assertTrue(result.pass_, result.message)
        self.assertEqual(result.details.get("scopeSource"), "verificationPlan.changedPaths")

    def test_new_blocker_inside_scope_fails(self):
        changed = "feature/src/test/java/example/NewBadTest.java"
        project = _project({changed: _bad_test("NewBadTest")})
        state, _ = _state_with_verified_scope(project, [changed])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertGreater(result.details.get("n_blockers", 0), 0)

    def test_touched_historical_debt_fails(self):
        touched = "legacy/src/test/java/example/LegacyBadTest.java"
        project = _project({touched: _bad_test("LegacyBadTest")})
        state, _ = _state_with_verified_scope(project, [touched])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertGreater(result.details.get("n_blockers", 0), 0)

    def test_plan_fingerprint_mismatch_fails_closed(self):
        original = "feature/src/test/java/example/CurrentTest.java"
        project = _project({original: _clean_test()})
        state, _ = _state_with_verified_scope(project, [original])
        state["verificationPlan"]["changedPaths"] = ["other/src/test/java/example/OtherTest.java"]

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCOPE_INVALID")

    def test_evidence_fingerprint_mismatch_fails_closed(self):
        changed = "feature/src/test/java/example/CurrentTest.java"
        project = _project({changed: _clean_test()})
        state, _ = _state_with_verified_scope(project, [changed])
        manifest = evidence.load_manifest(project, STORY_ID)
        manifest["entries"][-1]["inputFingerprint"] = "sha256:tampered"
        evidence.save_manifest(project, STORY_ID, manifest)

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_evidence_manifest_content_tamper_fails_closed(self):
        changed = "feature/src/test/java/example/CurrentTest.java"
        project = _project({changed: _clean_test()})
        state, _ = _state_with_verified_scope(project, [changed])
        manifest = evidence.load_manifest(project, STORY_ID)
        manifest["entries"][-1]["summary"]["changedPaths"] = ["forged/Test.java"]
        evidence.manifest_path(project, STORY_ID).write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_evidence_artifact_hash_mismatch_fails_closed(self):
        changed = "feature/src/test/java/example/CurrentTest.java"
        project = _project({changed: _clean_test()})
        state, report = _state_with_verified_scope(project, [changed])
        manifest = evidence.load_manifest(project, STORY_ID)
        snapshot = project / manifest["entries"][-1]["artifacts"][0]["snapshotPath"]
        snapshot.write_text("tampered", encoding="utf-8")

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_out_of_project_scope_fails_closed(self):
        project = _project({"src/test/java/example/CurrentTest.java": _clean_test()})
        state, _ = _state_with_verified_scope(project, ["../outside/OtherTest.java"])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCOPE_INVALID")

    def test_no_work_item_scope_keeps_strict_full_repository_scan(self):
        project = _project({
            "legacy/src/test/java/example/LegacyBadTest.java": _bad_test("LegacyBadTest"),
        })

        result = self._check(project, {"phase": "test-running"})

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeMode", "full-repository"), "full-repository")

    def test_g09_does_not_consume_gcode1_baseline(self):
        project = _project({"src/test/java/example/CurrentTest.java": _clean_test()})
        baseline.create(
            project,
            "G-CODE-1",
            [{"rule": "AP-1", "path": "src/main/java/example/Old.java", "severity": "BLOCKER"}],
            created_by="test",
            scanner_version="1",
            ruleset_fingerprint="rules-v1",
            project_fingerprint="project-v1",
            require_user_approval=True,
        )
        baseline_path = baseline.baseline_path(project, "G-CODE-1")
        payload = json.loads(baseline_path.read_text(encoding="utf-8"))
        payload["contentHash"] = "sha256:tampered"
        baseline_path.write_text(json.dumps(payload), encoding="utf-8")

        result = self._check(project, {"phase": "test-running"})

        self.assertTrue(result.pass_, "G-09 must not read or waive through the G-CODE-1 baseline")
        self.assertEqual(result.gate_id, "G-09")


if __name__ == "__main__":
    unittest.main(verbosity=2)
