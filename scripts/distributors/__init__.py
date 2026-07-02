"""ae-sdd 分发器注册表。

新增 Agent：在此列表 import + 注册即可，无需改 distribute.py / install.py。
详见 distributors/_base.py 顶部「如何新增一个 Agent 分发器」3 步法。
"""
from ._base import Distributor, CopytreeDistributor, DistributeContext, InstallResult
from .claude import ClaudeDistributor
from .codex import CodexDistributor
from .zcode import ZcodeDistributor
from .hermes import HermesDistributor
from .mavis import MavisDistributor

# 已注册分发器类（顺序=auto 模式下的执行顺序）。
# copytree 类在前（共用 dist 包），mavis 在后（需专属编译 agent.md）。
DISTRIBUTORS: list[type[Distributor]] = [
    ClaudeDistributor,
    CodexDistributor,
    ZcodeDistributor,
    HermesDistributor,
    MavisDistributor,
]

__all__ = [
    "Distributor",
    "CopytreeDistributor",
    "DistributeContext",
    "InstallResult",
    "DISTRIBUTORS",
]
