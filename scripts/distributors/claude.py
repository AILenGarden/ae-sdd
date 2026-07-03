"""Claude Code 分发器（兼容 shim）。

🆕 2026-07-03 注册表模式：分发器实例现由 ~/.ae-sdd/distributors.json 驱动构造。
本文件保留为向后兼容 shim，供旧测试与直接 import 使用。
"""
from __future__ import annotations

from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class ClaudeDistributor(CopytreeDistributor):
    """兼容 shim：Claude Code → ~/.claude/skills/ae-sdd（copytree, detect=always）。"""

    def __init__(self) -> None:
        super().__init__(
            name="claude",
            target_path=Path.home() / ".claude" / "skills" / SKILL_NAME,
            detect_fn=lambda: True,
        )
