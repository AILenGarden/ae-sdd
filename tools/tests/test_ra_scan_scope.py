"""Regression tests for authoritative RA scan scoping."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.ra_scan_scope import classify_formal_ra


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
IMPLEMENTATION_SCANNER = REPO_ROOT / "scripts" / "ra_implementation_scan.py"
CLI = REPO_ROOT / "tools" / "bin" / "ae-sdd"
BUILD_DIST = REPO_ROOT / "scripts" / "build_dist.py"
SCANNERS = (
    REPO_ROOT / "scripts" / "ra_authenticity_scan.py",
    REPO_ROOT / "scripts" / "ra_depth_scan.py",
    IMPLEMENTATION_SCANNER,
    REPO_ROOT / "scripts" / "flow_violation_scan.py",
)
CLI_SCAN_COMMANDS = (
    "ra-authenticity-scan",
    "ra-depth-scan",
    "ra-implementation-scan",
    "flow-violation-scan",
)
UTF8_SUBPROCESS_ENV = {**os.environ, "PYTHONIOENCODING": "utf-8"}


class TestExplicitRAScanScope(unittest.TestCase):
    def test_flow_scanner_accepts_markdown_headings_without_retired_generate_plan(self):
        root = Path(tempfile.mkdtemp())
        selected = root / "ae-sdd-doc" / "RA" / "RA-STORY-001.md"
        selected.parent.mkdir(parents=True, exist_ok=True)
        selected.write_text(
            "\n".join([
                "# RA", "",
                "## 0.5 RequirementAnalysisModel", "record",
                "## 2 \u89d2\u8272\u5206\u6790", "record",
                "## 3 \u573a\u666f\u5206\u6790", "record",
                "## 4 \u4e1a\u52a1\u6d41\u7a0b", "record",
                "## 5 \u6570\u636e\u8981\u7d20", "record",
                "## 6 \u4e1a\u52a1\u89c4\u5219", "record",
                "## 7 \u8bbe\u8ba1\u65b9\u5411", "record",
                "## 8 AC \u9a8c\u6536\u6807\u51c6", "record",
                "## 9 \u9690\u6027\u5047\u8bbe", "record",
                "## 10 \u7f3a\u53e3\u7ba1\u7406", "no blockers",
                "## 11 \u89c4\u6a21\u88c1\u5b9a", "large",
                "## 12 5 \u95ee\u81ea\u68c0", "pass rate 100%",
                "## 13 \u8def\u7531\u51b3\u7b56", "choose DR",
                "RA-G01 RA-G02 RA-G03 RA-G04",
            ]),
            encoding="utf-8",
        )

        result = subprocess.run(
            [
                sys.executable,
                str(REPO_ROOT / "scripts" / "flow_violation_scan.py"),
                "--root",
                str(root),
                "--file",
                str(selected),
                "--format",
                "json",
                "--strict",
            ],
            capture_output=True,
            text=True,
            check=False,
            encoding="utf-8",
            env=UTF8_SUBPROCESS_ENV,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "PASS")
        self.assertNotIn("RAGeneratePlan", selected.read_text(encoding="utf-8"))

    def test_formal_classifier_rejects_a_resolved_path_outside_root(self):
        root = Path(tempfile.mkdtemp()).resolve()
        outside_root = Path(tempfile.mkdtemp()).resolve()
        outside = outside_root / "RA-ESCAPE.md"
        outside.write_text("# outside\n", encoding="utf-8")

        accepted, reason = classify_formal_ra(outside, root)

        self.assertFalse(accepted)
        self.assertEqual(reason, "outside-scan-root")

    def test_build_dist_packages_the_shared_scope_helper(self):
        build_source = BUILD_DIST.read_text(encoding="utf-8")
        self.assertIn('"ra_scan_scope.py"', build_source)

    def test_file_scope_scans_only_the_selected_ra(self):
        root = Path(tempfile.mkdtemp())
        selected = root / "ae-sdd-doc" / "RA" / "RA-STORY-001-v1.md"
        selected.parent.mkdir(parents=True, exist_ok=True)
        selected.write_text("# Selected RA\n", encoding="utf-8")

        noise = root / "references" / "RA-third-party-guide.md"
        noise.parent.mkdir(parents=True, exist_ok=True)
        noise.write_text("# Unrelated RA-like guide\n", encoding="utf-8")

        result = subprocess.run(
            [
                sys.executable,
                str(IMPLEMENTATION_SCANNER),
                "--root",
                str(root),
                "--file",
                str(selected),
                "--format",
                "json",
            ],
            capture_output=True,
            text=True,
            check=False,
            encoding="utf-8",
            env=UTF8_SUBPROCESS_ENV,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["scopeMode"], "file")
        self.assertEqual(payload["raFiles"], 1)
        self.assertEqual(payload["selectedFiles"], ["ae-sdd-doc/RA/RA-STORY-001-v1.md"])
        self.assertTrue(payload["findings"])
        self.assertEqual(
            {finding["path"] for finding in payload["findings"]},
            {"ae-sdd-doc/RA/RA-STORY-001-v1.md"},
        )

    def test_all_ra_scanners_accept_the_same_repeatable_file_contract(self):
        root = Path(tempfile.mkdtemp())
        selected = root / "ae-sdd-doc" / "RA" / "RA-STORY-001-v1.md"
        selected.parent.mkdir(parents=True, exist_ok=True)
        selected.write_text("# Selected RA\n", encoding="utf-8")
        selected_two = root / "design" / "RA-STORY-002-v1.md"
        selected_two.parent.mkdir(parents=True, exist_ok=True)
        selected_two.write_text("# Second selected RA\n", encoding="utf-8")

        for scanner in SCANNERS:
            with self.subTest(scanner=scanner.name):
                result = subprocess.run(
                    [
                        sys.executable,
                        str(scanner),
                        "--root",
                        str(root),
                        "--file",
                        str(selected),
                        "--file",
                        str(selected_two),
                        "--format",
                        "json",
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                    encoding="utf-8",
                    env=UTF8_SUBPROCESS_ENV,
                )
                self.assertNotEqual(result.returncode, 2, result.stderr)
                payload = json.loads(result.stdout)
                self.assertEqual(payload["scopeMode"], "file")
                self.assertEqual(
                    payload["selectedFiles"],
                    [
                        "ae-sdd-doc/RA/RA-STORY-001-v1.md",
                        "design/RA-STORY-002-v1.md",
                    ],
                )
                self.assertEqual(payload["raFiles"], 2)

    def test_root_scope_excludes_ra_like_noise_and_generated_events(self):
        root = Path(tempfile.mkdtemp())
        files = {
            "ae-sdd-doc/RA/RA-STORY-001-v1.md": "# Canonical RA\n",
            "design/RA-LEGACY-001.md": "# Legacy RA\n",
            "design/legacy/archive/RA-LEGACY-002.md": "# Nested legacy RA\n",
            "references/RA-third-party-guide.md": "# Guide\n",
            "ae-sdd-doc/RA/RA-STORY-001-GeneratePlan-r1.md": "# Event\n",
            "ae-sdd-doc/RA/RA-STORY-001-Impact-r1.md": "# Impact event\n",
            "ae-sdd-doc/RA/RA-STORY-001-ReverseIssues.md": "# Reverse issues\n",
            "source/templates/design/RA-template-example.md": "# Template\n",
            "source/CHANGELOG/RA-release-note.md": "# Changelog\n",
            "dist/ae-sdd-doc/RA/RA-DIST-001.md": "# Built copy\n",
        }
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

        result = subprocess.run(
            [
                sys.executable,
                str(IMPLEMENTATION_SCANNER),
                "--root",
                str(root),
                "--format",
                "json",
            ],
            capture_output=True,
            text=True,
            check=False,
            encoding="utf-8",
            env=UTF8_SUBPROCESS_ENV,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["scopeMode"], "root")
        self.assertEqual(
            payload["selectedFiles"],
            [
                "ae-sdd-doc/RA/RA-STORY-001-v1.md",
                "design/legacy/archive/RA-LEGACY-002.md",
                "design/RA-LEGACY-001.md",
            ],
        )
        self.assertEqual(payload["raFiles"], 3)
        excluded = {item["path"]: item["reason"] for item in payload["excludedFiles"]}
        self.assertIn("references/RA-third-party-guide.md", excluded)
        self.assertIn("ae-sdd-doc/RA/RA-STORY-001-GeneratePlan-r1.md", excluded)
        self.assertIn("ae-sdd-doc/RA/RA-STORY-001-Impact-r1.md", excluded)
        self.assertIn("ae-sdd-doc/RA/RA-STORY-001-ReverseIssues.md", excluded)
        self.assertIn("source/templates/design/RA-template-example.md", excluded)
        self.assertIn("source/CHANGELOG/RA-release-note.md", excluded)
        self.assertIn("dist/ae-sdd-doc/RA/RA-DIST-001.md", excluded)

    def test_ae_sdd_cli_forwards_file_scope_to_all_ra_scanners(self):
        root = Path(tempfile.mkdtemp())
        selected = root / "ae-sdd-doc" / "RA" / "RA-STORY-001-v1.md"
        selected.parent.mkdir(parents=True, exist_ok=True)
        selected.write_text("# Selected RA\n", encoding="utf-8")

        for command in CLI_SCAN_COMMANDS:
            with self.subTest(command=command):
                result = subprocess.run(
                    [
                        sys.executable,
                        str(CLI),
                        command,
                        "--root",
                        str(root),
                        "--file",
                        str(selected),
                        "--json",
                    ],
                    cwd=str(REPO_ROOT),
                    capture_output=True,
                    text=True,
                    check=False,
                    encoding="utf-8",
                    env=UTF8_SUBPROCESS_ENV,
                )
                self.assertNotEqual(result.returncode, 2, result.stderr)
                payload = json.loads(result.stdout)
                self.assertEqual(payload["scopeMode"], "file")
                self.assertEqual(
                    payload["selectedFiles"],
                    ["ae-sdd-doc/RA/RA-STORY-001-v1.md"],
                )

    def test_invalid_explicit_file_returns_a_structured_scope_error(self):
        root = Path(tempfile.mkdtemp())
        missing = root / "ae-sdd-doc" / "RA" / "RA-MISSING.md"

        for scanner in SCANNERS:
            with self.subTest(scanner=scanner.name):
                result = subprocess.run(
                    [
                        sys.executable,
                        str(scanner),
                        "--root",
                        str(root),
                        "--file",
                        str(missing),
                        "--format",
                        "json",
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                    encoding="utf-8",
                    env=UTF8_SUBPROCESS_ENV,
                )
                self.assertEqual(result.returncode, 2)
                payload = json.loads(result.stdout)
                self.assertEqual(payload["status"], "ERROR")
                self.assertEqual(payload["error"]["code"], "INVALID_RA_SCAN_SCOPE")
                self.assertIn("does not exist", payload["error"]["message"])

    def test_ae_sdd_cli_preserves_invalid_scope_nonzero_exit(self):
        root = Path(tempfile.mkdtemp())
        missing = root / "ae-sdd-doc" / "RA" / "RA-MISSING.md"

        for command in CLI_SCAN_COMMANDS:
            with self.subTest(command=command):
                result = subprocess.run(
                    [
                        sys.executable,
                        str(CLI),
                        command,
                        "--root",
                        str(root),
                        "--file",
                        str(missing),
                        "--json",
                    ],
                    cwd=str(REPO_ROOT),
                    capture_output=True,
                    text=True,
                    check=False,
                    encoding="utf-8",
                    env=UTF8_SUBPROCESS_ENV,
                )
                self.assertNotEqual(result.returncode, 0)
                payload = json.loads(result.stdout)
                self.assertEqual(payload["status"], "ERROR")
                self.assertEqual(payload["error"]["code"], "INVALID_RA_SCAN_SCOPE")


if __name__ == "__main__":
    unittest.main(verbosity=2)
