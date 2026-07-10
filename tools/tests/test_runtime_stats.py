import os
import sys
import tempfile
import time
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
        """🆕 2026-07-03(B3): _detect_scale 从项目 state.json 读 scale

        v3.9.13 起 state 源从项目级 .ae-sdd/state.json 改为 task-scoped
        .auto-engineering/<work-item>/state.json（resolve_default_state 扫描
        .auto-engineering/*/state.json，恰好 1 个未 completed 的 work-item 命中）。
        故 fixture 落到 work-item state，scale 字段保持原值"微"。
        """
        import json as _json
        project_root = Path(self.tmp.name) / "proj"
        ae_sdd_dir = project_root / ".ae-sdd"
        ae_sdd_dir.mkdir(parents=True)
        # work-item state：落在 .auto-engineering/<work-item>/state.json
        work_item_dir = project_root / ".auto-engineering" / "Story-001"
        work_item_dir.mkdir(parents=True)
        (work_item_dir / "state.json").write_text(
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

    def test_bootstrap_ms_recorded_from_env(self):
        """🆕 2026-07-03 缺口1:入口 env 戳 → 事件含 bootstrapMs>0(顶层字段)"""
        # 模拟入口在 import 前打的戳(略早于 start_command)
        boot_ns = time.perf_counter_ns() - 1_000_000  # 1ms 前
        os.environ["AE_SDD_BOOT_NS"] = str(boot_ns)
        try:
            runtime_stats.start_command("boot test", argv=[])
            runtime_stats.finish_command(0)
            events = runtime_stats.read_events(limit=10)
            self.assertEqual(len(events), 1)
            # bootstrapMs 是顶层字段(与 scale 同级),值 > 0
            self.assertIn("bootstrapMs", events[0])
            self.assertGreater(events[0]["bootstrapMs"], 0.0)
        finally:
            os.environ.pop("AE_SDD_BOOT_NS", None)

    def test_bootstrap_ms_absent_without_env(self):
        """🆕 2026-07-03 缺口1:无 env 戳 → 事件无 bootstrapMs 字段(向后兼容)"""
        os.environ.pop("AE_SDD_BOOT_NS", None)
        runtime_stats.start_command("no boot", argv=[])
        runtime_stats.finish_command(0)
        events = runtime_stats.read_events(limit=10)
        self.assertEqual(len(events), 1)
        # 无戳时不应有 bootstrapMs 字段(旧事件/子进程未继承的兼容形态)
        self.assertNotIn("bootstrapMs", events[0])

    def test_finish_command_clears_boot_env(self):
        """🆕 2026-07-03 缺口1:finish_command 后清理 env 戳,防子进程继承错误戳"""
        os.environ["AE_SDD_BOOT_NS"] = str(time.perf_counter_ns())
        runtime_stats.start_command("clear boot", argv=[])
        runtime_stats.finish_command(0)
        # finish 后 env 应被 pop
        self.assertNotIn("AE_SDD_BOOT_NS", os.environ)

    def test_summarize_cpu_and_iowait(self):
        """🆕 2026-07-03 缺口3:summarize 含 cpuMs/ioWaitMs 分桶,ioWait=duration−cpu"""
        events = [
            {"command": "gates check", "durationMs": 1000.0, "cpuMs": 200.0,
             "startedAt": "2026-07-03T00:00:00Z", "spans": [], "attrs": {}},
            {"command": "gates check", "durationMs": 2000.0, "cpuMs": 300.0,
             "startedAt": "2026-07-03T00:01:00Z", "spans": [], "attrs": {}},
        ]
        summary = runtime_stats.summarize_events(events)
        # cpuMs 分桶
        self.assertIn("cpuMs", summary)
        self.assertAlmostEqual(summary["cpuMs"]["avgMs"], 250.0, places=1)
        # ioWaitMs = duration − cpu,事件1=800,事件2=1700,avg=1250
        self.assertIn("ioWaitMs", summary)
        self.assertAlmostEqual(summary["ioWaitMs"]["avgMs"], 1250.0, places=1)
        # commands 桶含 avgCpuMs
        cmds = {c["command"]: c for c in summary["commands"]}
        self.assertIn("avgCpuMs", cmds["gates check"])
        self.assertAlmostEqual(cmds["gates check"]["avgCpuMs"], 250.0, places=1)

    def test_summarize_by_scale_includes_cpu(self):
        """🆕 2026-07-03 缺口3:byScale 桶含 avgCpuMs/avgIoWaitMs"""
        events = [
            {"command": "gates check", "durationMs": 1000.0, "cpuMs": 200.0,
             "scale": "大", "startedAt": "2026-07-03T00:00:00Z", "spans": [], "attrs": {}},
        ]
        summary = runtime_stats.summarize_events(events)
        by_scale = {b["scale"]: b for b in summary["byScale"]}
        self.assertIn("avgCpuMs", by_scale["大"])
        self.assertAlmostEqual(by_scale["大"]["avgCpuMs"], 200.0, places=1)
        # ioWait = 1000 − 200 = 800
        self.assertAlmostEqual(by_scale["大"]["avgIoWaitMs"], 800.0, places=1)

    def test_summarize_bootstrap_ms_bucket(self):
        """🆕 2026-07-03 缺口1:summarize 含 bootstrapMs 分桶(仅含已打戳事件)"""
        events = [
            {"command": "version", "durationMs": 0.1, "cpuMs": 0.0, "bootstrapMs": 190.0,
             "startedAt": "2026-07-03T00:00:00Z", "spans": [], "attrs": {}},
            {"command": "version", "durationMs": 0.2, "cpuMs": 0.0, "bootstrapMs": 210.0,
             "startedAt": "2026-07-03T00:01:00Z", "spans": [], "attrs": {}},
        ]
        summary = runtime_stats.summarize_events(events)
        boot = summary["bootstrapMs"]
        self.assertEqual(boot["count"], 2)
        self.assertAlmostEqual(boot["avgMs"], 200.0, places=1)
        # slowestCommands 也应携带 bootstrapMs
        self.assertEqual(summary["slowestCommands"][0]["bootstrapMs"], 210.0)

    def test_runtime_exec_attrs_merged_into_span(self):
        """🆕 2026-07-03 缺口5:run_command 的 attrs 形参合并进 span attrs(scanRoot 等)"""
        runtime_stats.start_command("attr test", argv=[])
        runtime_exec.run_command(
            [sys.executable, "-c", "pass"],
            timeout=10,
            span_name="unit:attr",
            attrs={"scanRoot": "/tmp/proj"},
        )
        runtime_stats.finish_command(0)
        events = runtime_stats.read_events(limit=10)
        spans = events[0]["spans"]
        # 调用方传入的 scanRoot 应出现在 attrs
        self.assertEqual(spans[0]["attrs"]["scanRoot"], "/tmp/proj")
        # 内置 attrs(argsCount/arg0)也在
        self.assertIn("argsCount", spans[0]["attrs"])
        self.assertIn("arg0", spans[0]["attrs"])
