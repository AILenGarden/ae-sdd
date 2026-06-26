"""ZCode CLI 分发器：copytree → ~/.zcode/skills/ae-sdd。

逻辑迁自 install.py 的 ZCODE_DST 分支（v3.4.0+ post-commit hook 默认装 zcode）。
auto 模式下：目录已存在 或 zcode CLI 可用时才包含。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class ZcodeDistributor(CopytreeDistributor):
    name = "zcode"

    def target_path(self) -> Path:
        return Path.home() / ".zcode" / "skills" / SKILL_NAME

    def detect(self) -> bool:
        """auto 模式：zcode 已安装或 CLI 存在时包含（迁自 install.py:_target_paths）。"""
        dst = self.target_path()
        return dst.exists() or bool(shutil.which("zcode") or shutil.which("zcode.exe"))
