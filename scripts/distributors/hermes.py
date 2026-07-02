"""Hermes 分发器：copytree → ~/.hermes/skills/ae-sdd。

逻辑对齐 codex.py/zcode.py（标准 copytree 协议）。
auto 模式下：目录已存在 或 hermes CLI 可用时才包含。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class HermesDistributor(CopytreeDistributor):
    name = "hermes"

    def skills_root(self) -> Path:
        return Path.home() / ".hermes" / "skills"

    def target_path(self) -> Path:
        return self.skills_root() / SKILL_NAME

    def detect(self) -> bool:
        """auto 模式：hermes 已安装或 CLI 存在时包含（同 codex/zcode 判定模式）。"""
        dst = self.target_path()
        return (
            self.skills_root().is_dir()
            or dst.exists()
            or bool(shutil.which("hermes") or shutil.which("hermes.exe"))
        )
