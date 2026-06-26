"""分发器注册/发现机制。

通过 __init__.py 的 DISTRIBUTORS 显式注册表 + 本模块的 get_active_distributors()
实现"auto 模式只跑 detect=True 的分发器"。
"""
from __future__ import annotations

from typing import Optional

from ._base import Distributor, DistributeContext, log_warn


def get_active_distributors(
    ctx: Optional[DistributeContext] = None,
    target_filter: Optional[str] = None,
) -> list[Distributor]:
    """返回应该执行的分发器实例列表。

    - target_filter=None（auto）：返回所有 detect()=True 的分发器
    - target_filter="all"：返回所有已注册分发器（不 detect，强制全跑）
    - target_filter="<name>"：只返回指定 name 的分发器（不 detect，强制单跑）
    - target_filter="<path>"（以 / 或盘符开头）：由调用方处理 target_path，这里返回空
    """
    # 延迟 import 避免循环依赖
    from . import DISTRIBUTORS

    if target_filter in (None, "auto"):
        return [cls() for cls in DISTRIBUTORS if cls().detect()]
    if target_filter == "all":
        return [cls() for cls in DISTRIBUTORS]

    # 单个 name
    matched = [cls() for cls in DISTRIBUTORS if cls().name == target_filter]
    if not matched:
        log_warn(ctx, f"未找到名为 '{target_filter}' 的分发器，已知: "
                      f"{[cls().name for cls in DISTRIBUTORS]}")
    return matched


def list_registered() -> list[str]:
    """返回所有已注册分发器的 name（调试/帮助用）。"""
    from . import DISTRIBUTORS
    return [cls().name for cls in DISTRIBUTORS]
