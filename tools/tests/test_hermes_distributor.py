from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from distributors.hermes import HermesDistributor  # noqa: E402


class TestHermesDistributorDetect(unittest.TestCase):
    def test_detects_existing_skills_root(self):
        with tempfile.TemporaryDirectory() as td:
            home = Path(td)
            (home / ".hermes" / "skills").mkdir(parents=True)

            with patch.object(Path, "home", return_value=home), \
                 patch("distributors.hermes.shutil.which", return_value=None):
                self.assertTrue(HermesDistributor().detect())

    def test_detects_existing_target(self):
        with tempfile.TemporaryDirectory() as td:
            home = Path(td)
            (home / ".hermes" / "skills" / "ae-sdd").mkdir(parents=True)

            with patch.object(Path, "home", return_value=home), \
                 patch("distributors.hermes.shutil.which", return_value=None):
                self.assertTrue(HermesDistributor().detect())

    def test_detects_cli(self):
        with tempfile.TemporaryDirectory() as td:
            home = Path(td)

            def fake_which(name: str) -> str | None:
                return "C:/bin/hermes.exe" if name == "hermes.exe" else None

            with patch.object(Path, "home", return_value=home), \
                 patch("distributors.hermes.shutil.which", side_effect=fake_which):
                self.assertTrue(HermesDistributor().detect())

    def test_returns_false_without_root_target_or_cli(self):
        with tempfile.TemporaryDirectory() as td:
            home = Path(td)

            with patch.object(Path, "home", return_value=home), \
                 patch("distributors.hermes.shutil.which", return_value=None):
                self.assertFalse(HermesDistributor().detect())


if __name__ == "__main__":
    unittest.main(verbosity=2)
