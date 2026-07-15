"""Focused regression tests for RA scanner execution contracts."""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import runpy
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "tools"))

from lib import gates  # noqa: E402


GATE_WRAPPERS = (
    gates.check_ra_authenticity,
    gates.check_ra_flow_violation,
    gates.check_ra_depth,
    gates.check_ra_implementation,
)


VALID_BLOCKER_FINDING = {
    "severity": "BLOCKER",
    "rule": "TEST-RULE",
    "path": "ae-sdd-doc/RA/RA-STORY-001-v1.md",
    "line": 1,
    "message": "test blocker",
}


MALFORMED_FINDINGS = (
    ({}, "severity"),
    ({**VALID_BLOCKER_FINDING, "severity": "INFO"}, "severity"),
    ({key: value for key, value in VALID_BLOCKER_FINDING.items() if key != "message"}, "message"),
    ({**VALID_BLOCKER_FINDING, "line": "1"}, "line"),
    ({**VALID_BLOCKER_FINDING, "snippet": 123}, "snippet"),
    ({**VALID_BLOCKER_FINDING, "file": 123}, "file"),
    ({**VALID_BLOCKER_FINDING, "lineno": "1"}, "lineno"),
)


VALID_ERROR_REPORT = {
    "status": "ERROR",
    "raFiles": 0,
    "error": {
        "code": "INVALID_RA_SCAN_SCOPE",
        "message": "selected RA does not exist",
    },
}


MALFORMED_ERROR_REPORTS = (
    ({"status": "ERROR", "raFiles": 0}, "error"),
    ({**VALID_ERROR_REPORT, "error": {"code": "", "message": "scope error"}}, "error.code"),
    ({**VALID_ERROR_REPORT, "error": {"code": "INVALID", "message": 123}}, "error.message"),
    ({**VALID_ERROR_REPORT, "findings": [{}]}, "findings[0].severity"),
)


class TestRAGateScannerContracts(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.ra = self.root / "ae-sdd-doc" / "RA" / "RA-STORY-001-v1.md"
        self.ra.parent.mkdir(parents=True, exist_ok=True)
        self.ra.write_text("# Authoritative RA\n", encoding="utf-8")
        self.state = {
            "phase": "story-generated",
            "raDocPath": self.ra.relative_to(self.root).as_posix(),
        }
        self.master_source = REPO_ROOT / "source"

    def _run_all(self, completed):
        results = []
        with mock.patch.object(gates.runtime_exec, "run_command", return_value=completed):
            for wrapper in GATE_WRAPPERS:
                with self.subTest(wrapper=wrapper.__name__):
                    result = wrapper(
                        self.root,
                        self.state,
                        "STORY-001",
                        master_source=self.master_source,
                    )
                    self.assertFalse(result.pass_, result.message)
                    self.assertFalse(result.details.get("stub", False), result.details)
                    results.append(result)
        return results

    def test_exit_2_error_report_with_zero_files_blocks_all_wrappers(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=2,
            stdout=json.dumps(VALID_ERROR_REPORT),
            stderr="invalid selected scope",
        )

        results = self._run_all(completed)

        self.assertTrue(all(r.details.get("scanner_returncode") == 2 for r in results))
        self.assertTrue(all("INVALID_RA_SCAN_SCOPE" in r.message for r in results))

    def test_malformed_error_report_blocks_all_wrappers(self):
        for report, expected_field in MALFORMED_ERROR_REPORTS:
            with self.subTest(report=report, expected_field=expected_field):
                completed = subprocess.CompletedProcess(
                    args=["scanner"],
                    returncode=2,
                    stdout=json.dumps(report),
                    stderr="invalid scanner error envelope",
                )

                results = self._run_all(completed)

                self.assertTrue(all(expected_field in r.message for r in results))

    def test_nonzero_empty_output_blocks_all_wrappers(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=2,
            stdout="",
            stderr="scanner crashed",
        )

        results = self._run_all(completed)

        self.assertTrue(all("未输出 JSON" in r.message for r in results))

    def test_selected_ra_with_zero_scanned_files_is_never_a_stub(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=0,
            stdout=json.dumps({
                "status": "PASS",
                "raFiles": 0,
                "findings": [],
            }),
            stderr="",
        )

        results = self._run_all(completed)

        self.assertTrue(all(r.details.get("ra_files") == 0 for r in results))
        self.assertTrue(all("raFiles=0" in r.message for r in results))

    def test_exit_1_fail_report_preserves_blocker_evidence(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=1,
            stdout=json.dumps({
                "status": "FAIL",
                "raFiles": 1,
                "findings": [VALID_BLOCKER_FINDING],
            }),
            stderr="",
        )

        results = self._run_all(completed)

        self.assertTrue(all(r.details.get("blockers") == 1 for r in results))

    def test_non_object_finding_blocks_all_wrappers(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=0,
            stdout=json.dumps({
                "status": "PASS",
                "raFiles": 1,
                "findings": ["oops"],
            }),
            stderr="",
        )

        results = self._run_all(completed)

        self.assertTrue(all("findings" in r.message for r in results))

    def test_malformed_finding_schema_blocks_all_wrappers(self):
        for finding, expected_field in MALFORMED_FINDINGS:
            with self.subTest(finding=finding, expected_field=expected_field):
                completed = subprocess.CompletedProcess(
                    args=["scanner"],
                    returncode=0,
                    stdout=json.dumps({
                        "status": "PASS",
                        "raFiles": 1,
                        "findings": [finding],
                    }),
                    stderr="",
                )

                results = self._run_all(completed)

                self.assertTrue(all(expected_field in r.message for r in results))


class TestRACliScannerContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cli = runpy.run_path(str(REPO_ROOT / "tools" / "bin" / "ae-sdd"))
        cls.commands = (
            cls.cli["cmd_ra_authenticity_scan"],
            cls.cli["cmd_flow_violation_scan"],
            cls.cli["cmd_ra_depth_scan"],
            cls.cli["cmd_ra_implementation_scan"],
        )

    def _args(self):
        return argparse.Namespace(
            root=str(REPO_ROOT),
            file=[],
            json=True,
            strict=True,
        )

    def test_json_commands_reject_empty_and_malformed_scanner_output(self):
        cases = (
            (0, "", 4),
            (0, "{malformed", 4),
            (2, "", 2),
        )
        for command in self.commands:
            for returncode, stdout, expected in cases:
                with self.subTest(
                    command=command.__name__,
                    returncode=returncode,
                    stdout=stdout,
                ):
                    completed = subprocess.CompletedProcess(
                        args=["scanner"],
                        returncode=returncode,
                        stdout=stdout,
                        stderr="scanner failure",
                    )
                    with mock.patch.object(
                        self.cli["runtime_exec"],
                        "run_command",
                        return_value=completed,
                    ), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                        actual = command(self._args(), None)
                    self.assertEqual(actual, expected)
                    self.assertNotEqual(actual, 0)

    def test_json_commands_preserve_valid_fail_exit_code(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=1,
            stdout=json.dumps({
                "status": "FAIL",
                "raFiles": 1,
                "findings": [VALID_BLOCKER_FINDING],
            }),
            stderr="",
        )
        for command in self.commands:
            with self.subTest(command=command.__name__):
                with mock.patch.object(
                    self.cli["runtime_exec"],
                    "run_command",
                    return_value=completed,
                ), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    actual = command(self._args(), None)
                self.assertEqual(actual, 1)

    def test_json_commands_preserve_valid_error_envelope_and_nonzero_exit(self):
        encoded = json.dumps(VALID_ERROR_REPORT)
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=2,
            stdout=encoded,
            stderr="invalid selected scope",
        )
        for command in self.commands:
            with self.subTest(command=command.__name__):
                stdout = io.StringIO()
                with mock.patch.object(
                    self.cli["runtime_exec"],
                    "run_command",
                    return_value=completed,
                ), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(io.StringIO()):
                    actual = command(self._args(), None)
                self.assertEqual(actual, 2)
                self.assertEqual(json.loads(stdout.getvalue()), VALID_ERROR_REPORT)

    def test_json_commands_reject_malformed_error_envelopes(self):
        for command in self.commands:
            for report, expected_field in MALFORMED_ERROR_REPORTS:
                with self.subTest(
                    command=command.__name__,
                    report=report,
                    expected_field=expected_field,
                ):
                    completed = subprocess.CompletedProcess(
                        args=["scanner"],
                        returncode=2,
                        stdout=json.dumps(report),
                        stderr="invalid scanner error envelope",
                    )
                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with mock.patch.object(
                        self.cli["runtime_exec"],
                        "run_command",
                        return_value=completed,
                    ), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                        actual = command(self._args(), None)
                    self.assertEqual(actual, 2)
                    self.assertEqual(stdout.getvalue(), "")
                    self.assertIn(expected_field, stderr.getvalue())

    def test_json_commands_reject_non_object_findings(self):
        completed = subprocess.CompletedProcess(
            args=["scanner"],
            returncode=0,
            stdout=json.dumps({
                "status": "PASS",
                "raFiles": 1,
                "findings": ["oops"],
            }),
            stderr="",
        )
        for command in self.commands:
            with self.subTest(command=command.__name__):
                stderr = io.StringIO()
                with mock.patch.object(
                    self.cli["runtime_exec"],
                    "run_command",
                    return_value=completed,
                ), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
                    actual = command(self._args(), None)
                self.assertEqual(actual, 4)
                self.assertIn("findings[0]", stderr.getvalue())

    def test_json_commands_reject_malformed_finding_schema(self):
        for command in self.commands:
            for finding, expected_field in MALFORMED_FINDINGS:
                with self.subTest(
                    command=command.__name__,
                    finding=finding,
                    expected_field=expected_field,
                ):
                    completed = subprocess.CompletedProcess(
                        args=["scanner"],
                        returncode=0,
                        stdout=json.dumps({
                            "status": "PASS",
                            "raFiles": 1,
                            "findings": [finding],
                        }),
                        stderr="",
                    )
                    stderr = io.StringIO()
                    with mock.patch.object(
                        self.cli["runtime_exec"],
                        "run_command",
                        return_value=completed,
                    ), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
                        actual = command(self._args(), None)
                    self.assertEqual(actual, 4)
                    self.assertIn(f"findings[0].{expected_field}", stderr.getvalue())


if __name__ == "__main__":
    unittest.main(verbosity=2)
