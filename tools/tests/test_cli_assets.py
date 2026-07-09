import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CLI = REPO_ROOT / "tools" / "bin" / "ae-sdd"


class TestCliAssets(unittest.TestCase):
    def _project(self, key: str = "demo") -> Path:
        root = Path(tempfile.mkdtemp())
        ade_sdd = root / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text(
            f"projectKey: {key}\nassetPath: assets/{key}.assets.md\n",
            encoding="utf-8",
        )
        (root / "pom.xml").write_text("<project></project>\n", encoding="utf-8")
        return root

    def _run(self, *args: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(CLI), *args],
            cwd=str(cwd or REPO_ROOT),
            text=True,
            encoding="utf-8",
            capture_output=True,
        )

    def test_assets_generate_and_check_json(self):
        root = self._project()

        generated = self._run(
            "assets", "generate", "--project", "demo", "--project-dir", str(root), "--json"
        )
        self.assertEqual(generated.returncode, 0, generated.stderr + generated.stdout)
        payload = json.loads(generated.stdout)
        self.assertTrue(payload["pass"], payload)
        self.assertTrue(Path(payload["assetFile"]).is_file())

        checked = self._run(
            "assets", "check", "--project", "demo", "--project-dir", str(root), "--json"
        )
        self.assertEqual(checked.returncode, 0, checked.stderr + checked.stdout)
        check_payload = json.loads(checked.stdout)
        self.assertTrue(check_payload["pass"], check_payload)
        self.assertEqual(check_payload["missingSections"], [])


if __name__ == "__main__":
    unittest.main()
