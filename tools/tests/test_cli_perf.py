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

    def test_perf_report_includes_bootstrap(self):
        """🆕 2026-07-03 缺口1:真实 CLI 跑 version → perf report 含 bootstrapMs 维度"""
        # 跑一次 version,入口打戳 → 事件应含 bootstrapMs
        version = self._run("version", "--json")
        self.assertEqual(version.returncode, 0, version.stderr)

        report = self._run("perf", "report", "--json", "--last", "10")
        self.assertEqual(report.returncode, 0, report.stderr)
        payload = json.loads(report.stdout)
        summary = payload["summary"]
        # bootstrapMs 分桶应存在
        self.assertIn("bootstrapMs", summary)
        # 至少有 1 个已打戳事件(version)
        self.assertGreaterEqual(summary["bootstrapMs"]["count"], 1)
        # 真实 import 成本应 > 50ms(实测 ~200ms,给宽松下限防抖动)
        self.assertGreater(summary["bootstrapMs"]["avgMs"], 50.0)

    def test_perf_doctor_bootstrap_advice(self):
        """🆕 2026-07-03 缺口4:高 bootstrap → doctor advice 含 lazy import 建议(真实数据驱动)"""
        # 跑 version 触发打戳
        self._run("version", "--json")
        doctor = self._run("perf", "doctor", "--json", "--last", "10")
        self.assertEqual(doctor.returncode, 0, doctor.stderr)
        payload = json.loads(doctor.stdout)
        # advice 应提及 lazy import 或 import 固定成本(基于真实 bootstrapMs>150)
        advice_text = " ".join(payload["advice"])
        # 宽松断言:要么提到 lazy import,要么提到 I/O(取决于实际数据),至少有内容
        self.assertTrue(payload["advice"])
        # bootstrap 真实 >150ms 时应触发 lazy import 建议
        boot_p95 = payload["summary"].get("bootstrapMs", {}).get("p95Ms", 0)
        if boot_p95 > 150:
            self.assertTrue(
                "lazy import" in advice_text or "import 固定成本" in advice_text,
                f"bootstrap p95={boot_p95}>150 但 advice 未提 lazy import: {payload['advice']}"
            )
