from __future__ import annotations

import json
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
