"""
test_source_skill_slimming.py - source SKILL slimming standard tests.
"""
from __future__ import annotations

import importlib.util
import shutil
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "slim_source_skills.py"


def _load_slimmer_module():
    spec = importlib.util.spec_from_file_location("source_skill_slimmer", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load source slimmer: {SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


slimmer = _load_slimmer_module()


class TestSourceSkillSlimming(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="source-skill-slim-"))
        self.source = self.tmp / "source"
        self.source.mkdir()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_slims_source_with_v2_semantic_inventory_and_validates(self):
        source_text = (
            "---\n"
            "name: sample-skill\n"
            "description: Use when a workflow needs a gate and CLI command.\n"
            "---\n\n"
            "# Sample Skill\n\n"
            "Use this skill when routing a Story workflow.\n\n"
            "## Workflow\n\n"
            "Run `ae-sdd gates check --only G-00` before output.\n\n"
            "## Output Contract\n\n"
            "Save the document through `source/templates/design/story-template.md`.\n"
        )
        (self.source / "SKILL.md").write_text(source_text, encoding="utf-8")

        summary = slimmer.slim_source_skills(self.source)

        self.assertEqual(summary["counts"]["slimmed"], 1)
        self.assertEqual(summary["counts"]["failed"], 0)
        slim_text = (self.source / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("source_slim_schema: ae-sdd-source-slim/v2", slim_text)
        self.assertIn("## Semantic Inventory", slim_text)
        self.assertIn("identity_trigger", slim_text)
        self.assertIn("gate_constraint", slim_text)
        self.assertEqual(
            (self.source / "skill-fallbacks" / "SKILL.full.md").read_text(encoding="utf-8"),
            source_text,
        )

        second = slimmer.slim_source_skills(self.source)
        self.assertEqual(second["counts"]["slimmed"], 0)
        self.assertEqual(second["counts"]["skipped"], 1)
        self.assertEqual(second["counts"]["validated"], 1)
        self.assertEqual(second["counts"]["failed"], 0)

    def test_upgrade_rerenders_from_fallback_not_existing_slim_text(self):
        fallback = self.source / "skill-fallbacks" / "SKILL.full.md"
        fallback.parent.mkdir()
        full_text = "---\nname: upgrade-skill\n---\n\n# Full Skill\n\n## Workflow\n\nRun `ae-sdd state read`.\n"
        fallback.write_text(full_text, encoding="utf-8")
        old_slim = (
            "---\n"
            "name: upgrade-skill\n"
            "source_slimmed: true\n"
            "source_fallback: skill-fallbacks/SKILL.full.md\n"
            f"source_fallback_sha256: {slimmer._sha256_text(full_text)}\n"
            "source_slimmer: slim_source_skills.py@1\n"
            "---\n\n"
            "# Old Slim\n\n"
            "This text must not be used as semantic input.\n"
        )
        (self.source / "SKILL.md").write_text(old_slim, encoding="utf-8")

        summary = slimmer.slim_source_skills(self.source, upgrade=True)

        self.assertEqual(summary["counts"]["upgraded"], 1)
        self.assertEqual(summary["counts"]["failed"], 0)
        upgraded = (self.source / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("source_slim_schema: ae-sdd-source-slim/v2", upgraded)
        self.assertIn("Full Skill", upgraded)
        self.assertNotIn("This text must not be used as semantic input.", upgraded)

    def test_validation_detects_fallback_hash_mismatch(self):
        source_text = "---\nname: broken-skill\n---\n\n# Broken\n\n## Workflow\n\nRun `ae-sdd version`.\n"
        (self.source / "SKILL.md").write_text(source_text, encoding="utf-8")
        summary = slimmer.slim_source_skills(self.source)
        self.assertEqual(summary["counts"]["failed"], 0)

        slim_text = (self.source / "SKILL.md").read_text(encoding="utf-8")
        slim_text = slim_text.replace("source_fallback_sha256: ", "source_fallback_sha256: deadbeef")
        (self.source / "SKILL.md").write_text(slim_text, encoding="utf-8")

        validation = slimmer.slim_source_skills(self.source, validate_only=True)

        self.assertGreaterEqual(validation["counts"]["failed"], 1)
        self.assertTrue(
            any("source_fallback_sha256 mismatch" in failure["error"] for failure in validation["failures"]),
            validation["failures"],
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
