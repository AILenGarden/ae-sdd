"""
test_state.py — state.py 单元测试

覆盖：
- read_state（空 + 已有）
- write_state（原子写：tmp + rename）
- record_history
- set_phase（合法/非法 phase）
- next_step_suggestion（所有 10 个 phase）
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import state as state_mod  # noqa: E402


class TestReadState(unittest.TestCase):
    """read_state 测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_read_nonexistent_returns_default(self):
        s = state_mod.read_state(self.state_path)
        self.assertEqual(s["version"], "1")
        self.assertEqual(s["phase"], "initialized")
        self.assertIsNone(s["currentStory"])
        self.assertIsNone(s["currentTask"])
        self.assertEqual(s["history"], [])

    def test_read_existing(self):
        self.state_path.write_text(json.dumps({
            "version": "1",
            "projectKey": "test",
            "phase": "story-generated",
            "currentStory": "STORY-001",
            "currentTask": None,
            "history": [{"phase": "initialized", "timestamp": "2026-06-18T00:00:00Z", "by": "init"}],
        }, ensure_ascii=False), encoding="utf-8")
        s = state_mod.read_state(self.state_path)
        self.assertEqual(s["phase"], "story-generated")
        self.assertEqual(s["currentStory"], "STORY-001")
        self.assertEqual(len(s["history"]), 1)


class TestWriteState(unittest.TestCase):
    """write_state 测试（原子写）"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_write_creates_file(self):
        s = state_mod.read_state(self.state_path)
        state_mod.set_phase(s, "dr-generated")
        state_mod.write_state(self.state_path, s)
        self.assertTrue(self.state_path.is_file())
        reloaded = json.loads(self.state_path.read_text(encoding="utf-8"))
        self.assertEqual(reloaded["phase"], "dr-generated")

    def test_write_atomic_no_leftover_tmp(self):
        """写完后不应残留 .tmp 文件"""
        s = state_mod.read_state(self.state_path)
        state_mod.write_state(self.state_path, s)
        self.assertFalse((self.state_path.with_suffix(".json.tmp")).exists())

    def test_write_overwrites_existing(self):
        self.state_path.write_text('{"phase": "old"}', encoding="utf-8")
        s = state_mod.read_state(self.state_path)
        s["phase"] = "new"
        state_mod.write_state(self.state_path, s)
        reloaded = json.loads(self.state_path.read_text(encoding="utf-8"))
        self.assertEqual(reloaded["phase"], "new")


class TestSetPhase(unittest.TestCase):
    """set_phase 测试"""

    def test_set_valid_phase(self):
        s = {"history": []}
        state_mod.set_phase(s, "dr-generated", by="test")
        self.assertEqual(s["phase"], "dr-generated")
        self.assertEqual(len(s["history"]), 1)
        self.assertEqual(s["history"][0]["by"], "test")
        self.assertIn("timestamp", s["history"][0])

    def test_set_invalid_phase_raises(self):
        s = {"history": []}
        with self.assertRaises(ValueError) as ctx:
            state_mod.set_phase(s, "totally-bogus-phase")
        self.assertIn("未知 phase", str(ctx.exception))

    def test_default_by_is_ae_sdd(self):
        s = {"history": []}
        state_mod.set_phase(s, "initialized")
        self.assertEqual(s["history"][0]["by"], "ae-sdd")


class TestRecordHistory(unittest.TestCase):
    """record_history 测试"""

    def test_records_timestamp(self):
        s = {"history": []}
        state_mod.record_history(s, "story-generated", by="story-generate-skill")
        self.assertEqual(len(s["history"]), 1)
        entry = s["history"][0]
        self.assertEqual(entry["phase"], "story-generated")
        self.assertEqual(entry["by"], "story-generate-skill")
        # ISO 8601 UTC 格式
        self.assertRegex(entry["timestamp"], r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

    def test_appends_in_order(self):
        s = {"history": []}
        state_mod.record_history(s, "phase-1")
        state_mod.record_history(s, "phase-2")
        state_mod.record_history(s, "phase-3")
        self.assertEqual([h["phase"] for h in s["history"]],
                         ["phase-1", "phase-2", "phase-3"])


class TestNextStepSuggestion(unittest.TestCase):
    """next_step_suggestion 测试 — 🆕 v3.5.15 按 scale 选子链"""

    def test_initialized_to_ra_generate_large(self):
        """大链 initialized → ra-generated"""
        s = {"phase": "initialized", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "ra-generated")

    def test_micro_initialized_to_coding(self):
        """🆕 v3.5.15 修复可观测 bug：微链 initialized → coding（不再误建议跑 RA）"""
        s = {"phase": "initialized", "scale": "微"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "coding")
        self.assertIn("coding-skill.md", sug["skill"])

    def test_small_ra_to_task(self):
        """小链 ra-generated → task-generated（跳过 DR/Story）"""
        s = {"phase": "ra-generated", "scale": "小"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "task-generated")

    def test_medium_ra_to_story(self):
        """中链 ra-generated → story-generated（跳过 DR）"""
        s = {"phase": "ra-generated", "scale": "中"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "story-generated")

    def test_large_ra_to_dr(self):
        """大链 ra-generated → dr-generated"""
        s = {"phase": "ra-generated", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "dr-generated")

    def test_completed_terminates(self):
        s = {"phase": "completed", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertIn("已结束", sug["next"])

    def test_all_phases_have_suggestion_per_chain(self):
        """🆕 v3.5.15 每条子链每个 phase 都该有建议"""
        for scale, chain in state_mod.PHASE_FLOWS.items():
            for phase in chain:
                sug = state_mod.next_step_suggestion({"phase": phase, "scale": scale})
                self.assertIn("current", sug, f"scale={scale} phase={phase} 缺 current")
                self.assertIn("next", sug, f"scale={scale} phase={phase} 缺 next")
                self.assertIn("action", sug, f"scale={scale} phase={phase} 缺 action")
                self.assertIn("skill", sug, f"scale={scale} phase={phase} 缺 skill")


class TestPhaseFlowCoverage(unittest.TestCase):
    """🆕 v3.5.15 PHASE_FLOWS 4 子链完整性测试"""

    def test_phase_flows_has_4_scales(self):
        self.assertEqual(set(state_mod.PHASE_FLOWS.keys()),
                         {"大", "中", "小", "微"})

    def test_large_chain_has_11_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["大"]), 11)

    def test_medium_chain_has_10_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["中"]), 10)

    def test_small_chain_has_8_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["小"]), 8)

    def test_micro_chain_has_4_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["微"]), 4)

    def test_all_chains_start_with_initialized(self):
        for scale, chain in state_mod.PHASE_FLOWS.items():
            self.assertEqual(chain[0], "initialized", f"{scale} 链起点非 initialized")

    def test_all_chains_end_with_completed(self):
        for scale, chain in state_mod.PHASE_FLOWS.items():
            self.assertEqual(chain[-1], "completed", f"{scale} 链终点非 completed")

    def test_micro_chain_skips_ra(self):
        """微链不含 ra-generated（核心修复点）"""
        self.assertNotIn("ra-generated", state_mod.PHASE_FLOWS["微"])

    def test_phase_flow_alias_is_large_chain(self):
        """向后兼容：PHASE_FLOW 别名 = 大链"""
        self.assertEqual(state_mod.PHASE_FLOW, state_mod.PHASE_FLOWS["大"])


class TestSetScale(unittest.TestCase):
    """🆕 v3.5.15 set_scale 测试"""

    def test_set_valid_scale(self):
        s = {}
        state_mod.set_scale(s, "微")
        self.assertEqual(s["scale"], "微")

    def test_set_scale_with_entry_node(self):
        s = {}
        state_mod.set_scale(s, "微", entry_node="BUG")
        self.assertEqual(s["scale"], "微")
        self.assertEqual(s["entryNode"], "BUG")

    def test_set_invalid_scale_raises(self):
        s = {}
        with self.assertRaises(ValueError) as ctx:
            state_mod.set_scale(s, "巨")
        self.assertIn("未知 scale", str(ctx.exception))


class TestInferScale(unittest.TestCase):
    """🆕 v3.5.15 _infer_scale 旧 state 兼容测试"""

    def test_infer_large_from_dr_step(self):
        s = {"completedSteps": ["step-1-dr2story"], "phase": "story-generated"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "大")
        self.assertGreaterEqual(conf, 0.8)

    def test_infer_medium_from_story_step(self):
        s = {"completedSteps": ["step-1-story"], "phase": "story-reviewed"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "中")

    def test_infer_small_from_task_step(self):
        s = {"completedSteps": ["step-1-task"], "phase": "task-reviewed"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "小")

    def test_infer_coding_phase_defaults_large(self):
        """🟡 v3.5.15 安全策略：coding 阶段无法可靠区分微 vs 大，默认大（最保守）。
        微任务必须显式 --scale=微，不靠反推。"""
        s = {"completedSteps": [], "phase": "coding"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "大")
        self.assertLess(conf, 0.5, "coding 阶段反推置信度应低，提示用户显式 --scale")

    def test_infer_default_large_when_unknown(self):
        """无法判定 → 默认大（最保守）"""
        s = {"completedSteps": [], "phase": "initialized"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "大")
        self.assertLess(conf, 0.5, "无法判定时置信度应低，提示用户显式 --scale")


class TestSetPhasePerScale(unittest.TestCase):
    """🆕 v3.5.15 set_phase 按 scale 子链校验"""

    def test_micro_set_coding_from_initialized(self):
        """微链允许 initialized → coding"""
        s = {"phase": "initialized", "scale": "微", "history": []}
        state_mod.set_phase(s, "coding")
        self.assertEqual(s["phase"], "coding")

    def test_large_set_coding_is_valid_phase(self):
        """set_phase 只校验 phase ∈ 子链，不拦跨步跳跃（跨步由 gate_intercept 管）。
        coding 在大链内 → 合法 phase，不抛 ValueError。"""
        s = {"phase": "initialized", "scale": "大", "history": []}
        # coding 在大链内，set_phase 不拦（跨步跳跃拦截是 gate_intercept 职责，M2 处理）
        state_mod.set_phase(s, "coding")
        self.assertEqual(s["phase"], "coding")

    def test_set_phase_outside_chain_blocked(self):
        """phase 不在当前 scale 子链内 → ValueError。
        微链不含 ra-generated，微任务 set ra-generated 应被拦。"""
        s = {"phase": "initialized", "scale": "微", "history": []}
        with self.assertRaises(ValueError):
            state_mod.set_phase(s, "ra-generated")

    def test_set_bogus_phase_blocked(self):
        """完全非法 phase 名 → ValueError"""
        s = {"phase": "initialized", "scale": "大", "history": []}
        with self.assertRaises(ValueError):
            state_mod.set_phase(s, "totally-bogus-phase")

    def test_legacy_state_no_scale_infers_and_allows(self):
        """旧 state 无 scale → _resolve_scale 反推后 set_phase 正常工作"""
        s = {"phase": "initialized", "history": []}  # 无 scale 字段
        # 反推：无 completedSteps + initialized → 默认大链
        # initialized 在大链内，合法
        state_mod.set_phase(s, "ra-generated")
        self.assertEqual(s["phase"], "ra-generated")
        # 反推结果应回写
        self.assertEqual(s["scale"], "大")


if __name__ == "__main__":
    unittest.main(verbosity=2)
