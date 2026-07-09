import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))

from lib import gates, project_assets  # noqa: E402
from scripts import init as init_script  # noqa: E402


class TestProjectAssets(unittest.TestCase):
    def _project(self, key: str = "demo") -> tuple[Path, Path]:
        root = Path(tempfile.mkdtemp())
        (root / ".ae-sdd").mkdir()
        (root / ".ae-sdd" / "config.yaml").write_text(
            f"projectKey: {key}\nassetPath: assets/{key}.assets.md\n",
            encoding="utf-8",
        )
        (root / "pom.xml").write_text("<project></project>\n", encoding="utf-8")
        (root / "src" / "main" / "java").mkdir(parents=True)
        (root / "src" / "main" / "resources").mkdir(parents=True)
        (root / "src" / "main" / "resources" / "application.yml").write_text(
            "server:\n  port: 8080\n",
            encoding="utf-8",
        )
        return root, root / ".ae-sdd"

    def test_generate_writes_g00_ready_asset(self):
        root, ade_sdd = self._project()

        result = project_assets.generate_project_assets(ade_sdd, "demo", project_root=root)

        self.assertTrue(result.pass_)
        self.assertTrue(result.asset_file.is_file())
        text = result.asset_file.read_text(encoding="utf-8")
        for section in project_assets.REQUIRED_SECTIONS:
            self.assertIn(section, text)
        check = project_assets.check_asset(ade_sdd, "demo")
        self.assertTrue(check["pass"], check)

    def test_generate_repairs_incomplete_asset_with_backup(self):
        root, ade_sdd = self._project()
        asset_file = ade_sdd / "assets" / "demo.assets.md"
        asset_file.parent.mkdir(parents=True)
        asset_file.write_text("# incomplete\n", encoding="utf-8")

        result = project_assets.generate_project_assets(ade_sdd, "demo", project_root=root)

        self.assertTrue(result.pass_)
        self.assertTrue(result.backup_file and result.backup_file.is_file())
        self.assertIn("§A", asset_file.read_text(encoding="utf-8"))

    def test_existing_complete_asset_is_kept_without_force(self):
        root, ade_sdd = self._project()
        first = project_assets.generate_project_assets(ade_sdd, "demo", project_root=root)

        second = project_assets.generate_project_assets(ade_sdd, "demo", project_root=root)

        self.assertTrue(first.changed)
        self.assertFalse(second.changed)
        self.assertTrue(second.pass_)

    def test_init_generates_asset_and_g00_passes(self):
        root = Path(tempfile.mkdtemp())
        (root / "pom.xml").write_text("<project></project>\n", encoding="utf-8")

        rc = init_script.init_project(root, "demo", no_hooks=True)

        self.assertEqual(rc, 0)
        ade_sdd = root / ".ae-sdd"
        check = project_assets.check_asset(ade_sdd, "demo")
        self.assertTrue(check["pass"], check)
        g00 = gates.check_g00(None, ade_sdd, "demo")
        self.assertTrue(g00.pass_, g00.message)


if __name__ == "__main__":
    unittest.main()
