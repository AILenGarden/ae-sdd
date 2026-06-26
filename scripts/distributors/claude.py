"""Claude Code 分发器：copytree → ~/.claude/skills/ae-sdd。

逻辑迁自 install.py 的 CLAUDE_DST 分支（auto 模式下永远包含）。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class ClaudeDistributor(CopytreeDistributor):
    name = "claude"

    def target_path(self) -> Path:
        return Path.home() / ".claude" / "skills" / SKILL_NAME

    def detect(self) -> bool:
        """auto 模式：Claude 目标永远包含（向后兼容，迁自 install.py:_target_paths）。"""
        return True
