"""
test_memory_compiler.py - memory compiler unit tests (🆕 v3.10.3).

Tests the compile_source_to_memory pipeline: source contexts -> compact slices -> manifest.
Verifies determinism, compact format, common extraction, and size limits.
"""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "tools"))

from lib import memory_compiler  # noqa: E402


class TestMemoryCompiler(unittest.TestCase):

    def test_compile_produces_four_outputs(self):
        result = memory_compiler.compile_source_to_memory(
            entity_type="story",
            entity_id="STORY-001-BE",
            source_contexts={"constraints": "Use BigDecimal."},
            series_chain=["story-generate"],
            current_series="story-generate",
            next_step="generate",
            constraints=["BigDecimal"],
        )
        self.assertIn("boot.compact.md", result)
        self.assertIn("context.compact.md", result)
        self.assertIn("pending.compact.md", result)
        self.assertIn("manifest.json", result)

    def test_boot_contains_entity_info(self):
        result = memory_compiler.compile_source_to_memory(
            entity_type="story",
            entity_id="STORY-001-BE",
            source_contexts={},
            series_chain=["story-generate", "story-review"],
            current_series="story-generate",
            next_step="generate story doc",
        )
        boot = result["boot.compact.md"]
        self.assertIn("story/STORY-001-BE", boot)
        self.assertIn("story-generate", boot)
        self.assertIn("generate story doc", boot)
        self.assertIn("story-generate -> story-review", boot)

    def test_context_contains_acs_and_constraints(self):
        result = memory_compiler.compile_source_to_memory(
            entity_type="story",
            entity_id="STORY-001-BE",
            source_contexts={},
            story_acs=[{"id": "AC-1", "description": "user login", "status": "pending"}],
            constraints=["BigDecimal", "幂等", "禁大事务"],
        )
        context = result["context.compact.md"]
        self.assertIn("AC-1", context)
        self.assertIn("user login", context)
        self.assertIn("BigDecimal", context)
        self.assertIn("幂等", context)

    def test_pending_contains_items(self):
        result = memory_compiler.compile_source_to_memory(
            entity_type="story",
            entity_id="STORY-001-BE",
            source_contexts={},
            pending_items=[{"id": "D-001", "description": "AC-2 unclear", "owner": "root", "status": "open"}],
            review_loop_status="round 1, 2 findings",
        )
        pending = result["pending.compact.md"]
        self.assertIn("D-001", pending)
        self.assertIn("AC-2 unclear", pending)
        self.assertIn("round 1", pending)

    def test_manifest_has_fingerprint_and_hashes(self):
        result = memory_compiler.compile_source_to_memory(
            entity_type="story",
            entity_id="STORY-001-BE",
            source_contexts={"constraints": "Use BigDecimal."},
        )
        manifest = json.loads(result["manifest.json"])
        self.assertEqual(manifest["schema"], "ae-sdd-memory/v1")
        self.assertEqual(manifest["entity_type"], "story")
        self.assertEqual(manifest["entity_id"], "STORY-001-BE")
        self.assertTrue(manifest["deterministic"])
        self.assertEqual(len(manifest["fingerprint"]), 64)
        self.assertIn("boot", manifest["slices"])
        self.assertIn("context", manifest["slices"])
        self.assertIn("pending", manifest["slices"])
        self.assertIn("constraints", manifest["source_hashes"])

    def test_compile_is_deterministic(self):
        """Same input -> same output (byte-level)."""
        kwargs = dict(
            entity_type="story",
            entity_id="STORY-001-BE",
            source_contexts={"constraints": "Use BigDecimal. 禁止大事务."},
            series_chain=["story-generate"],
            current_series="story-generate",
            next_step="generate",
            constraints=["BigDecimal"],
            story_acs=[{"id": "AC-1", "description": "login", "status": "pending"}],
        )
        first = memory_compiler.compile_source_to_memory(**kwargs)
        second = memory_compiler.compile_source_to_memory(**kwargs)
        for key in first:
            self.assertEqual(first[key], second[key], f"non-deterministic: {key}")

    def test_common_extraction_finds_constraints(self):
        common = memory_compiler.extract_common({
            "constraints": "金额字段必须用 BigDecimal，禁止 Double。禁止大事务。分布式操作必须幂等。",
            "standards": "SQL 必须用参数化查询防注入。循环内禁止调用外部接口。",
        })
        self.assertIn("BigDecimal", common)
        self.assertIn("禁止大事务", common)
        self.assertIn("幂等", common)
        self.assertIn("SQL", common)
        self.assertIn("循环内", common)

    def test_common_extraction_dedupes(self):
        common = memory_compiler.extract_common({
            "a": "禁止大事务。Use BigDecimal.",
            "b": "禁止大事务。Use BigDecimal.",
        })
        # "禁止大事务" should appear only once (deduped)
        self.assertEqual(common.count("禁止大事务"), 1)

    def test_common_extraction_empty_returns_placeholder(self):
        common = memory_compiler.extract_common({"notes": "This is just a note without constraints."})
        self.assertIn("no reusable constraints", common)

    def test_common_size_limit_enforced(self):
        """common must not exceed COMMON_MAX_CHARS."""
        huge_content = "禁止大事务。" * 1000
        common = memory_compiler.extract_common({"constraints": huge_content})
        self.assertLessEqual(len(common), memory_compiler.COMMON_MAX_CHARS + 200)  # +200 for header/warning
        self.assertIn("truncated", common)

    def test_empty_contexts_produce_valid_output(self):
        result = memory_compiler.compile_source_to_memory(
            entity_type="common",
            entity_id="default",
            source_contexts={},
        )
        boot = result["boot.compact.md"]
        context = result["context.compact.md"]
        pending = result["pending.compact.md"]
        self.assertIn("common/default", boot)
        self.assertIn("no context extracted", context)
        self.assertIn("no pending items", pending)


if __name__ == "__main__":
    unittest.main(verbosity=2)
