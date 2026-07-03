"""Mavis 分发器（兼容 shim）。

🆕 2026-07-03 注册表模式：harness_mount 协议逻辑已迁入 _base.HarnessMountDistributor。
本文件保留为向后兼容 shim，供旧测试与直接 import 使用。
新代码应通过注册表 + HarnessMountDistributor 构造，不需此子类。
"""
from __future__ import annotations

from pathlib import Path

from ._base import HarnessMountDistributor

MAVIS_HOME = Path.home() / ".mavis"


class MavisDistributor(HarnessMountDistributor):
    """兼容 shim：Mavis → ~/.mavis（harness_mount, detect=cli_exists mavis）。"""

    def __init__(self) -> None:
        # build_harness.py 与本包同级（scripts/），由调用方保证 sys.path 含 scripts/
        def _detect() -> bool:
            try:
                from build_harness import find_mavis_cmd
                return find_mavis_cmd() is not None
            except ImportError:
                return False

        super().__init__(
            name="mavis",
            agent_home=MAVIS_HOME,
            detect_fn=_detect,
        )
