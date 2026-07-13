"""
test_subprocess_agent.py - subprocess agent management tests (🆕 v3.10.3).

Tests register/update/collect/list subprocess agents in state.json,
and the CLI subprocess spawn/collect lifecycle.
"""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "tools"))

from lib import state  # noqa: E402


class TestSubprocessAgentState(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"
        st = {"version": "1", "phase": "initialized", "projectKey": "test"}
        self.state_path.write_text(json.dumps(st), encoding="utf-8")

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_register_creates_agent(self):
        st = state.read_state(self.state_path)
        record = state.register_subprocess_agent(
            st, series_type="story", entity_id="STORY-001-BE",
        )
        self.assertTrue(record["agentId"].startswith("spa-"))
        self.assertEqual(record["seriesType"], "story")
        self.assertEqual(record["entityId"], "STORY-001-BE")
        self.assertEqual(record["status"], "running")
        self.assertEqual(record["memoryEntityType"], "story")
        self.assertIn("STORY-001-BE", record["memoryPath"])
        self.assertIn(record, st["subprocessAgents"])

    def test_register_invalid_series_raises(self):
        st = state.read_state(self.state_path)
        with self.assertRaises(ValueError):
            state.register_subprocess_agent(st, series_type="invalid", entity_id="X")

    def test_update_agent(self):
        st = state.read_state(self.state_path)
        record = state.register_subprocess_agent(st, series_type="coding", entity_id="STORY-001-BE")
        updated = state.update_subprocess_agent(st, record["agentId"], status="failed")
        self.assertEqual(updated["status"], "failed")

    def test_update_nonexistent_raises(self):
        st = state.read_state(self.state_path)
        with self.assertRaises(KeyError):
            state.update_subprocess_agent(st, "spa-nonexist", status="failed")

    def test_collect_marks_completed(self):
        st = state.read_state(self.state_path)
        record = state.register_subprocess_agent(st, series_type="dr", entity_id="DR-001")
        result = state.collect_subprocess_agent(
            st, record["agentId"],
            deliverables=[{"name": "DR文档", "path": "doc.md"}],
        )
        self.assertEqual(result["status"], "completed")
        self.assertEqual(len(result["deliverables"]), 1)
        self.assertIn("completedAt", result)

    def test_list_filters_by_status(self):
        st = state.read_state(self.state_path)
        state.register_subprocess_agent(st, series_type="story", entity_id="S1")
        r2 = state.register_subprocess_agent(st, series_type="coding", entity_id="S2")
        state.collect_subprocess_agent(st, r2["agentId"])
        running = state.list_subprocess_agents(st, status="running")
        completed = state.list_subprocess_agents(st, status="completed")
        self.assertEqual(len(running), 1)
        self.assertEqual(len(completed), 1)

    def test_get_active_agent(self):
        st = state.read_state(self.state_path)
        self.assertIsNone(state.get_active_subprocess_agent(st))
        state.register_subprocess_agent(st, series_type="story", entity_id="S1")
        active = state.get_active_subprocess_agent(st)
        self.assertIsNotNone(active)
        self.assertEqual(active["seriesType"], "story")


class TestCompactTrigger(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_read_compact_trigger_none_when_absent(self):
        self.assertIsNone(state.read_compact_trigger(self.tmp))

    def test_read_compact_trigger_returns_content(self):
        (self.tmp / ".ae-sdd").mkdir()
        trigger = {"prdId": "PRD-001", "summaryPath": "summary.md", "triggeredAt": "2026-07-13T10:00:00Z"}
        (self.tmp / ".ae-sdd" / "compact-trigger").write_text(
            json.dumps(trigger), encoding="utf-8"
        )
        result = state.read_compact_trigger(self.tmp)
        self.assertEqual(result["prdId"], "PRD-001")

    def test_clear_compact_trigger(self):
        (self.tmp / ".ae-sdd").mkdir()
        (self.tmp / ".ae-sdd" / "compact-trigger").write_text("{}", encoding="utf-8")
        self.assertTrue(state.clear_compact_trigger(self.tmp))
        self.assertFalse((self.tmp / ".ae-sdd" / "compact-trigger").is_file())

    def test_clear_nonexistent_returns_false(self):
        self.assertFalse(state.clear_compact_trigger(self.tmp))


if __name__ == "__main__":
    unittest.main(verbosity=2)
