"""
test_compact_reload.py - compact trigger + memory reload tests (🆕 v3.10.3).

Tests the compact-trigger read end + pre/post compact snapshot lifecycle:
  pre_compact_snapshot -> compact -> post_compact_reload
"""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "tools"))

from lib import memory_store, state  # noqa: E402


class TestCompactReload(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_pre_compact_snapshot_writes_progress(self):
        scope = memory_store.locate_scope(
            project=str(self.tmp), entity_type="coding", entity_id="STORY-001-BE",
        )
        memory_store.create_memory(scope, source_contexts={}, current_series="coding", next_step="write code")
        result = memory_store.pre_compact_snapshot(
            scope,
            current_series="test-running",
            next_step="run tests",
            pending_items=[{"id": "D-001", "description": "fix NPE", "owner": "root", "status": "open"}],
            review_loop_status="round 2, 1 finding",
        )
        self.assertTrue(result["snapshotted"])
        # Verify snapshot written to memory
        mem = memory_store.read_memory(scope)
        self.assertIn("test-running", mem["boot"])
        self.assertIn("run tests", mem["boot"])
        self.assertIn("D-001", mem["pending"])
        self.assertIn("fix NPE", mem["pending"])

    def test_post_compact_reload_returns_full_memory(self):
        scope = memory_store.locate_scope(
            project=str(self.tmp), entity_type="story", entity_id="STORY-002-BE",
        )
        memory_store.create_memory(
            scope,
            source_contexts={},
            current_series="story-generate",
            next_step="generate",
            constraints=["BigDecimal"],
            story_acs=[{"id": "AC-1", "description": "login", "status": "done"}],
        )
        memory_store.pre_compact_snapshot(
            scope,
            current_series="story-review",
            next_step="review",
            pending_items=[{"id": "D-002", "description": "AC-2 vague", "owner": "root", "status": "open"}],
            review_loop_status="round 1",
        )
        reloaded = memory_store.post_compact_reload(scope)
        self.assertIn("story-review", reloaded["boot"])
        self.assertIn("BigDecimal", reloaded["context"])
        self.assertIn("D-002", reloaded["pending"])

    def test_prompt_inject_compact_trigger_reload(self):
        """compact-trigger 存在时 prompt_inject 注入重载提示。"""
        from lib import prompt_inject
        ade_sdd = self.tmp / ".ae-sdd"
        ade_sdd.mkdir()
        # Write compact-trigger
        trigger = {"prdId": "PRD-001", "summaryPath": "summary.md", "triggeredAt": "2026-07-13T10:00:00Z"}
        (ade_sdd / "compact-trigger").write_text(json.dumps(trigger), encoding="utf-8")
        # Create memory for coding entity
        scope = memory_store.locate_scope(
            project=str(self.tmp), entity_type="coding", entity_id="STORY-001-BE",
        )
        memory_store.create_memory(scope, source_contexts={}, current_series="coding", next_step="code")
        # _check_compact_trigger should read trigger + reload memory + clear trigger
        result = prompt_inject._check_compact_trigger(ade_sdd, "coding", "STORY-001-BE")
        self.assertIsNotNone(result)
        self.assertIn("COMPACT RELOAD", result)
        self.assertIn("PRD-001", result)
        self.assertIn("MEMORY RELOADED", result)
        # Trigger should be cleared after read
        self.assertFalse((ade_sdd / "compact-trigger").is_file())

    def test_prompt_inject_no_trigger_returns_none(self):
        """无 compact-trigger 时返回 None。"""
        from lib import prompt_inject
        ade_sdd = self.tmp / ".ae-sdd"
        ade_sdd.mkdir()
        result = prompt_inject._check_compact_trigger(ade_sdd, "coding", "STORY-001-BE")
        self.assertIsNone(result)

    def test_compact_trigger_without_memory_still_reloads(self):
        """compact-trigger 存在但 memory 不存在时，仍返回重载提示（主流程走 state.json）。"""
        from lib import prompt_inject
        ade_sdd = self.tmp / ".ae-sdd"
        ade_sdd.mkdir()
        trigger = {"prdId": "PRD-001", "summaryPath": "summary.md", "triggeredAt": "2026-07-13T10:00:00Z"}
        (ade_sdd / "compact-trigger").write_text(json.dumps(trigger), encoding="utf-8")
        result = prompt_inject._check_compact_trigger(ade_sdd, "coding", "STORY-999-BE")
        self.assertIsNotNone(result)
        self.assertIn("COMPACT RELOAD", result)
        self.assertIn("no subprocess memory to reload", result)
        # Trigger still cleared
        self.assertFalse((ade_sdd / "compact-trigger").is_file())


if __name__ == "__main__":
    unittest.main(verbosity=2)
