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
        self.assertEqual(parsed["compiler"]["version"], "1")
        self.assertEqual(len(parsed["runtime_fingerprint"]), 64)
        self.assertEqual(len(parsed["source"]["fallback_sha256"]), 64)
        self.assertEqual(parsed["source"]["file_count"], 2)
        self.assertEqual(parsed["extracts"]["gate_count"], len(GATE_REGISTRY))
        self.assertEqual(set(parsed["extracts"]["flow_scales"]), set(PHASE_FLOWS.keys()))
        self.assertEqual(manifest["version"], parsed["version"])

        fallback = self.dist / "runtime" / "fallback" / "SKILL.full.md"
        self.assertTrue(fallback.is_file())
        self.assertEqual(fallback.read_text(encoding="utf-8"), "# Dist Skill Full\n")

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


if __name__ == "__main__":
    unittest.main(verbosity=2)
