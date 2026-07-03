import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CLI = REPO_ROOT / "tools" / "bin" / "ae-sdd"


class CliPerfTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.stats_dir = Path(self.tmp.name) / "stats"
        self.env = os.environ.copy()
        self.env["AE_SDD_STATS_DIR"] = str(self.stats_dir)
        self.env["PYTHONIOENCODING"] = "utf-8"
        self.env["PYTHONUTF8"] = "1"

    def tearDown(self):
        self.tmp.cleanup()

    def _run(self, *args):
        return subprocess.run(
            [sys.executable, str(CLI), *args],
            cwd=str(REPO_ROOT),
            env=self.env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=30,
            check=False,
        )

    def test_perf_report_doctor_and_clear(self):
        version = self._run("version", "--json")
        self.assertEqual(version.returncode, 0, version.stderr)

        report = self._run("perf", "report", "--json", "--last", "10")
        self.assertEqual(report.returncode, 0, report.stderr)
        payload = json.loads(report.stdout)
        self.assertEqual(payload["statsDir"], str(self.stats_dir))
        self.assertGreaterEqual(payload["summary"]["count"], 1)
        self.assertTrue(
            any(item["command"] == "version" for item in payload["summary"]["commands"])
        )

        doctor = self._run("perf", "doctor", "--json", "--last", "10")
        self.assertEqual(doctor.returncode, 0, doctor.stderr)
        doctor_payload = json.loads(doctor.stdout)
        self.assertTrue(doctor_payload["advice"])

        clear = self._run("perf", "clear", "--json")
        self.assertEqual(clear.returncode, 0, clear.stderr)
        clear_payload = json.loads(clear.stdout)
        self.assertGreaterEqual(clear_payload["deletedFiles"], 1)

        empty = self._run("perf", "report", "--json", "--last", "10")
        self.assertEqual(empty.returncode, 0, empty.stderr)
        empty_payload = json.loads(empty.stdout)
        self.assertEqual(empty_payload["summary"]["count"], 0)
