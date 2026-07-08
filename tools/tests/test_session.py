"""
test_session.py — session.py 单元测试（🆕 v3.9.3 走 R6 顶层名空间）

覆盖：
- session_path 走 R6 顶层目录
- enter 写入 R6 顶层目录
- 幂等键：projectKey + workItemKey
- 旧 raw story_id 调用方式（破坏性）→ 报错
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import session as session_mod  # noqa: E402
from lib import paths as paths_mod  # noqa: E402


class TestSessionPathV393(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="session-v393-"))
        self.ade_sdd = self.tmp / ".ae-sdd"
        self.ade_sdd.mkdir()

    def test_session_path_r6_top(self):
        """v3.9.3 session.json 走 R6 顶层目录。"""
        sp = session_mod.session_path(
            self.ade_sdd, top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
        )
        expected = self.tmp / ".auto-engineering" / "Story-006" / "session.json"
        self.assertEqual(sp, expected)

    def test_enter_writes_to_r6_dir(self):
        """enter 写入 R6 顶层目录。"""
        s = session_mod.enter(
            "icec-test", top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
            ade_sdd=self.ade_sdd,
        )
        self.assertEqual(s["workItemKey"], "Story-006")
        self.assertEqual(s["topNode"], "STORY")
        sp = self.tmp / ".auto-engineering" / "Story-006" / "session.json"
        self.assertTrue(sp.is_file())
        loaded = json.loads(sp.read_text(encoding="utf-8"))
        self.assertEqual(loaded["sessionId"], s["sessionId"])

    def test_enter_idempotent_same_key(self):
        """幂等键 projectKey + workItemKey 一致 → 同一 token。"""
        s1 = session_mod.enter(
            "icec-test", top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
            ade_sdd=self.ade_sdd,
        )
        s2 = session_mod.enter(
            "icec-test", top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
            ade_sdd=self.ade_sdd,
        )
        self.assertEqual(s1["sessionId"], s2["sessionId"])

    def test_enter_different_work_item_creates_new_session(self):
        """不同 workItemKey → 不同 session.json（独立顶层名）。"""
        s1 = session_mod.enter(
            "icec-test", top_node="STORY",
            features={"story_ids": ["STORY-005-BE"]},
            ade_sdd=self.ade_sdd,
        )
        s2 = session_mod.enter(
            "icec-test", top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
            ade_sdd=self.ade_sdd,
        )
        self.assertNotEqual(s1["sessionId"], s2["sessionId"])
        self.assertEqual(s1["workItemKey"], "Story-005")
        self.assertEqual(s2["workItemKey"], "Story-006")

    def test_session_path_no_top_node_fallback(self):
        """top_node 空 → 项目级 session.json（向后兼容）。"""
        sp = session_mod.session_path(self.ade_sdd, top_node="")
        self.assertEqual(sp, self.ade_sdd / "session.json")

    def test_has_valid_entry_token_v393(self):
        """v3.9.3 has_valid_entry_token 走 R6 顶层名。"""
        self.assertFalse(session_mod.has_valid_entry_token(
            self.ade_sdd, top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
        ))
        session_mod.enter(
            "icec-test", top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
            ade_sdd=self.ade_sdd,
        )
        self.assertTrue(session_mod.has_valid_entry_token(
            self.ade_sdd, top_node="STORY",
            features={"story_ids": ["STORY-006-BE"]},
        ))

    def test_read_session_finds_legacy_raw_story_dir(self):
        """legacy .auto-engineering/STORY-ID/session.json must remain readable."""
        legacy = self.tmp / ".auto-engineering" / "STORY-004-BE" / "session.json"
        legacy.parent.mkdir(parents=True)
        legacy.write_text(json.dumps({
            "sessionId": "legacy-sid",
            "projectKey": "icec-test",
            "storyId": "STORY-004-BE",
            "userConfirmedPhases": [],
        }), encoding="utf-8")

        loaded = session_mod.read_session(self.ade_sdd, "STORY-004-BE")

        self.assertIsNotNone(loaded)
        self.assertEqual(loaded["sessionId"], "legacy-sid")
        self.assertTrue(session_mod.has_valid_entry_token(self.ade_sdd, "STORY-004-BE"))

    def test_confirm_phase_updates_existing_legacy_session(self):
        """state confirm --story should append to an existing legacy session instead of creating a new R6 token."""
        legacy = self.tmp / ".auto-engineering" / "STORY-004-BE" / "session.json"
        legacy.parent.mkdir(parents=True)
        legacy.write_text(json.dumps({
            "sessionId": "legacy-sid",
            "projectKey": "icec-test",
            "storyId": "STORY-004-BE",
            "userConfirmedPhases": [],
        }), encoding="utf-8")

        session_mod.confirm_phase(self.ade_sdd, "task-reviewed", story_id="STORY-004-BE")

        updated = json.loads(legacy.read_text(encoding="utf-8"))
        self.assertEqual(updated["sessionId"], "legacy-sid")
        self.assertEqual(updated["userConfirmedPhases"][0]["phase"], "task-reviewed")
        self.assertFalse((self.tmp / ".auto-engineering" / "Story-004" / "session.json").exists())

    def test_read_session_finds_legacy_raw_story_for_double_segment_work_item(self):
        """legacy work-item keys like STORY-ID--name should resolve back to the raw Story session."""
        legacy = self.tmp / ".auto-engineering" / "STORY-004-BE" / "session.json"
        legacy.parent.mkdir(parents=True)
        legacy.write_text(json.dumps({
            "sessionId": "legacy-sid",
            "projectKey": "icec-test",
            "storyId": "STORY-004-BE",
            "userConfirmedPhases": [],
        }), encoding="utf-8")

        loaded = session_mod.read_session(
            self.ade_sdd,
            top_node="STORY",
            features={"story_ids": ["STORY-004-BE--车主端预约单操作-BE"]},
        )

        self.assertIsNotNone(loaded)
        self.assertEqual(loaded["sessionId"], "legacy-sid")


if __name__ == "__main__":
    unittest.main(verbosity=2)
