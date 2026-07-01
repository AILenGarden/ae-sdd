import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import document_storage  # noqa: E402


def _setup_project() -> Path:
    tmp = Path(tempfile.mkdtemp())
    (tmp / ".ae-sdd" / "assets").mkdir(parents=True, exist_ok=True)
    (tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
    (tmp / ".ae-sdd" / "assets" / "test.assets.md").write_text(
        f"# §A §B §C §D §E §F §G\n\n| gitPath | `{tmp}` |\n| docWorkspacePath | `{tmp}` |\n",
        encoding="utf-8",
    )
    return tmp


class TestResolvePathVersion(unittest.TestCase):

    def test_dict_major_minor_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "TEST_REPORT",
            story_id="STORY-001", version={"major": 2, "minor": 3},
        )
        self.assertTrue(r.full_path.endswith("STORY-001-Report-v2-r3.md"))

    def test_dict_v_r_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "TEST_REPORT",
            story_id="STORY-001", version={"v": 2, "r": 4},
        )
        self.assertTrue(r.full_path.endswith("STORY-001-Report-v2-r4.md"))

    def test_string_version(self):
        tmp = _setup_project()
        r = document_storage.resolve_path(
            tmp / ".ae-sdd", "test", "CODING_REPORT",
            story_id="STORY-001", version="v3-r5",
        )
        self.assertTrue(r.full_path.endswith("STORY-001-CodingReport-v3-r5.md"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
