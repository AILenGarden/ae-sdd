import json
import sqlite3
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import db_tool, git_insight, memory_store  # noqa: E402


class TestMemoryStore(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_exit_blocks_without_write_after_enter(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        memory_store.enter(scope, actor="test")
        result = memory_store.exit_phase(scope, actor="test")
        self.assertFalse(result["pass"])
        self.assertTrue(result["blocked"])

    def test_exit_blocks_without_enter_even_if_written(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        memory_store.write(scope, summary="RA note without enter", evidence=["ra.md:1"], actor="test")
        result = memory_store.exit_phase(scope, actor="test")
        self.assertFalse(result["pass"])
        self.assertTrue(result["blocked"])
        self.assertIn("enter", result["reason"])

    def test_check_exit_ready_does_not_append_exit_event(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        memory_store.enter(scope, actor="test")
        memory_store.write(scope, summary="RA confirmed user roles", evidence=["ra.md:10"], actor="test")
        before = memory_store.read(scope, include_project=False, limit=100)
        result = memory_store.check_exit_ready(scope)
        after = memory_store.read(scope, include_project=False, limit=100)
        self.assertTrue(result["pass"])
        self.assertEqual(len(before), len(after))
        self.assertFalse(any(e.get("type") == "exit" for e in after))

    def test_exit_passes_after_write(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        memory_store.enter(scope, actor="test")
        memory_store.write(scope, summary="RA confirmed user roles", evidence=["ra.md:10"], actor="test")
        result = memory_store.exit_phase(scope, actor="test")
        self.assertTrue(result["pass"])
        entries = memory_store.read(scope)
        self.assertTrue(any(e.get("summary") == "RA confirmed user roles" for e in entries))

    def test_exit_writes_last_exit_at_to_stage(self):
        """🆕 v3.8.2 exit_phase 成功后 .stage 含 last_exit_at，且 scope 变为非活跃。"""
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        memory_store.enter(scope, actor="test")
        memory_store.write(scope, summary="RA done", evidence=["ra.md:1"], actor="test")
        result = memory_store.exit_phase(scope, actor="test")
        self.assertTrue(result["pass"])
        self.assertIsNotNone(result["stage"].get("last_exit_at"))
        self.assertFalse(memory_store.is_scope_active(scope))

    def test_enter_clears_last_exit_at(self):
        """🆕 v3.8.2 重新 enter 清除 last_exit_at，scope 重新变为活跃。"""
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        memory_store.enter(scope, actor="test")
        memory_store.write(scope, summary="RA done", evidence=["ra.md:1"], actor="test")
        memory_store.exit_phase(scope, actor="test")
        self.assertFalse(memory_store.is_scope_active(scope))
        memory_store.enter(scope, actor="test")
        self.assertTrue(memory_store.is_scope_active(scope))

    def test_promote_l1_to_l2(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="coding-plan", story="STORY-001")
        memory_store.write(
            scope,
            summary="Use existing repository pattern",
            layer="L1",
            kind="decision",
            evidence=["plan.md:12"],
            actor="test",
        )
        result = memory_store.promote(scope, from_layer="L1", to_layer="L2", actor="test")
        self.assertEqual(result["promoted"], 1)

    def test_l1_memory_requires_evidence(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        with self.assertRaisesRegex(ValueError, "requires --evidence"):
            memory_store.write(scope, summary="Decision without evidence", layer="L1", actor="test")

    def test_l0_memory_allows_scratch_without_evidence(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        result = memory_store.write(scope, summary="Scratch note", layer="L0", actor="test")
        self.assertTrue(result["written"])

    def test_memory_summary_length_is_enforced(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="ra", story="STORY-001")
        with self.assertRaisesRegex(ValueError, "summary too long"):
            memory_store.write(
                scope,
                summary="x" * 181,
                layer="L1",
                kind="decision",
                evidence=["ra.md:1"],
                actor="test",
            )

    def test_l2_observation_is_rejected(self):
        scope = memory_store.locate_scope(project=str(self.tmp), phase="coding", story="STORY-001")
        with self.assertRaisesRegex(ValueError, "kind=observation"):
            memory_store.write(
                scope,
                summary="Project reusable fact",
                layer="L2",
                kind="observation",
                evidence=["coding.md:1"],
                actor="test",
            )


class TestDbTool(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.db = self.tmp / "local.db"
        with sqlite3.connect(self.db) as conn:
            conn.execute("create table user(id integer primary key, name text)")
            conn.execute("insert into user(name) values ('alice')")
            conn.commit()
        profile = self.tmp / ".ae-sdd" / "secrets" / "db-connections.local.json"
        profile.parent.mkdir(parents=True, exist_ok=True)
        profile.write_text(json.dumps({
            "profiles": [{"name": "local", "driver": "sqlite", "database": str(self.db)}]
        }), encoding="utf-8")

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_select_query_sqlite(self):
        result = db_tool.query(profile_name="local", sql="select * from user", project=str(self.tmp))
        self.assertTrue(result["ok"])
        self.assertEqual(result["rows"][0]["name"], "alice")

    def test_write_sql_requires_flag(self):
        result = db_tool.query(profile_name="local", sql="update user set name='bob'", project=str(self.tmp))
        self.assertFalse(result["ok"])
        self.assertTrue(result["blocked"])


class TestGitInsight(unittest.TestCase):

    def test_impact_risk_hints(self):
        result = git_insight.impact(
            project=str(Path(__file__).resolve().parent.parent.parent),
            files=["service/src/main/resources/mapper/UserMapper.xml", "api/UserController.java"],
        )
        self.assertIn("service", result["modules"])
        self.assertTrue(any("database" in h for h in result["risk_hints"]))
        self.assertTrue(any("API" in h for h in result["risk_hints"]))


if __name__ == "__main__":
    unittest.main(verbosity=2)
