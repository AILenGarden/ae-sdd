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
    """next_step_suggestion 测试 — 所有 10 个 phase"""

    def test_initialized_to_ra_generate(self):
        s = {"phase": "initialized"}
        sug = state_mod.next_step_suggestion(s)
        # v1.1: 'next' 必须与 PHASE_FLOW 一致（可直接传给 state write --phase）
        # 🆕 v3.4.0：initialized → ra-generated（RA 需求分析阶段）
        self.assertEqual(sug["next"], "ra-generated")

    def test_ra_generated_to_dr_generate(self):
        s = {"phase": "ra-generated"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "dr-generated")

    def test_dr_generated_to_story_generate(self):
        s = {"phase": "dr-generated"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "story-generated")

    def test_story_generated_to_story_review(self):
        s = {"phase": "story-generated"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "story-reviewed")

    def test_story_reviewed_to_testcase(self):
        s = {"phase": "story-reviewed"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "task-generated")

    def test_task_generated_to_task_review(self):
        s = {"phase": "task-generated"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "task-reviewed")

    def test_task_reviewed_to_coding_plan(self):
        s = {"phase": "task-reviewed"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "coding")

    def test_coding_to_test_run(self):
        s = {"phase": "coding"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "test-running")

    def test_test_running_to_coding_report(self):
        s = {"phase": "test-running"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "code-reviewed")

    def test_code_reviewed_to_user_confirm(self):
        s = {"phase": "code-reviewed"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "completed")

    def test_completed_terminates(self):
        s = {"phase": "completed"}
        sug = state_mod.next_step_suggestion(s)
        self.assertIn("已结束", sug["next"])

    def test_all_10_phases_have_suggestion(self):
        """所有 10 个 phase 都该有建议"""
        for phase in state_mod.PHASE_FLOW:
            sug = state_mod.next_step_suggestion({"phase": phase})
            self.assertIn("current", sug)
            self.assertIn("next", sug)
            self.assertIn("action", sug)
            self.assertIn("skill", sug)


class TestPhaseFlowCoverage(unittest.TestCase):
    """PHASE_FLOW 完整性测试"""

    def test_phase_flow_has_11_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOW), 11)  # 🆕 v3.4.0: +ra-generated

    def test_phase_flow_starts_with_initialized(self):
        self.assertEqual(state_mod.PHASE_FLOW[0], "initialized")

    def test_phase_flow_ends_with_completed(self):
        self.assertEqual(state_mod.PHASE_FLOW[-1], "completed")


if __name__ == "__main__":
    unittest.main(verbosity=2)
