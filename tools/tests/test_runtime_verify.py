from __future__ import annotations

import json
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib.runtime_verify import verify_runtime_package  # noqa: E402


FP = "a" * 64


def _write_package(root: Path) -> Path:
    package = root / "ae-sdd"
    runtime = package / "runtime"
    fallback = runtime / "fallback"
    fallback.mkdir(parents=True)
    (runtime / "boot.compact.md").write_text("# boot\n", encoding="utf-8")
    (runtime / "route.compact.md").write_text("# route\n", encoding="utf-8")
    (fallback / "SKILL.full.md").write_text(
        "# source skill\n\n" + ("This is preserved fallback content.\n" * 8),
        encoding="utf-8",
    )
    manifest = {
        "schema": "ae-sdd-runtime/v1",
        "compiled": True,
        "deterministic": True,
        "version": "1.2.3",
        "compiler": {"name": "compile_skill_runtime.py", "version": "1"},
        "runtime_fingerprint": FP,
        "entry": "SKILL.md",
        "load_order": ["runtime/boot.compact.md", "runtime/route.compact.md"],
        "generated_files": [
            "runtime/manifest.json",
            "runtime/boot.compact.md",
            "runtime/route.compact.md",
            "runtime/fallback/SKILL.full.md",
        ],
        "source": {
            "skill_sha256": "b" * 64,
            "fallback_sha256": "c" * 64,
            "checksums": {"SKILL.md": "d" * 64},
        },
        "extracts": {
            "gate_count": 1,
            "flow_scales": ["\u5927", "\u4e2d", "\u5c0f", "\u5fae"],
        },
    }
    (runtime / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
    (package / "SKILL.md").write_text(
        "---\n"
        "name: ae-sdd\n"
        "version: 1.2.3\n"
        "compiled: true\n"
        "runtime: runtime/manifest.json\n"
        "---\n\n"
        "# ae-sdd Compiled Runtime Entry\n\n"
        f"runtime_fingerprint: {FP}\n",
        encoding="utf-8",
    )
    return package


def _add_compiled_child(package: Path, manifest: dict) -> None:
    child_entry = "skills/phase/child-skill.md"
    child_base = "runtime/skills/phase/child-skill"
    child_fp = "e" * 64
    child_source = "# Child source\n\nFull fallback content.\n"

    (package / "runtime" / "subskills.compact.md").write_text("# subskills\n", encoding="utf-8")
    (package / child_entry).parent.mkdir(parents=True, exist_ok=True)
    (package / child_entry).write_text(
        "---\n"
        "name: child\n"
        "description: child compiled entry\n"
        "compiled: true\n"
        f"runtime: {child_base}/manifest.json\n"
        "---\n\n"
        "# child Compiled Sub-SKILL Entry\n\n"
        f"runtime_fingerprint: {child_fp}\n",
        encoding="utf-8",
    )
    (package / child_base / "fallback").mkdir(parents=True, exist_ok=True)
    (package / child_base / "boot.compact.md").write_text("# boot child\n", encoding="utf-8")
    (package / child_base / "outline.compact.md").write_text("# outline child\n", encoding="utf-8")
    (package / child_base / "fallback" / "SKILL.full.md").write_text(child_source, encoding="utf-8")
    child_manifest = {
        "schema": "ae-sdd-subskill-runtime/v1",
        "compiled": True,
        "deterministic": True,
        "compiler": {"name": "compile_skill_runtime.py", "version": "2"},
        "entry": child_entry,
        "runtime_fingerprint": child_fp,
        "load_order": [
            f"{child_base}/boot.compact.md",
            f"{child_base}/outline.compact.md",
        ],
    }
    (package / child_base / "manifest.json").write_text(json.dumps(child_manifest), encoding="utf-8")

    manifest["load_order"].append("runtime/subskills.compact.md")
    manifest["generated_files"].extend([
        "runtime/subskills.compact.md",
        child_entry,
        f"{child_base}/manifest.json",
        f"{child_base}/boot.compact.md",
        f"{child_base}/outline.compact.md",
        f"{child_base}/fallback/SKILL.full.md",
    ])
    manifest["source"]["checksums"][child_entry] = "f" * 64
    manifest["subskills"] = [{
        "entry": child_entry,
        "source_path": f"source/{child_entry}",
        "manifest": f"{child_base}/manifest.json",
        "boot": f"{child_base}/boot.compact.md",
        "outline": f"{child_base}/outline.compact.md",
        "fallback": f"{child_base}/fallback/SKILL.full.md",
        "source_sha256": "f" * 64,
        "fallback_sha256": hashlib.sha256(child_source.encode("utf-8")).hexdigest(),
        "runtime_fingerprint": child_fp,
        "heading_count": 1,
        "ref_count": 0,
    }]
    manifest["extracts"]["subskill_count"] = 1
    (package / "runtime" / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")


def _read_manifest(package: Path) -> dict:
    return json.loads((package / "runtime" / "manifest.json").read_text(encoding="utf-8"))


def _write_manifest(package: Path, manifest: dict) -> None:
    (package / "runtime" / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")


class TestRuntimeVerify(unittest.TestCase):
    def test_valid_compiled_package_passes(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            result = verify_runtime_package(package)
            self.assertTrue(result.ok, result.issues)

    def test_missing_compiled_frontmatter_fails(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            (package / "SKILL.md").write_text(
                "---\nname: ae-sdd\nversion: 1.2.3\n---\n",
                encoding="utf-8",
            )
            result = verify_runtime_package(package)
            self.assertFalse(result.ok)
            self.assertTrue(any("compiled: true" in item for item in result.issues))

    def test_missing_load_order_file_fails(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            (package / "runtime" / "route.compact.md").unlink()
            result = verify_runtime_package(package)
            self.assertFalse(result.ok)
            self.assertTrue(any("load_order file missing" in item for item in result.issues))

    def test_fallback_bootloader_pollution_fails(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            (package / "runtime" / "fallback" / "SKILL.full.md").write_text(
                "---\ncompiled: true\n---\n\n# ae-sdd Compiled Runtime Entry\n",
                encoding="utf-8",
            )
            result = verify_runtime_package(package)
            self.assertFalse(result.ok)
            self.assertTrue(any("generated bootloader" in item for item in result.issues))

    def test_compiled_child_skill_passes(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            manifest = _read_manifest(package)
            _add_compiled_child(package, manifest)
            result = verify_runtime_package(package)
            self.assertTrue(result.ok, result.issues)

    def test_child_skill_source_without_compiled_record_fails(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            manifest = _read_manifest(package)
            manifest["source"]["checksums"]["skills/phase/child-skill.md"] = "f" * 64
            _write_manifest(package, manifest)
            result = verify_runtime_package(package)
            self.assertFalse(result.ok)
            self.assertTrue(any("manifest.subskills" in item for item in result.issues))

    def test_uncompiled_child_skill_entry_fails(self):
        with tempfile.TemporaryDirectory() as td:
            package = _write_package(Path(td))
            manifest = _read_manifest(package)
            _add_compiled_child(package, manifest)
            (package / "skills" / "phase" / "child-skill.md").write_text(
                "# Child source\n\nThis is an uncompiled child skill.\n",
                encoding="utf-8",
            )
            result = verify_runtime_package(package)
            self.assertFalse(result.ok)
            self.assertTrue(any("compiled: true" in item or "uncompiled source" in item for item in result.issues))


if __name__ == "__main__":
    unittest.main(verbosity=2)
