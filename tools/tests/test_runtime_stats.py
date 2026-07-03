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
