"""ZCode CLI 分发器（兼容 shim）。

🆕 2026-07-03 注册表模式：分发器实例现由 ~/.ae-sdd/distributors.json 驱动构造。
本文件保留为向后兼容 shim，供旧测试与直接 import 使用。
"""
from __future__ import annotations

import shutil
from pathlib import Path

from ._base import CopytreeDistributor

SKILL_NAME = "ae-sdd"


class ZcodeDistributor(CopytreeDistributor):
    """兼容 shim：ZCode → ~/.zcode/skills/ae-sdd（copytree, detect=path_exists|cli）。"""

    def __init__(self) -> None:
        skills_root = Path.home() / ".zcode" / "skills"
        target = skills_root / SKILL_NAME

        def _detect() -> bool:
            return (
                skills_root.is_dir()
                or target.exists()
                or bool(shutil.which("zcode") or shutil.which("zcode.exe"))
            )

        super().__init__(name="zcode", target_path=target, detect_fn=_detect)
        self._skills_root = skills_root

    def skills_root(self) -> Path:
        """保留旧接口（测试可能调用）。"""
        return self._skills_root
