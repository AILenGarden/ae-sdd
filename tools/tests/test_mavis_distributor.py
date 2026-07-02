from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from distributors._base import DistributeContext  # noqa: E402
from distributors.mavis import MavisDistributor  # noqa: E402
from build_harness import mavis_harness_name_for_path  # noqa: E402


class TestMavisDistributor(unittest.TestCase):
    def _ctx(self, repo: Path) -> DistributeContext:
        return DistributeContext(repo_root=repo, dist_path=repo / "dist" / "ae-sdd", quiet=True)

    def test_verify_rejects_empty_harness_list(self):
        with patch("build_harness.run_mavis", return_value=(0, '{"ok":true,"harnesses":[]}')):
            self.assertFalse(MavisDistributor().verify(self._ctx(REPO_ROOT)))

    def test_install_requires_verify_after_successful_mount(self):
        with tempfile.TemporaryDirectory() as td:
            repo = Path(td)
            source = repo / ".harness"
            source.mkdir()
            calls: list[list[str]] = []
            mounted = False

            def fake_run_mavis(args: list[str]) -> tuple[int, str]:
                nonlocal mounted
                calls.append(args)
                if args[:2] == ["harness", "mount"]:
                    mounted = True
                    return 0, 'Harness "d-item-ae-sdd" mounted'
                if args[:2] == ["harness", "list"]:
                    if mounted:
                        return 0, '{"ok":true,"harnesses":[{"name":"d-item-ae-sdd","displayName":"ae-sdd"}]}'
                    return 0, '{"ok":true,"harnesses":[]}'
                return 0, ""

            with patch("build_harness.find_mavis_cmd", return_value=["mavis"]), \
                 patch("build_harness.run_mavis", side_effect=fake_run_mavis):
                result = MavisDistributor().install(source, self._ctx(repo))

            self.assertEqual(result.status, "ok")
            self.assertIn(["harness", "unmount", mavis_harness_name_for_path(repo)], calls)
            self.assertIn(["harness", "unmount", mavis_harness_name_for_path(repo / "harness")], calls)
            self.assertIn(["harness", "mount", str(repo)], calls)
            self.assertIn(["harness", "list"], calls)

    def test_install_keeps_existing_mount(self):
        with tempfile.TemporaryDirectory() as td:
            repo = Path(td)
            source = repo / ".harness"
            source.mkdir()
            calls: list[list[str]] = []

            def fake_run_mavis(args: list[str]) -> tuple[int, str]:
                calls.append(args)
                if args[:2] == ["harness", "list"]:
                    return 0, '{"ok":true,"harnesses":[{"name":"d-item-ae-sdd","displayName":"ae-sdd"}]}'
                return 0, ""

            with patch("build_harness.find_mavis_cmd", return_value=["mavis"]), \
                 patch("build_harness.run_mavis", side_effect=fake_run_mavis):
                result = MavisDistributor().install(source, self._ctx(repo))

            self.assertEqual(result.status, "ok")
            self.assertNotIn(["harness", "unmount", mavis_harness_name_for_path(repo)], calls)
            self.assertNotIn(["harness", "mount", str(repo)], calls)

    def test_install_fails_when_mount_returns_zero_but_verify_fails(self):
        with tempfile.TemporaryDirectory() as td:
            repo = Path(td)
            source = repo / ".harness"
            source.mkdir()

            def fake_run_mavis(args: list[str]) -> tuple[int, str]:
                if args[:2] == ["harness", "mount"]:
                    return 0, "No harnesses found to mount"
                if args[:2] == ["harness", "list"]:
                    return 0, '{"ok":true,"harnesses":[]}'
                return 0, ""

            with patch("build_harness.find_mavis_cmd", return_value=["mavis"]), \
                 patch("build_harness.run_mavis", side_effect=fake_run_mavis):
                result = MavisDistributor().install(source, self._ctx(repo))

            self.assertEqual(result.status, "fail")


if __name__ == "__main__":
    unittest.main(verbosity=2)
