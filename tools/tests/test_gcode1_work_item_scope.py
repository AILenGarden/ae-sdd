"""TDD contracts for trustworthy work-item scoped G-CODE-1 scans."""
from __future__ import annotations

import json
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import evidence, gates, verification_plan  # noqa: E402


STORY_ID = "STORY-004-BE"
MASTER_SOURCE = Path(__file__).resolve().parents[2] / "source"


def _project(files: dict[str, str]) -> Path:
    root = Path(tempfile.mkdtemp())
    for relative, content in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    return root


def _clean_code() -> str:
    return "package example; public class CleanService { public int value() { return 1; } }\n"


def _bad_code() -> str:
    return 'package example; public class BadService { private String token = "abcdefg"; }\n'


def _verified_state(project: Path, changed_paths: list[str]) -> tuple[dict, Path]:
    plan = verification_plan.build_plan(project, STORY_ID, changed_paths)
    report = project / ".auto-engineering" / STORY_ID / "evidence" / "g09-report.json"
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps({
        "status": "PASS",
        "storyId": STORY_ID,
        "scope": sorted(changed_paths),
    }), encoding="utf-8")
    evidence.record(
        project,
        STORY_ID,
        kind="test-authenticity",
        command="test_authenticity_scan.py",
        input_fingerprint=plan["planFingerprint"],
        toolchain_fingerprint="test-authenticity:v1",
        exit_code=0,
        artifacts=[{
            "path": report.relative_to(project).as_posix(),
            "sha256": evidence.artifact_hash(report),
        }],
        summary={"gate": "G-09", "storyId": STORY_ID,
                 "status": "PASS", "changedPaths": sorted(changed_paths)},
    )
    return {"phase": "test-running", "entryNode": "STORY", "verificationPlan": plan}, report


def _mutate_manifest(project: Path, mutate) -> None:
    manifest = evidence.load_manifest(project, STORY_ID)
    mutate(manifest["entries"][-1])
    evidence.save_manifest(project, STORY_ID, manifest)


def _load_scanner_module():
    scanner_path = Path(__file__).resolve().parents[2] / "scripts" / "coding_authenticity_scan.py"
    spec = importlib.util.spec_from_file_location("coding_authenticity_scan_for_test", scanner_path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class TestGCode1WorkItemScope(unittest.TestCase):
    def _check(self, project: Path, state: dict) -> gates.GateResult:
        return gates.check_gcode1(project, state, STORY_ID, master_source=MASTER_SOURCE)

    def test_verified_scope_clean_ignores_untouched_repository_debt(self):
        current = "feature/src/main/java/example/CleanService.java"
        project = _project({
            current: _clean_code(),
            "legacy/src/main/java/example/BadService.java": _bad_code(),
        })
        state, _ = _verified_state(project, [current])

        with patch("lib.baseline.load", side_effect=AssertionError("scoped gate must not read baseline")):
            result = self._check(project, state)

        self.assertTrue(result.pass_, result.message)
        self.assertEqual(result.details.get("scopeMode"), "work-item")
        self.assertEqual(result.details.get("scopePaths"), [current])
        self.assertEqual(result.details.get("n_code_files"), 1)

    def test_scoped_new_blocker_fails(self):
        changed = "feature/src/main/java/example/BadService.java"
        project = _project({changed: _bad_code()})
        state, _ = _verified_state(project, [changed])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertGreater(result.details.get("n_blockers", 0), 0)
        self.assertEqual(result.details.get("scopeMode"), "work-item")

    def test_scoped_touched_historical_debt_fails(self):
        touched = "legacy/src/main/java/example/BadService.java"
        project = _project({touched: _bad_code()})
        state, _ = _verified_state(project, [touched])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertGreater(result.details.get("n_blockers", 0), 0)

    def test_plan_fingerprint_tamper_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        state["verificationPlan"]["planFingerprint"] = "sha256:tampered"

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCOPE_INVALID")

    def test_manifest_content_tamper_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        manifest_path = evidence.manifest_path(project, STORY_ID)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["entries"][-1]["inputFingerprint"] = "sha256:tampered"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_verified_plan_without_manifest_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        plan = verification_plan.build_plan(project, STORY_ID, [changed])
        state = {"phase": "test-running", "verificationPlan": plan}

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_manifest_without_current_g09_entry_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        manifest = evidence.load_manifest(project, STORY_ID)
        manifest["entries"][-1]["kind"] = "unrelated"
        manifest["entries"][-1]["summary"]["gate"] = "G-12"
        evidence.save_manifest(project, STORY_ID, manifest)

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_evidence_semantic_bindings_fail_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        mutations = {
            "input-fingerprint": lambda entry: entry.update(inputFingerprint="sha256:other"),
            "command": lambda entry: entry.update(commandHash=evidence.command_hash("other-command")),
            "toolchain": lambda entry: entry.update(toolchainFingerprint="unknown-toolchain"),
            "summary-story": lambda entry: entry["summary"].update(storyId="STORY-999-BE"),
            "summary-status": lambda entry: entry["summary"].update(status="FAIL"),
            "summary-paths": lambda entry: entry["summary"].update(changedPaths=["other.java"]),
        }
        for label, mutate in mutations.items():
            with self.subTest(binding=label):
                project = _project({changed: _clean_code()})
                state, _ = _verified_state(project, [changed])
                _mutate_manifest(project, mutate)

                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_g09_report_semantic_bindings_fail_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        mutations = {
            "report-story": lambda report: report.update(storyId="STORY-999-BE"),
            "report-status": lambda report: report.update(status="FAIL"),
            "report-scope": lambda report: report.update(scope=["other.java"]),
        }
        for label, mutate in mutations.items():
            with self.subTest(binding=label):
                project = _project({changed: _clean_code()})
                state, report_path = _verified_state(project, [changed])
                report = json.loads(report_path.read_text(encoding="utf-8"))
                mutate(report)
                report_path.write_text(json.dumps(report), encoding="utf-8")

                def update_artifact(entry):
                    entry["artifacts"][0]["sha256"] = evidence.artifact_hash(report_path)

                _mutate_manifest(project, update_artifact)
                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_artifact_outside_project_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, report_path = _verified_state(project, [changed])
        outside = project.parent / f"{project.name}-outside-g09.json"
        outside.write_text(report_path.read_text(encoding="utf-8"), encoding="utf-8")

        def point_outside(entry):
            entry["artifacts"][0] = {
                "path": str(outside),
                "sha256": evidence.artifact_hash(outside),
            }

        _mutate_manifest(project, point_outside)
        try:
            result = self._check(project, state)
        finally:
            outside.unlink(missing_ok=True)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_scoped_invalid_scanner_result_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        invalid_results = (
            SimpleNamespace(returncode=2, stdout=json.dumps({"status": "PASS", "findings": []})),
            SimpleNamespace(returncode=0, stdout=json.dumps({"status": "UNKNOWN", "findings": []})),
            SimpleNamespace(returncode=0, stdout=json.dumps({"status": "ERROR", "findings": []})),
        )
        for scanner_result in invalid_results:
            with self.subTest(scanner_result=scanner_result), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)
                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_scoped_malformed_finding_paths_fail_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        for finding_path in ("", "../escape.java", str((project / changed).resolve())):
            report = {
                "status": "FAIL",
                "findings": [{"severity": "BLOCKER", "rule": "x", "path": finding_path}],
            }
            scanner_result = SimpleNamespace(returncode=0, stdout=json.dumps(report))
            with self.subTest(finding_path=finding_path), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)
                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_artifact_hash_tamper_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, report = _verified_state(project, [changed])
        report.write_text("tampered", encoding="utf-8")

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_scope_path_escape_fails_closed(self):
        project = _project({"src/main/java/example/CleanService.java": _clean_code()})
        state, _ = _verified_state(project, ["../outside/Other.java"])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCOPE_INVALID")

    def test_missing_scope_keeps_strict_full_repository_scan(self):
        project = _project({"legacy/src/main/java/example/BadService.java": _bad_code()})

        result = self._check(project, {"phase": "test-running"})

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeMode", "full-repository"), "full-repository")

    def test_empty_plan_scope_falls_back_to_strict_full_repository_scan(self):
        project = _project({"legacy/src/main/java/example/BadService.java": _bad_code()})
        state = {
            "phase": "test-running",
            "verificationPlan": {"storyId": STORY_ID, "changedPaths": []},
        }

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeMode", "full-repository"), "full-repository")

    def test_test_and_document_only_scope_fails_closed(self):
        changed = ["src/test/java/example/CleanTest.java", "document/STORY-004.md"]
        project = _project({
            changed[0]: "package example; public class CleanTest {}\n",
            changed[1]: "# Story\n",
            "legacy/src/main/java/example/BadService.java": _bad_code(),
        })
        state, _ = _verified_state(project, changed)

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_NO_PRODUCTION_SCOPE")

    def test_all_supported_test_only_layouts_fail_without_production_scope(self):
        test_only_paths = (
            "module/src/integrationTest/java/example/Service.java",
            "module/src/testFixtures/java/example/Fixture.java",
            "module/tests/example/Helper.java",
            "module/src/main/java/example/ServiceIT.java",
            "module/src/main/java/example/ServiceTests.java",
            "module/src/main/java/example/ServiceSpec.java",
        )
        for changed in test_only_paths:
            with self.subTest(changed=changed):
                project = _project({changed: _clean_code()})
                state, _ = _verified_state(project, [changed])

                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_NO_PRODUCTION_SCOPE")

    def test_scanner_uses_same_test_only_classification(self):
        scanner = _load_scanner_module()
        test_only_paths = (
            "module/src/integrationTest/java/example/Service.java",
            "module/src/testFixtures/java/example/Fixture.java",
            "module/tests/example/Helper.java",
            "module/src/main/java/example/ServiceIT.java",
            "module/src/main/java/example/ServiceTests.java",
            "module/src/main/java/example/ServiceSpec.java",
        )
        project = _project({path: _clean_code() for path in test_only_paths})

        self.assertEqual(list(scanner.iter_code_files(project)), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
