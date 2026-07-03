"""
test_standalone_skill_runtime_compiler.py - standalone compiler skill tests.
"""
from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "standalone-skills" / "skill-runtime-compiler" / "scripts" / "compile_skill_package.py"


def _load_compiler_module():
    spec = importlib.util.spec_from_file_location("standalone_skill_runtime_compiler", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load compiler script: {SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


compiler = _load_compiler_module()


class TestStandaloneSkillRuntimeCompiler(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="standalone-skill-compiler-"))
        self.source = self.tmp / "example-skill"
        self.source.mkdir()
        (self.source / "SKILL.md").write_text(
            "---\n"
            "name: example-skill\n"
            "description: Example source skill for compiler tests.\n"
            "version: 1.2.3\n"
            "---\n\n"
            "# Example Skill\n\n"
            "Use this skill for deterministic compiler tests.\n\n"
            "## Workflow\n\n"
            "Run the tool.\n\n"
            "## Verification\n\n"
            "Check the manifest.\n",
            encoding="utf-8",
        )
        (self.source / "references").mkdir()
        (self.source / "references" / "guide.md").write_text("# Guide\n", encoding="utf-8")
        (self.source / "scripts").mkdir()
        (self.source / "scripts" / "helper.py").write_text("print('ok')\n", encoding="utf-8")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _snapshot(self, package: Path) -> dict[str, bytes]:
        paths = [package / "SKILL.md"]
        paths.extend(path for path in (package / "runtime").rglob("*") if path.is_file())
        return {
            path.relative_to(package).as_posix(): path.read_bytes()
            for path in sorted(paths)
        }

    def test_compile_creates_sibling_runtime_package_without_mutating_source(self):
        manifest = compiler.compile_skill_package(self.source)
        package = self.tmp / "example-skill-compiled"

        self.assertTrue(package.is_dir())
        self.assertEqual(Path(manifest["package_path"]), package)
        self.assertEqual((self.source / "SKILL.md").read_text(encoding="utf-8").count("compiled: true"), 0)
        self.assertIn("compiled: true", (package / "SKILL.md").read_text(encoding="utf-8"))
        self.assertEqual(
            (package / "runtime" / "fallback" / "SKILL.full.md").read_text(encoding="utf-8"),
            (self.source / "SKILL.md").read_text(encoding="utf-8"),
        )
        self.assertTrue((package / "references" / "guide.md").is_file())
        self.assertTrue((package / "scripts" / "helper.py").is_file())

        parsed = json.loads((package / "runtime" / "manifest.json").read_text(encoding="utf-8"))
        self.assertTrue(parsed["compiled"])
        self.assertTrue(parsed["deterministic"])
        self.assertEqual(parsed["schema"], compiler.MANIFEST_SCHEMA)
        self.assertEqual(parsed["source"]["package_name"], "example-skill")
        self.assertEqual(parsed["extracts"]["heading_count"], 3)
        self.assertEqual(parsed["extracts"]["script_count"], 1)
        self.assertEqual(parsed["extracts"]["reference_count"], 1)
        self.assertEqual(len(parsed["runtime_fingerprint"]), 64)

    def test_compile_is_byte_idempotent(self):
        compiler.compile_skill_package(self.source)
        package = self.tmp / "example-skill-compiled"
        first = self._snapshot(package)

        compiler.compile_skill_package(self.source)
        second = self._snapshot(package)

        self.assertEqual(first, second)

    def test_refuses_unrelated_existing_output_without_force(self):
        output = self.tmp / "custom-output"
        output.mkdir()
        (output / "note.txt").write_text("do not remove\n", encoding="utf-8")

        with self.assertRaises(compiler.CompileError):
            compiler.compile_skill_package(self.source, output=output)
        self.assertTrue((output / "note.txt").is_file())

        compiler.compile_skill_package(self.source, output=output, force=True)
        self.assertTrue((output / "runtime" / "manifest.json").is_file())

    def test_cli_json_output(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), str(self.source), "--json"],
            cwd=REPO_ROOT,
            text=True,
            encoding="utf-8",
            capture_output=True,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        parsed = json.loads(result.stdout)
        self.assertTrue(parsed["compiled"])
        self.assertEqual(parsed["source"]["package_name"], "example-skill")

    def test_frontmatter_parser_handles_crlf_and_block_scalars(self):
        metadata = compiler.parse_simple_frontmatter(
            "---\r\n"
            "name: block-skill\r\n"
            "description: |-\r\n"
            "  Line one.\r\n"
            "  Line two.\r\n"
            "---\r\n"
            "# Body\r\n"
        )

        self.assertEqual(metadata["name"], "block-skill")
        self.assertEqual(metadata["description"], "Line one.\nLine two.")

    def test_batch_compile_skips_references_and_compiled_sources_by_default(self):
        root = self.tmp / "repo"
        owned = root / "plugins" / "owned-skill"
        reference = root / "references" / "external-skill"
        compiled_source = root / "standalone-skills" / "compiled-source"
        for path in (owned, reference, compiled_source):
            path.mkdir(parents=True)
        (owned / "SKILL.md").write_text(
            "---\nname: owned-skill\n---\n\n# Owned\n",
            encoding="utf-8",
        )
        (reference / "SKILL.md").write_text(
            "---\nname: external-skill\n---\n\n# External\n",
            encoding="utf-8",
        )
        (compiled_source / "SKILL.md").write_text(
            "---\nname: compiled-source\ncompiled: true\nruntime: runtime/manifest.json\n---\n\n# Compiled\n",
            encoding="utf-8",
        )

        summary = compiler.compile_skill_packages_under(root)

        self.assertEqual(summary["counts"]["compiled"], 1)
        self.assertEqual(summary["counts"]["failed"], 0)
        self.assertEqual({record["source"] for record in summary["records"]}, {"plugins/owned-skill"})
        self.assertIn(
            {"source": "standalone-skills/compiled-source", "reason": "already-compiled-source"},
            summary["skipped"],
        )
        self.assertFalse(any(item["source"] == "references/external-skill" for item in summary["records"]))
        self.assertTrue((root / summary["records"][0]["output"] / "runtime" / "manifest.json").is_file())

        second = compiler.compile_skill_packages_under(root)
        self.assertEqual(second["counts"]["compiled"], 0)
        self.assertEqual(second["counts"]["up_to_date"], 1)

    def test_batch_compile_can_include_references(self):
        root = self.tmp / "repo-with-refs"
        owned = root / "plugins" / "owned-skill"
        reference = root / "references" / "external-skill"
        for path in (owned, reference):
            path.mkdir(parents=True)
            (path / "SKILL.md").write_text(
                f"---\nname: {path.name}\n---\n\n# {path.name}\n",
                encoding="utf-8",
            )

        summary = compiler.compile_skill_packages_under(root, include_references=True)

        self.assertEqual(summary["counts"]["compiled"], 2)
        self.assertEqual(
            {record["source"] for record in summary["records"]},
            {"plugins/owned-skill", "references/external-skill"},
        )

    def test_compile_uses_source_fallback_for_slimmed_package(self):
        fallback = self.source / "source-fallback" / "SKILL.full.md"
        fallback.parent.mkdir()
        full_text = (
            "---\nname: example-skill\nversion: 1.2.3\n---\n\n"
            "# Full Example Skill\n\n"
            "Use the full source as runtime fallback.\n\n"
            "## Full Detail\n"
        )
        fallback.write_text(full_text, encoding="utf-8")
        (self.source / "SKILL.md").write_text(
            "---\n"
            "name: example-skill\n"
            "version: 1.2.3\n"
            "source_slimmed: true\n"
            "source_fallback: source-fallback/SKILL.full.md\n"
            "---\n\n"
            "# Slim Example Skill\n",
            encoding="utf-8",
        )

        compiler.compile_skill_package(self.source)
        package = self.tmp / "example-skill-compiled"

        self.assertEqual(
            (package / "runtime" / "fallback" / "SKILL.full.md").read_text(encoding="utf-8"),
            full_text,
        )
        outline = (package / "runtime" / "outline.compact.md").read_text(encoding="utf-8")
        self.assertIn("Full Detail", outline)


if __name__ == "__main__":
    unittest.main(verbosity=2)
