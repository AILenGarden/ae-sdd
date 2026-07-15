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


def _hardcoded_url_code() -> str:
    return ('package example; public class BadUrlService '
            '{ private String endpoint = "https://api.example.com/v1"; }\n')


def _standard_maven_pom() -> str:
    return """<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <groupId>example</groupId>
  <artifactId>demo</artifactId>
  <version>1.0.0</version>
</project>
"""


def _scanner_report(project: Path, scanned_paths: list[str], *, status: str = "PASS",
                    findings: list[dict] | None = None, coding_reports: int = 0) -> dict:
    findings = findings or []
    blockers = sum(1 for finding in findings if finding.get("severity") == "BLOCKER")
    warnings = sum(1 for finding in findings if finding.get("severity") == "WARN")
    return {
        "root": str(project.resolve()),
        "status": status,
        "codeFiles": len(scanned_paths),
        "scannedPaths": scanned_paths,
        "codingReports": coding_reports,
        "reportStats": {
            "codeFiles": len(scanned_paths),
            "codingReports": coding_reports,
            "blockerFindings": blockers,
            "warnFindings": warnings,
        },
        "findings": findings,
    }


def _verified_state(project: Path, changed_paths: list[str]) -> tuple[dict, Path]:
    plan = verification_plan.build_plan(project, STORY_ID, changed_paths)
    command = "test_authenticity_scan.py"
    toolchain = "test-authenticity:v1"
    report = project / ".auto-engineering" / STORY_ID / "evidence" / "g09-report.json"
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps({
        "status": "PASS",
        "storyId": STORY_ID,
        "scope": sorted(changed_paths),
        "commandHash": evidence.command_hash(command),
        "toolchainFingerprint": toolchain,
    }), encoding="utf-8")
    evidence.record(
        project,
        STORY_ID,
        kind="test-authenticity",
        command=command,
        input_fingerprint=plan["planFingerprint"],
        toolchain_fingerprint=toolchain,
        exit_code=0,
        artifacts=[{
            "path": report.relative_to(project).as_posix(),
            "sha256": evidence.artifact_hash(report),
        }],
        summary={"gate": "G-09", "storyId": STORY_ID, "status": "PASS",
                 "changedPaths": sorted(changed_paths), "scope": sorted(changed_paths),
                 "commandHash": evidence.command_hash(command),
                 "toolchainFingerprint": toolchain,
                 "report": report.relative_to(project).as_posix()},
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

    def test_standard_maven_pom_namespaces_do_not_block_scoped_gate(self):
        changed = "pom.xml"
        project = _project({changed: _standard_maven_pom()})
        state, _ = _verified_state(project, [changed])

        result = self._check(project, state)

        self.assertTrue(result.pass_, result.message)
        self.assertEqual(result.details.get("n_blockers"), 0)
        self.assertEqual(result.details.get("scopePaths"), [changed])

    def test_real_java_hardcoded_external_url_remains_blocker(self):
        changed = "feature/src/main/java/example/BadUrlService.java"
        project = _project({changed: _hardcoded_url_code()})
        state, _ = _verified_state(project, [changed])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertIn("hardcoded-external-url", result.details.get("blocker_rules", []))

    def test_real_xml_hardcoded_external_url_remains_blocker(self):
        changed = "feature/src/main/resources/client.xml"
        project = _project({changed: '<client endpoint="https://api.example.com/v1"/>\n'})
        state, _ = _verified_state(project, [changed])

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertIn("hardcoded-external-url", result.details.get("blocker_rules", []))

    def test_uppercase_supported_extension_is_scanned_for_blockers(self):
        changed = "feature/src/main/java/example/BadService.JAVA"
        project = _project({changed: _bad_code()})
        state, _ = _verified_state(project, [changed])
        scanner = _load_scanner_module()

        result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertGreater(result.details.get("n_blockers", 0), 0)
        self.assertEqual(
            [path.relative_to(project).as_posix() for path in scanner.iter_code_files(project)],
            [changed],
        )

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
        self.assertEqual(result.details.get("evidenceReason"), "absent")

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
        self.assertEqual(result.details.get("evidenceReason"), "no-current-g09-entry")

    def test_evidence_semantic_bindings_fail_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        mutations = {
            "input-fingerprint": lambda entry: entry.update(inputFingerprint="sha256:other"),
            "command": lambda entry: entry.update(commandHash=evidence.command_hash("other-command")),
            "toolchain": lambda entry: entry.update(toolchainFingerprint="unknown-toolchain"),
            "summary-story": lambda entry: entry["summary"].update(storyId="STORY-999-BE"),
            "summary-status": lambda entry: entry["summary"].update(status="FAIL"),
            "summary-paths": lambda entry: entry["summary"].update(changedPaths=["other.java"]),
            "summary-scope": lambda entry: entry["summary"].update(scope=["other.java"]),
            "summary-command": lambda entry: entry["summary"].update(commandHash="sha256:other"),
            "summary-toolchain": lambda entry: entry["summary"].update(toolchainFingerprint="other"),
            "summary-report": lambda entry: entry["summary"].update(report="other.json"),
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
        self.assertEqual(result.details.get("evidenceReason"), "artifact-integrity")

    def test_artifact_path_must_be_project_relative_without_parent_traversal(self):
        changed = "feature/src/main/java/example/CleanService.java"
        cases = ("absolute-inside", "parent-traversal")
        for case in cases:
            with self.subTest(case=case):
                project = _project({changed: _clean_code()})
                state, report_path = _verified_state(project, [changed])
                outside = project.parent / f"{project.name}-outside-g09.json"
                if case == "parent-traversal":
                    outside.write_text(report_path.read_text(encoding="utf-8"), encoding="utf-8")
                    artifact_path = f"../{outside.name}"
                    artifact_hash = evidence.artifact_hash(outside)
                else:
                    artifact_path = str(report_path.resolve())
                    artifact_hash = evidence.artifact_hash(report_path)

                def mutate(entry):
                    entry["artifacts"][0] = {"path": artifact_path, "sha256": artifact_hash}
                    entry["summary"]["report"] = artifact_path

                _mutate_manifest(project, mutate)
                try:
                    result = self._check(project, state)
                finally:
                    outside.unlink(missing_ok=True)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")

    def test_report_command_and_toolchain_bind_entry_summary_and_report(self):
        changed = "feature/src/main/java/example/CleanService.java"
        mutations = {
            "command": ("commandHash", "sha256:synchronized-forgery"),
            "toolchain": ("toolchainFingerprint", "synchronized-forgery"),
        }
        for label, (field, forged) in mutations.items():
            with self.subTest(binding=label):
                project = _project({changed: _clean_code()})
                state, _ = _verified_state(project, [changed])

                def synchronize_entry_and_summary(entry):
                    entry[field] = forged
                    entry["summary"][field] = forged

                _mutate_manifest(project, synchronize_entry_and_summary)
                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_EVIDENCE_INVALID")
                self.assertEqual(result.details.get("evidenceReason"), f"report-{label}")

    def test_scoped_invalid_scanner_result_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        invalid_results = (
            SimpleNamespace(returncode=2, stdout=json.dumps(
                _scanner_report(project, [changed]))),
            SimpleNamespace(returncode=1, stdout=json.dumps(
                _scanner_report(project, [changed]))),
            SimpleNamespace(returncode=0, stdout=json.dumps(
                _scanner_report(project, [changed], status="FAIL"))),
            SimpleNamespace(returncode=0, stdout=json.dumps(
                _scanner_report(project, [changed], status="UNKNOWN"))),
            SimpleNamespace(returncode=0, stdout=json.dumps(
                _scanner_report(project, [changed], status="ERROR"))),
        )
        for scanner_result in invalid_results:
            with self.subTest(scanner_result=scanner_result), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)
                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_scoped_missing_scanner_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])

        with patch("lib.gates._locate_coding_scanner", return_value=None):
            result = self._check(project, state)

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_full_repository_missing_scanner_fails_closed(self):
        project = _project({"src/main/java/example/CleanService.java": _clean_code()})

        with patch("lib.gates._locate_coding_scanner", return_value=None):
            result = self._check(project, {"phase": "test-running"})

        self.assertFalse(result.pass_)
        self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_scoped_malformed_finding_paths_fail_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        for finding_path in ("", "../escape.java", "/absolute/escape.java",
                             str((project / changed).resolve()),
                             (project / changed).parent.relative_to(project).as_posix()):
            report = _scanner_report(
                project, [changed], status="FAIL",
                findings=[{"severity": "BLOCKER", "rule": "x", "path": finding_path}],
            )
            scanner_result = SimpleNamespace(returncode=1, stdout=json.dumps(report))
            with self.subTest(finding_path=finding_path), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)
                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_scoped_unknown_or_missing_finding_severity_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        for severity in ("ERROR", None, "blocker"):
            finding = {"rule": "x", "path": changed}
            if severity is not None:
                finding["severity"] = severity
            report = _scanner_report(project, [changed], findings=[finding])
            scanner_result = SimpleNamespace(returncode=0, stdout=json.dumps(report))
            with self.subTest(severity=severity), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_scoped_scanned_paths_attestation_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        other = "other/src/main/java/example/OtherService.java"
        project = _project({changed: _clean_code(), other: _clean_code()})
        state, _ = _verified_state(project, [changed])
        cases = {
            "missing": None,
            "empty": [],
            "wrong": [other],
            "parent": ["../escape.java"],
            "absolute": [str((project / changed).resolve())],
            "duplicate": [changed, changed],
            "wrong-type": changed,
        }
        for label, scanned_paths in cases.items():
            valid_paths = scanned_paths if isinstance(scanned_paths, list) else [changed]
            report = _scanner_report(project, valid_paths)
            if scanned_paths is not None:
                report["scannedPaths"] = scanned_paths
                if isinstance(scanned_paths, list):
                    report["codeFiles"] = len(scanned_paths)
                    report["reportStats"]["codeFiles"] = len(scanned_paths)
            else:
                report.pop("scannedPaths")
            scanner_result = SimpleNamespace(returncode=0, stdout=json.dumps(report))
            with self.subTest(case=label), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_scoped_scanner_report_schema_and_counters_fail_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, _ = _verified_state(project, [changed])
        mutations = {
            "root-mismatch": lambda report: report.update(root=str(project.parent.resolve())),
            "root-missing": lambda report: report.pop("root"),
            "root-type": lambda report: report.update(root=7),
            "code-files-zero": lambda report: report.update(codeFiles=0),
            "code-files-missing": lambda report: report.pop("codeFiles"),
            "code-files-type": lambda report: report.update(codeFiles="1"),
            "coding-reports-negative": lambda report: report.update(codingReports=-7),
            "coding-reports-missing": lambda report: report.pop("codingReports"),
            "coding-reports-type": lambda report: report.update(codingReports="0"),
            "stats-missing": lambda report: report.pop("reportStats"),
            "stats-type": lambda report: report.update(reportStats=[]),
            "stats-code-files": lambda report: report["reportStats"].update(codeFiles=0),
            "stats-coding-reports": lambda report: report["reportStats"].update(codingReports=1),
            "stats-blockers": lambda report: report["reportStats"].update(blockerFindings=1),
            "stats-warnings": lambda report: report["reportStats"].update(warnFindings=1),
        }
        for label, mutate in mutations.items():
            report = _scanner_report(project, [changed])
            mutate(report)
            scanner_result = SimpleNamespace(returncode=0, stdout=json.dumps(report))
            with self.subTest(case=label), patch(
                "lib.gates.runtime_exec.run_command", return_value=scanner_result
            ):
                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_SCAN_INVALID")

    def test_artifact_hash_tamper_fails_closed(self):
        changed = "feature/src/main/java/example/CleanService.java"
        project = _project({changed: _clean_code()})
        state, report = _verified_state(project, [changed])
        manifest = evidence.load_manifest(project, STORY_ID)
        snapshot = project / manifest["entries"][-1]["artifacts"][0]["snapshotPath"]
        snapshot.write_text("tampered", encoding="utf-8")

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

    def test_scanner_excluded_directories_do_not_count_as_production_scope(self):
        scanner = _load_scanner_module()
        self.assertEqual(
            gates._GCODE1_EXCLUDED_DIRS,
            {value.lower() for value in scanner.EXCLUDED_DIRS},
        )
        for excluded in ("build", "target"):
            with self.subTest(excluded=excluded):
                changed = f"module/{excluded}/generated/example/CleanService.java"
                project = _project({changed: _clean_code()})
                state, _ = _verified_state(project, [changed])

                result = self._check(project, state)

                self.assertFalse(result.pass_)
                self.assertEqual(result.details.get("scopeStatus"), "BLOCK_NO_PRODUCTION_SCOPE")
                self.assertEqual(list(scanner.iter_code_files(project)), [])

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

    def test_test_only_suffixes_cover_kotlin_without_excluding_production_names(self):
        scanner = _load_scanner_module()
        tests = (
            "module/src/main/kotlin/example/ServiceIT.kt",
            "module/src/main/kotlin/example/ServiceTests.kt",
            "module/src/main/kotlin/example/ServiceSpec.kt",
            "module/src/main/java/example/servicetest.java",
            "module/test/example/LegacyTest.java",
        )
        production = (
            "module/src/main/java/example/SpecService.java",
            "module/src/main/java/example/Audit.java",
        )
        project = _project({path: _clean_code() for path in tests + production})

        self.assertEqual(
            sorted(path.relative_to(project).as_posix() for path in scanner.iter_code_files(project)),
            sorted(production),
        )

    def test_text_code_extensions_are_scanned_in_verified_work_item_scope(self):
        production = {
            "tools/lib/clean_module.py": "def value():\n    return 1\n",
            "web/src/clean.js": "export const value = 1;\n",
            "web/src/clean.ts": "export const value: number = 1;\n",
        }
        project = _project({
            **production,
            "tools/tests/test_clean_module.py": "def test_value():\n    assert True\n",
            "web/tests/clean.spec.ts": "export const testValue = 1;\n",
        })
        changed = list(production)
        state, _ = _verified_state(project, changed)

        result = self._check(project, state)

        self.assertTrue(result.pass_, result.message)
        self.assertEqual(result.details.get("scopeStatus"), "VERIFIED")
        self.assertEqual(result.details.get("scopePaths"), sorted(changed))
        self.assertEqual(result.details.get("scannedPaths"), sorted(changed))

    def test_text_code_scanner_keeps_test_paths_out_of_production(self):
        scanner = _load_scanner_module()
        production = (
            "tools/lib/clean_module.py",
            "web/src/clean.js",
            "web/src/clean.ts",
        )
        tests = (
            "tools/tests/test_clean_module.py",
            "web/tests/clean.spec.ts",
            "web/src/clean.test.js",
        )
        project = _project({path: "value = 1\n" for path in production + tests})

        self.assertEqual(
            sorted(path.relative_to(project).as_posix() for path in scanner.iter_code_files(project)),
            sorted(production),
        )
        self.assertEqual(gates._gcode1_production_scope(list(tests)), [])

    def test_coding_scanner_self_hosting_has_no_blockers(self):
        scanner = _load_scanner_module()
        root = Path(__file__).resolve().parents[2]
        findings = []

        scanner.scan_code_file(root / "scripts" / "coding_authenticity_scan.py", root, findings)
        scanner.scan_code_file(root / "tools" / "lib" / "gates.py", root, findings)

        blockers = [
            (finding.rule, finding.path, finding.line)
            for finding in findings
            if finding.severity == "BLOCKER"
        ]
        self.assertEqual(blockers, [])

    def test_python_business_antipatterns_remain_blockers(self):
        scanner = _load_scanner_module()
        project = _project({
            "app/service.py": (
                'endpoint = "https://api.example.com/v1"\n'
                "timeout = 60\n"
                'legacy = "WebSecurityConfigurerAdapter"\n'
            ),
        })
        findings = []

        scanner.scan_code_file(project / "app" / "service.py", project, findings)

        blocker_rules = {
            finding.rule for finding in findings if finding.severity == "BLOCKER"
        }
        self.assertTrue({
            "hardcoded-external-url",
            "hardcoded-timeout-retry-ttl",
            "legacy-web-security-configurer-adapter",
        }.issubset(blocker_rules))

    def test_maven_metadata_uri_in_business_python_remains_blocker(self):
        scanner = _load_scanner_module()
        project = _project({
            "app/service.py": (
                'endpoint = "http://maven.apache.org/POM/4.0.0"\n'
                'schema = "http://www.w3.org/2001/XMLSchema-instance"\n'
            ),
        })
        findings = []

        scanner.scan_code_file(project / "app" / "service.py", project, findings)

        self.assertEqual(
            [f.rule for f in findings if f.severity == "BLOCKER"],
            ["hardcoded-external-url", "hardcoded-external-url"],
        )

    def test_virtualenv_dependencies_and_conventional_tests_are_excluded_consistently(self):
        scanner = _load_scanner_module()
        excluded = (
            ".venv/lib/pkg.py",
            "venv/lib/pkg.py",
            ".tox/env/pkg.py",
            "lib/site-packages/pkg.py",
            "web/__tests__/helper.ts",
            "app/test_service.py",
            "app/service_test.py",
            "web/service.test.ts",
            "web/service.spec.js",
        )
        project = _project({path: "value = 1\n" for path in excluded})

        self.assertEqual(list(scanner.iter_code_files(project)), [])
        self.assertEqual(gates._gcode1_production_scope(list(excluded)), [])

    def test_audit_and_spec_service_production_names_are_retained(self):
        scanner = _load_scanner_module()
        production = ("app/Audit.py", "web/SpecService.js")
        project = _project({path: "value = 1\n" for path in production})

        self.assertEqual(
            sorted(path.relative_to(project).as_posix() for path in scanner.iter_code_files(project)),
            sorted(production),
        )
        self.assertEqual(gates._gcode1_production_scope(list(production)), list(production))


if __name__ == "__main__":
    unittest.main(verbosity=2)
