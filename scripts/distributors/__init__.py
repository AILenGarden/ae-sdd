"""ae-sdd 分发器注册表（🆕 2026-07-03 注册表模式）。

分发器实例现由 ~/.ae-sdd/distributors.json 驱动，不再需要在此硬编码列表。
新增 Agent：用 `ae-sdd distributor register <name> ...` 注册，或编辑 JSON。
详见 distributors/_base.py 的 CopytreeDistributor / HarnessMountDistributor。

兼容：DISTRIBUTORS 列表保留为空（旧代码 `from . import DISTRIBUTORS` 不破，
但 get_active_distributors 现走 JSON 注册表，不走此列表）。
"""
from ._base import (
    Distributor,
    CopytreeDistributor,
    HarnessMountDistributor,
    DistributeContext,
    InstallResult,
)

# 🆕 2026-07-03：注册表模式后，分发器实例由 JSON 驱动构造，此列表保留为空仅向后兼容。
# 旧代码 [cls() for cls in DISTRIBUTORS] 会得到空列表，应改用 get_active_distributors()。
DISTRIBUTORS: list[type[Distributor]] = []

__all__ = [
    "Distributor",
    "CopytreeDistributor",
    "HarnessMountDistributor",
    "DistributeContext",
    "InstallResult",
    "DISTRIBUTORS",
]
