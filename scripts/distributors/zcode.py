"""ZCode CLI 分发器：copytree → ~/.zcode/skills/ae-sdd。

逻辑迁自 install.py 的 ZCODE_DST 分支（v3.4.0+ post-commit hook 默认装 zcode）。
auto 模式下：ZCode skills 根目录存在、目标目录已存在或 zcode CLI 可用时包含。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class ZcodeDistributor(CopytreeDistributor):
    name = "zcode"

    def skills_root(self) -> Path:
        return Path.home() / ".zcode" / "skills"

    def target_path(self) -> Path:
        return self.skills_root() / SKILL_NAME

    def detect(self) -> bool:
        """auto 模式：ZCode skills 根目录存在、ae-sdd 已安装或 CLI 存在时包含。"""
        dst = self.target_path()
        return (
            self.skills_root().is_dir()
            or dst.exists()
            or bool(shutil.which("zcode") or shutil.which("zcode.exe"))
        )
