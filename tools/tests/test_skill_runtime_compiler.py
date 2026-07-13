"""
test_skill_runtime_compiler.py - runtime compiler unit tests.
"""
from __future__ import annotations

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
sys.path.insert(0, str(REPO_ROOT / "tools"))

from compile_skill_runtime import compile_runtime_package  # noqa: E402
from lib.gates import GATE_REGISTRY  # noqa: E402
from lib.state import PHASE_FLOWS  # noqa: E402


class TestSkillRuntimeCompiler(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="skill-runtime-"))
        self.source = self.tmp / "source"
        self.dist = self.tmp / "dist" / "ae-sdd"
        self.source.mkdir(parents=True)
        self.dist.mkdir(parents=True)
        (self.source / "SKILL.md").write_text(
            "---\nname: ae-sdd\nversion: 9.9.9\n---\n\n# Source Skill\n",
            encoding="utf-8",
        )
        (self.source / "skills").mkdir()
        (self.source / "skills" / "child-skill.md").write_text("# Child\n", encoding="utf-8")
        (self.dist / "SKILL.md").write_text("# Dist Skill Full\n", encoding="utf-8")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _runtime_snapshot(self) -> dict[str, bytes]:
        paths = [self.dist / "SKILL.md"]
        runtime_dir = self.dist / "runtime"
        paths.extend(path for path in runtime_dir.rglob("*") if path.is_file())
        skills_dir = self.dist / "skills"
        if skills_dir.is_dir():
            paths.extend(path for path in skills_dir.rglob("*.md") if path.is_file())
        return {
            path.relative_to(self.dist).as_posix(): path.read_bytes()
            for path in sorted(paths)
        }

    def test_compile_writes_bootloader_manifest_and_fallback(self):
        manifest = compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )

        bootloader = (self.dist / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("compiled: true", bootloader)
        self.assertIn("runtime/manifest.json", bootloader)
        self.assertIn("version: 9.9.9", bootloader)

        manifest_path = self.dist / "runtime" / "manifest.json"
        self.assertTrue(manifest_path.is_file())
        parsed = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertTrue(parsed["compiled"])
        self.assertEqual(parsed["version"], "9.9.9")
        self.assertTrue(parsed["deterministic"])
        self.assertNotIn("compiled_at", parsed)
        self.assertEqual(parsed["compiler"]["version"], "2")
        self.assertEqual(len(parsed["runtime_fingerprint"]), 64)
        self.assertEqual(len(parsed["source"]["fallback_sha256"]), 64)
        self.assertEqual(parsed["source"]["file_count"], 2)
        self.assertEqual(parsed["extracts"]["gate_count"], len(GATE_REGISTRY))
        self.assertEqual(set(parsed["extracts"]["flow_scales"]), set(PHASE_FLOWS.keys()))
        self.assertEqual(parsed["extracts"]["subskill_count"], 1)
        self.assertEqual(len(parsed["subskills"]), 1)
        self.assertEqual(manifest["version"], parsed["version"])

        fallback = self.dist / "runtime" / "fallback" / "SKILL.full.md"
        self.assertTrue(fallback.is_file())
        self.assertEqual(fallback.read_text(encoding="utf-8"), "# Dist Skill Full\n")

        child_entry = self.dist / "skills" / "child-skill.md"
        child_fallback = self.dist / "runtime" / "skills" / "child-skill" / "fallback" / "SKILL.full.md"
        child_manifest = self.dist / "runtime" / "skills" / "child-skill" / "manifest.json"
        child_outline = self.dist / "runtime" / "skills" / "child-skill" / "outline.compact.md"
        child_core = self.dist / "runtime" / "skills" / "child-skill" / "core.compact.md"
        self.assertTrue(child_entry.is_file())
        self.assertTrue(child_fallback.is_file())
        self.assertTrue(child_manifest.is_file())
        self.assertTrue(child_outline.is_file())
        self.assertTrue(child_core.is_file(), "core.compact.md must be generated for each subskill")
        self.assertIn("compiled: true", child_entry.read_text(encoding="utf-8"))
        self.assertIn("Compiled Sub-SKILL Entry", child_entry.read_text(encoding="utf-8"))
        self.assertEqual(child_fallback.read_text(encoding="utf-8"), "# Child\n")

        # core.compact.md must have the executable-core header + fallback guard.
        core_text = child_core.read_text(encoding="utf-8")
        self.assertIn("Executable Core Compact", core_text)
        self.assertIn("Fallback Guard", core_text)

        # child manifest load_order must include core.compact.md between boot and outline.
        child_manifest_parsed = json.loads(child_manifest.read_text(encoding="utf-8"))
        self.assertIn("runtime/skills/child-skill/core.compact.md", child_manifest_parsed["load_order"])
        boot_idx = child_manifest_parsed["load_order"].index("runtime/skills/child-skill/boot.compact.md")
        core_idx = child_manifest_parsed["load_order"].index("runtime/skills/child-skill/core.compact.md")
        outline_idx = child_manifest_parsed["load_order"].index("runtime/skills/child-skill/outline.compact.md")
        self.assertLess(boot_idx, core_idx)
        self.assertLess(core_idx, outline_idx)

    def test_compile_is_byte_idempotent(self):
        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )
        first = self._runtime_snapshot()

        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2030-01-01T00:00:00Z",
        )
        second = self._runtime_snapshot()

        self.assertEqual(first, second)
        self.assertEqual(
            (self.dist / "runtime" / "fallback" / "SKILL.full.md").read_text(encoding="utf-8"),
            "# Dist Skill Full\n",
        )

    def test_load_order_files_exist(self):
        manifest = compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )
        for rel in manifest["load_order"]:
            self.assertTrue((self.dist / rel).is_file(), rel)

    def test_gates_and_flow_compact_include_extracted_data(self):
        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )

        gates = (self.dist / "runtime" / "gates.compact.md").read_text(encoding="utf-8")
        self.assertIn(f"gate_count: {len(GATE_REGISTRY)}", gates)
        self.assertIn("G-DOC-STORAGE", gates)
        self.assertIn("tools/lib/gates.py:GATE_REGISTRY", gates)

        flow = (self.dist / "runtime" / "flow.compact.md").read_text(encoding="utf-8")
        for scale in PHASE_FLOWS.keys():
            self.assertIn(f"| {scale} |", flow)
        self.assertIn("tools/lib/state.py:PHASE_FLOWS", flow)

    def test_compile_uses_source_fallback_for_slimmed_source_skills(self):
        root_fallback = self.source / "skill-fallbacks" / "SKILL.full.md"
        root_fallback.parent.mkdir(parents=True)
        root_full = "---\nname: ae-sdd\nversion: 9.9.9\n---\n\n# Full Root\n\n## Original Root Detail\n"
        root_fallback.write_text(root_full, encoding="utf-8")
        (self.source / "SKILL.md").write_text(
            "---\n"
            "name: ae-sdd\n"
            "version: 9.9.9\n"
            "source_slimmed: true\n"
            "source_fallback: skill-fallbacks/SKILL.full.md\n"
            "---\n\n"
            "# Slim Root\n",
            encoding="utf-8",
        )

        child_fallback = self.source / "skill-fallbacks" / "skills" / "child-skill.full.md"
        child_fallback.parent.mkdir(parents=True)
        child_full = "---\nname: child\n---\n\n# Full Child\n\n## Original Child Detail\n"
        child_fallback.write_text(child_full, encoding="utf-8")
        (self.source / "skills" / "child-skill.md").write_text(
            "---\n"
            "name: child\n"
            "source_slimmed: true\n"
            "source_fallback: skill-fallbacks/skills/child-skill.full.md\n"
            "---\n\n"
            "# Slim Child\n",
            encoding="utf-8",
        )

        manifest = compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )

        self.assertTrue(manifest["source"]["source_slimmed"])
        self.assertEqual(
            (self.dist / "runtime" / "fallback" / "SKILL.full.md").read_text(encoding="utf-8"),
            root_full,
        )
        child_runtime_fallback = self.dist / "runtime" / "skills" / "child-skill" / "fallback" / "SKILL.full.md"
        self.assertEqual(child_runtime_fallback.read_text(encoding="utf-8"), child_full)
        child_outline = (self.dist / "runtime" / "skills" / "child-skill" / "outline.compact.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("Original Child Detail", child_outline)


    def test_gates_compact_uses_hint_from_registry(self):
        """🆕 v3.10.3: gates.compact.md must read hint from GATE_REGISTRY, not GATE_HINTS."""
        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )
        gates_text = (self.dist / "runtime" / "gates.compact.md").read_text(encoding="utf-8")
        # Every gate in the real registry must appear with its hint scope (no "see CLI" fallback
        # for gates that now carry a hint field).
        for gate in GATE_REGISTRY:
            self.assertIn(gate["id"], gates_text, f"gate {gate['id']} missing from gates.compact.md")
            hint = gate.get("hint") or {}
            if hint.get("scope"):
                self.assertIn(hint["scope"], gates_text, f"hint scope for {gate['id']} missing")

    def test_manifest_index_includes_core_path(self):
        """🆕 v3.10.3: manifest-index.json subskills must include core path."""
        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )
        index = json.loads((self.dist / "runtime" / "manifest-index.json").read_text(encoding="utf-8"))
        self.assertEqual(len(index["subskills"]), 1)
        sub = index["subskills"][0]
        self.assertIn("core", sub)
        self.assertEqual(sub["core"], "runtime/skills/child-skill/core.compact.md")

    def test_core_compact_extracts_structural_lines(self):
        """🆕 v3.10.3: core.compact.md must keep structural lines (headings, lists, commands) and drop prose."""
        # Build a skill with mixed structural + prose content.
        rich_skill = self.source / "skills" / "rich-skill.md"
        rich_skill.write_text(
            "---\nname: rich\n---\n\n"
            "# Rich Skill\n\n"
            "This is a long prose paragraph that explains the background and motivation "
            "of this skill in great detail. It should be dropped from the core because "
            "it is pure prose without any structural or executable value.\n\n"
            "## 步骤\n\n"
            "1. 第一步：读取输入\n"
            "2. 第二步：生成内容\n"
            "3. 第三步：自检\n\n"
            "## 门禁\n\n"
            "- 🔴 禁止跳过步骤\n"
            "- ae-sdd gates check --only G-XX\n"
            "- BLOCK if gate fails\n",
            encoding="utf-8",
        )
        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )
        core = (self.dist / "runtime" / "skills" / "rich-skill" / "core.compact.md").read_text(encoding="utf-8")
        # Structural lines must survive.
        self.assertIn("Rich Skill", core)
        self.assertIn("步骤", core)
        self.assertIn("第一步", core)
        self.assertIn("禁止跳过步骤", core)
        self.assertIn("ae-sdd gates check", core)
        # Pure prose paragraph must be dropped.
        self.assertNotIn("long prose paragraph that explains the background", core)

    def test_boot_compact_load_order_includes_manifest_index(self):
        """🆕 v3.10.3: boot.compact.md Load Order must mention manifest-index.json (sync with bootloader)."""
        compile_runtime_package(
            REPO_ROOT,
            self.source,
            self.dist,
            build_date="2026-07-02T00:00:00Z",
        )
        boot = (self.dist / "runtime" / "boot.compact.md").read_text(encoding="utf-8")
        self.assertIn("manifest-index.json", boot)


if __name__ == "__main__":
    unittest.main(verbosity=2)
