import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import runtime_exec, runtime_stats  # noqa: E402


class RuntimeStatsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.stats_dir = Path(self.tmp.name) / "stats"
        self.old_stats_dir = os.environ.get("AE_SDD_STATS_DIR")
        self.old_stats = os.environ.get("AE_SDD_STATS")
        os.environ["AE_SDD_STATS_DIR"] = str(self.stats_dir)
        os.environ.pop("AE_SDD_STATS", None)

    def tearDown(self):
        runtime_stats.suppress_current_event()
        runtime_stats.finish_command(0)
        if self.old_stats_dir is None:
            os.environ.pop("AE_SDD_STATS_DIR", None)
        else:
            os.environ["AE_SDD_STATS_DIR"] = self.old_stats_dir
        if self.old_stats is None:
            os.environ.pop("AE_SDD_STATS", None)
        else:
            os.environ["AE_SDD_STATS"] = self.old_stats
        self.tmp.cleanup()

    def test_records_command_and_span(self):
        runtime_stats.start_command("unit test", argv=["--token", "abc", "--plain", "ok"])
        with runtime_stats.span("unit-span", {"path": Path("x")}):
            pass
        runtime_stats.finish_command(0)

        events = runtime_stats.read_events(limit=10)
        self.assertEqual(len(events), 1)
        event = events[0]
        self.assertEqual(event["command"], "unit test")
        self.assertEqual(event["argv"], ["--token", "***", "--plain", "ok"])
        self.assertEqual(event["exitCode"], 0)
        self.assertGreaterEqual(event["durationMs"], 0)
        self.assertEqual(event["spans"][0]["name"], "unit-span")
        self.assertEqual(event["spans"][0]["attrs"]["path"], "x")

    def test_runtime_exec_records_utf8_subprocess_span(self):
        runtime_stats.start_command("unit exec", argv=[])
        result = runtime_exec.run_command(
            [sys.executable, "-c", "print('中文')"],
            timeout=10,
            span_name="unit:exec",
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("中文", result.stdout)
        runtime_stats.finish_command(0)

        events = runtime_stats.read_events(limit=10)
        self.assertEqual(len(events), 1)
        spans = events[0]["spans"]
        self.assertEqual(spans[0]["name"], "unit:exec")
        self.assertEqual(spans[0]["attrs"]["exitCode"], 0)
        self.assertGreater(spans[0]["attrs"]["stdoutChars"], 0)

    def test_clear_events_removes_jsonl_files(self):
        runtime_stats.start_command("unit clear", argv=[])
        runtime_stats.finish_command(0)
        self.assertEqual(len(runtime_stats.read_events(limit=10)), 1)

        deleted = runtime_stats.clear_events()
        self.assertEqual(deleted, 1)
        self.assertEqual(runtime_stats.read_events(limit=10), [])

    def test_summarize_events_groups_by_scale(self):
        """🆕 2026-07-03(B3): summarize_events 按 scale 分桶 + scaleRatios 诊断比例失调"""
        import json as _json
        # 造 3 个事件：2 个微任务（快）、1 个大任务（慢）
        events = [
            {"command": "gates check", "durationMs": 100.0, "scale": "微", "startedAt": "2026-07-03T00:00:00Z", "spans": [], "attrs": {}},
            {"command": "gates check", "durationMs": 150.0, "scale": "微", "startedAt": "2026-07-03T00:01:00Z", "spans": [], "attrs": {}},
            {"command": "gates check", "durationMs": 6000.0, "scale": "大", "startedAt": "2026-07-03T00:02:00Z", "spans": [], "attrs": {}},
        ]
        summary = runtime_stats.summarize_events(events)
        by_scale = {b["scale"]: b for b in summary["byScale"]}
        self.assertIn("微", by_scale)
        self.assertIn("大", by_scale)
        self.assertEqual(by_scale["微"]["count"], 2)
        self.assertEqual(by_scale["大"]["count"], 1)
        # 微任务平均 (100+150)/2 = 125，大任务 6000
        self.assertAlmostEqual(by_scale["微"]["avgMs"], 125.0, places=1)
        self.assertAlmostEqual(by_scale["大"]["avgMs"], 6000.0, places=1)
        # 比例 微/大 = 125/6000 ≈ 0.021（远低于 0.8，不告警）
        self.assertIn("微/大", summary["scaleRatios"])
        self.assertLess(summary["scaleRatios"]["微/大"], 0.8)

    def test_summarize_events_scale_ratio_flags_imbalance(self):
        """🆕 2026-07-03(B3): 微任务开销接近大任务时 ratio ≥ 0.8（比例失调信号）"""
        events = [
            {"command": "gates check", "durationMs": 5000.0, "scale": "微", "startedAt": "2026-07-03T00:00:00Z", "spans": [], "attrs": {}},
            {"command": "gates check", "durationMs": 6000.0, "scale": "大", "startedAt": "2026-07-03T00:01:00Z", "spans": [], "attrs": {}},
        ]
        summary = runtime_stats.summarize_events(events)
        # 微/大 = 5000/6000 ≈ 0.833 ≥ 0.8 → 比例失调
        self.assertGreaterEqual(summary["scaleRatios"]["微/大"], 0.8)

    def test_detect_scale_reads_state_json(self):
        """🆕 2026-07-03(B3): _detect_scale 从项目 state.json 读 scale"""
        import json as _json
        project_root = Path(self.tmp.name) / "proj"
        ae_sdd_dir = project_root / ".ae-sdd"
        ae_sdd_dir.mkdir(parents=True)
        (ae_sdd_dir / "state.json").write_text(
            _json.dumps({"scale": "微", "phase": "coding"}), encoding="utf-8"
        )
        scale = runtime_stats._detect_scale(project_root)
        self.assertEqual(scale, "微")

    def test_detect_scale_returns_none_without_state(self):
        """🆕 2026-07-03(B3): 无 state.json 时 _detect_scale 返回 None（静默）"""
        project_root = Path(self.tmp.name) / "empty_proj"
        project_root.mkdir(parents=True)
        scale = runtime_stats._detect_scale(project_root)
        self.assertIsNone(scale)
