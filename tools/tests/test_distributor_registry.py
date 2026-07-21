"""🆕 2026-07-03 分发器注册表模式单测。"""
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from lib import distributor_registry as dr  # noqa: E402


class DistributorRegistryTests(unittest.TestCase):
    """注册表读写 + scan 测试。"""

    def setUp(self):
        # 用临时 HOME 隔离测试，避免污染真实 ~/.ae-sdd/distributors.json
        self.tmp = tempfile.TemporaryDirectory()
        self.real_home = os.environ.get("HOME")
        self.real_userprofile = os.environ.get("USERPROFILE")
        os.environ["HOME"] = self.tmp.name
        os.environ["USERPROFILE"] = self.tmp.name
        # 清除可能缓存的状态
        dr.registry_path.cache_clear() if hasattr(dr.registry_path, "cache_clear") else None

    def tearDown(self):
        if self.real_home is not None:
            os.environ["HOME"] = self.real_home
        else:
            os.environ.pop("HOME", None)
        if self.real_userprofile is not None:
            os.environ["USERPROFILE"] = self.real_userprofile
        else:
            os.environ.pop("USERPROFILE", None)
        self.tmp.cleanup()

    def _registry_path(self) -> Path:
        return Path(self.tmp.name) / ".ae-sdd" / "distributors.json"

    def test_load_registry_seeds_defaults_when_absent(self):
        """首次加载无文件时，用种子初始化（含 5 个，mavis 默认禁用）。"""
        entries = dr.load_registry()
        names = [e.name for e in entries]
        self.assertEqual(len(entries), 5)
        self.assertIn("claude", names)
        self.assertIn("mavis", names)
        # mavis 默认禁用
        mavis = next(e for e in entries if e.name == "mavis")
        self.assertFalse(mavis.enabled)
        # 文件已落盘
        self.assertTrue(self._registry_path().is_file())

    def test_register_one_new(self):
        """注册新分发器。"""
        ok, msg, entries = dr.register_one(
            name="myagent", protocol="copytree",
            target_path="~/myagent/skills/ae-sdd",
            detect="path_exists", notes="test",
        )
        self.assertTrue(ok)
        self.assertIn("myagent", [e.name for e in entries])
        entry = next(e for e in entries if e.name == "myagent")
        self.assertEqual(entry.protocol, "copytree")
        self.assertTrue(entry.enabled)

    def test_register_duplicate_without_force_fails(self):
        """重名注册不加 --force 失败。"""
        ok, msg, _ = dr.register_one(
            name="claude", protocol="copytree",
            target_path="~/other/skills/ae-sdd",
        )
        self.assertFalse(ok)
        self.assertIn("已存在", msg)

    def test_register_duplicate_with_force_overwrites(self):
        """重名注册加 --force 覆盖。"""
        ok, _, entries = dr.register_one(
            name="claude", protocol="copytree",
            target_path="~/new/skills/ae-sdd",
            force=True,
        )
        self.assertTrue(ok)
        entry = next(e for e in entries if e.name == "claude")
        self.assertEqual(entry.target_path, "~/new/skills/ae-sdd")

    def test_register_invalid_protocol_fails(self):
        """非法协议失败。"""
        ok, msg, _ = dr.register_one(
            name="bad", protocol="unknown",
            target_path="~/bad",
        )
        self.assertFalse(ok)
        self.assertIn("未知协议", msg)

    def test_register_cli_exists_requires_detect_cli(self):
        """detect=cli_exists 时必须指定 detect_cli。"""
        ok, msg, _ = dr.register_one(
            name="bad", protocol="copytree",
            target_path="~/bad", detect="cli_exists",
        )
        self.assertFalse(ok)
        self.assertIn("detect-cli", msg)

    def test_unregister_one(self):
        """注销分发器：从注册表删除。"""
        ok, msg, entries = dr.unregister_one("codex")
        self.assertTrue(ok)
        self.assertNotIn("codex", [e.name for e in entries])

    def test_unregister_nonexistent_fails(self):
        """注销不存在的分发器失败。"""
        ok, msg, _ = dr.unregister_one("nonexistent")
        self.assertFalse(ok)
        self.assertIn("不存在", msg)

    def test_set_enable_disable(self):
        """启用/禁用分发器（软注销/恢复）。"""
        # 禁用 claude（默认启用）
        ok, msg, _ = dr.set_enabled("claude", False)
        self.assertTrue(ok)
        entries = dr.load_registry()
        self.assertFalse(next(e for e in entries if e.name == "claude").enabled)
        # 恢复
        ok, _, _ = dr.set_enabled("claude", True)
        entries = dr.load_registry()
        self.assertTrue(next(e for e in entries if e.name == "claude").enabled)

    def test_set_enabled_idempotent(self):
        """重复启用/禁用返回成功但消息提示已是该状态。"""
        ok, msg, _ = dr.set_enabled("mavis", False)  # mavis 默认就禁用
        self.assertTrue(ok)
        self.assertIn("已是", msg)

    def test_evaluate_detect_always(self):
        """detect=always 永远 True。"""
        entry = dr.DistributorEntry(
            name="x", protocol="copytree", target_path="~/x",
            detect="always", detect_cli=None,
        )
        self.assertTrue(dr.evaluate_detect(entry))

    def test_evaluate_detect_path_exists(self):
        """detect=path_exists 检查目录存在性。"""
        entry = dr.DistributorEntry(
            name="x", protocol="copytree", target_path="~/nonexistent_xyz",
            detect="path_exists", detect_cli=None,
        )
        self.assertFalse(dr.evaluate_detect(entry))
        # 创建目录后应 True
        Path(self.tmp.name, "nonexistent_xyz").mkdir()
        self.assertTrue(dr.evaluate_detect(entry))

    def test_scan_for_agents_returns_known(self):
        """scan_for_agents 返回已知 Agent 清单。"""
        with patch.object(dr, "_cli_exists", side_effect=lambda cli: cli == "mavis"):
            found = dr.scan_for_agents()
        names = [f["name"] for f in found]
        self.assertIn("claude", names)
        self.assertIn("mavis", names)

    def test_scan_unregistered_excludes_registered(self):
        """scan_unregistered 排除已注册的。"""
        # 默认注册表已有 5 个，scan_unregistered 应排除它们
        unreg = dr.scan_unregistered()
        registered = {e.name for e in dr.load_registry()}
        for item in unreg:
            self.assertNotIn(item["name"], registered)

    def test_resolved_target_expands_home(self):
        """resolved_target 展开 ~ 为 home。"""
        entry = dr.DistributorEntry(
            name="x", protocol="copytree", target_path="~/foo",
            detect="always", detect_cli=None,
        )
        resolved = entry.resolved_target()
        self.assertFalse(str(resolved).startswith("~"))
        self.assertTrue(str(resolved).endswith("foo"))


if __name__ == "__main__":
    unittest.main()
