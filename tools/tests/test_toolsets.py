import json
import sqlite3
import tempfile
import unittest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import db_tool, git_insight, memory_store  # noqa: E402


class TestMemoryStore(unittest.TestCase):
    """🆕 v3.10.3: entity-tree memory store tests (compiled compact docs)."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _make_scope(self, entity_type="story", entity_id="STORY-001-BE"):
        return memory_store.locate_scope(
            project=str(self.tmp), entity_type=entity_type, entity_id=entity_id,
        )

    def test_create_writes_compact_slices_and_manifest(self):
        scope = self._make_scope()
        result = memory_store.create_memory(
            scope,
            source_contexts={"constraints": "Use BigDecimal for money. 禁止大事务."},
            series_chain=["story-generate", "story-review"],
            current_series="story-generate",
            next_step="generate story doc",
            constraints=["BigDecimal", "幂等"],
            story_acs=[{"id": "AC-1", "description": "user can login", "status": "pending"}],
        )
        self.assertTrue(result["created"])
        self.assertTrue((scope.entity_dir / "boot.compact.md").is_file())
        self.assertTrue((scope.entity_dir / "context.compact.md").is_file())
        self.assertTrue((scope.entity_dir / "pending.compact.md").is_file())
        self.assertTrue((scope.entity_dir / "manifest.json").is_file())

    def test_read_returns_compact_content(self):
        scope = self._make_scope()
        memory_store.create_memory(
            scope,
            source_contexts={},
            current_series="story-generate",
            next_step="generate",
            constraints=["BigDecimal"],
        )
        mem = memory_store.read_memory(scope)
        self.assertIn("story-generate", mem["boot"])
        self.assertIn("BigDecimal", mem["context"])

    def test_read_nonexistent_returns_empty(self):
        scope = self._make_scope()
        self.assertEqual(memory_store.read_memory(scope), {})

    def test_exists_memory(self):
        scope = self._make_scope()
        self.assertFalse(memory_store.exists_memory(scope))
        memory_store.create_memory(scope, source_contexts={})
        self.assertTrue(memory_store.exists_memory(scope))

    def test_update_memory_slice(self):
        scope = self._make_scope()
        memory_store.create_memory(scope, source_contexts={})
        result = memory_store.update_memory(scope, slice_name="pending", content="# Updated Pending\n")
        self.assertTrue(result["updated"])
        mem = memory_store.read_memory(scope)
        self.assertIn("Updated Pending", mem["pending"])

    def test_update_invalid_slice_raises(self):
        scope = self._make_scope()
        memory_store.create_memory(scope, source_contexts={})
        with self.assertRaises(ValueError):
            memory_store.update_memory(scope, slice_name="invalid", content="x")

    def test_update_without_create_raises(self):
        scope = self._make_scope()
        with self.assertRaises(FileNotFoundError):
            memory_store.update_memory(scope, slice_name="pending", content="x")

    def test_clean_memory_deletes_entity_dir(self):
        scope = self._make_scope()
        memory_store.create_memory(scope, source_contexts={})
        result = memory_store.clean_memory(scope)
        self.assertTrue(result["cleaned"])
        self.assertFalse(scope.entity_dir.exists())

    def test_clean_memory_preserves_common(self):
        scope = self._make_scope()
        memory_store.create_memory(scope, source_contexts={"constraints": "Use BigDecimal."})
        common_scope = memory_store.locate_scope(
            project=str(self.tmp), entity_type="common", entity_id="default",
        )
        self.assertTrue(memory_store.exists_memory(common_scope))
        memory_store.clean_memory(scope)
        # common should still exist after cleaning story
        self.assertTrue(memory_store.exists_memory(common_scope))

    def test_clean_memory_refuses_common(self):
        scope = memory_store.locate_scope(
            project=str(self.tmp), entity_type="common", entity_id="default",
        )
        memory_store.create_memory(scope, source_contexts={})
        result = memory_store.clean_memory(scope)
        self.assertFalse(result["cleaned"])

    def test_clean_all_removes_entities_preserves_common(self):
        story_scope = self._make_scope("story", "STORY-001")
        coding_scope = self._make_scope("coding", "STORY-001")
        memory_store.create_memory(story_scope, source_contexts={"constraints": "BigDecimal."})
        memory_store.create_memory(coding_scope, source_contexts={})
        result = memory_store.clean_all_memory(story_scope)
        self.assertTrue(result["cleaned"])
        self.assertIn("story", result["removed_types"])
        self.assertIn("coding", result["removed_types"])
        self.assertIn("common", result["preserved"])
        # common should survive clean-all
        common = memory_store.read_common(story_scope)
        self.assertTrue(common)

    def test_common_extraction_from_source_contexts(self):
        scope = self._make_scope()
        memory_store.create_memory(
            scope,
            source_contexts={
                "constraints": "金额字段必须用 BigDecimal，禁止 Double。禁止大事务。分布式操作必须幂等。",
            },
        )
        common = memory_store.read_common(scope)
        self.assertIn("BigDecimal", common["context"])
        self.assertIn("禁止大事务", common["context"])

    def test_pre_compact_snapshot_and_post_compact_reload(self):
        scope = self._make_scope()
        memory_store.create_memory(scope, source_contexts={}, current_series="story-generate", next_step="gen")
        memory_store.pre_compact_snapshot(
            scope,
            current_series="story-review",
            next_step="review story",
            pending_items=[{"id": "D-001", "description": "AC-2 needs clarification", "owner": "root", "status": "open"}],
            review_loop_status="round 1, 2 findings",
        )
        mem = memory_store.post_compact_reload(scope)
        self.assertIn("story-review", mem["boot"])
        self.assertIn("D-001", mem["pending"])
        self.assertIn("round 1", mem["pending"])

    def test_legacy_phase_story_args_compat(self):
        """过渡期：旧 --phase/--story 参数兼容，内部转换为 entity_type/entity_id。"""
        scope = memory_store.locate_scope(
            project=str(self.tmp), phase="coding", story="STORY-001-BE",
        )
        self.assertEqual(scope.entity_type, "coding")
        self.assertEqual(scope.entity_id, "STORY-001-BE")

    def test_entity_type_for_state_phase(self):
        self.assertEqual(memory_store.entity_type_for_state_phase("coding"), "coding")
        self.assertEqual(memory_store.entity_type_for_state_phase("ra-generated"), "prd")
        self.assertIsNone(memory_store.entity_type_for_state_phase("initialized"))


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
