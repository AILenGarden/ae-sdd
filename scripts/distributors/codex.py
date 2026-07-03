"""Codex CLI 分发器（兼容 shim）。

🆕 2026-07-03 注册表模式：分发器实例现由 ~/.ae-sdd/distributors.json 驱动构造。
本文件保留为向后兼容 shim，供旧测试与直接 import 使用。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class CodexDistributor(CopytreeDistributor):
    """兼容 shim：Codex → ~/.codex/skills/ae-sdd（copytree, detect=path_exists|cli）。"""

    def __init__(self) -> None:
        target = Path.home() / ".codex" / "skills" / SKILL_NAME

        def _detect() -> bool:
            return target.exists() or bool(shutil.which("codex") or shutil.which("codex.exe"))

        super().__init__(name="codex", target_path=target, detect_fn=_detect)
