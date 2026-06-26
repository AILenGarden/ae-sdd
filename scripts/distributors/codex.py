"""Codex CLI 分发器：copytree → ~/.codex/skills/ae-sdd。

逻辑迁自 install.py 的 CODEX_DST 分支。
auto 模式下：目录已存在 或 codex CLI 可用时才包含。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class CodexDistributor(CopytreeDistributor):
    name = "codex"

    def target_path(self) -> Path:
        return Path.home() / ".codex" / "skills" / SKILL_NAME

    def detect(self) -> bool:
        """auto 模式：codex 已安装或 CLI 存在时包含（迁自 install.py:_target_paths）。"""
        dst = self.target_path()
        return dst.exists() or bool(shutil.which("codex") or shutil.which("codex.exe"))
