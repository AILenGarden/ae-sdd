"""分发器注册/发现机制（🆕 2026-07-03 注册表模式）。

从 ~/.ae-sdd/distributors.json 读取注册表，按 enabled + detect 过滤，
用协议模板（CopytreeDistributor / HarnessMountDistributor）构造实例。

注销/禁用 = 注册表 enabled:false 或条目除名 → 实例不再构造 → auto 分发跳过。
"""
from __future__ import annotations

from pathlib import Path
from typing import Optional

from ._base import (
    Distributor,
    CopytreeDistributor,
    HarnessMountDistributor,
    DistributeContext,
    log_warn,
)


def _tools_lib_path() -> str:
    """tools/ 目录绝对路径（用于 import lib.distributor_registry）。"""
    return str(Path(__file__).resolve().parents[2] / "tools")


def _build_distributor(entry) -> Optional[Distributor]:
    """根据注册表条目构造分发器实例。

    entry: distributor_registry.DistributorEntry
    返回 None 表示协议未知或构造失败。
    """
    import sys
    tools_dir = _tools_lib_path()
    if tools_dir not in sys.path:
        sys.path.insert(0, tools_dir)
    from lib.distributor_registry import evaluate_detect

    # detect_fn 闭包：捕获 entry，运行时调 evaluate_detect
    def detect_fn(e=entry):
        return evaluate_detect(e)

    if entry.protocol == "copytree":
        return CopytreeDistributor(
            name=entry.name,
            target_path=entry.resolved_target(),
            detect_fn=detect_fn,
        )
    if entry.protocol == "harness_mount":
        return HarnessMountDistributor(
            name=entry.name,
            agent_home=entry.resolved_target(),
            detect_fn=detect_fn,
        )
    return None


def get_active_distributors(
    ctx: Optional[DistributeContext] = None,
    target_filter: Optional[str] = None,
) -> list[Distributor]:
    """返回应该执行的分发器实例列表。

    - target_filter=None（auto）：返回所有 enabled + detect()=True 的分发器
    - target_filter="all"：返回所有 enabled 分发器（不 detect，强制全跑）
    - target_filter="<name>"：只返回指定 name（不 detect，强制单跑；disabled 也跑）
    - target_filter="<path>"（以 / 或盘符开头）：由调用方处理 target_path，这里返回空
    """
    import sys
    tools_dir = _tools_lib_path()
    if tools_dir not in sys.path:
        sys.path.insert(0, tools_dir)
    from lib.distributor_registry import load_registry, find_entry

    entries = load_registry()

    if target_filter in (None, "auto"):
        result = []
        for entry in entries:
            if not entry.enabled:
                continue
            dist = _build_distributor(entry)
            if dist is not None and dist.detect():
                result.append(dist)
        return result
    if target_filter == "all":
        result = []
        for entry in entries:
            if not entry.enabled:
                continue
            dist = _build_distributor(entry)
            if dist is not None:
                result.append(dist)
        return result

    # 单个 name（含 disabled，强制单跑）
    entry = find_entry(entries, target_filter)
    if entry is None:
        log_warn(ctx, f"未找到名为 '{target_filter}' 的分发器，已知: "
                      f"{[e.name for e in entries]}")
        return []
    dist = _build_distributor(entry)
    return [dist] if dist is not None else []


def list_registered() -> list[str]:
    """返回所有已注册分发器的 name（调试/帮助用）。"""
    import sys
    tools_dir = _tools_lib_path()
    if tools_dir not in sys.path:
        sys.path.insert(0, tools_dir)
    from lib.distributor_registry import load_registry
    return [e.name for e in load_registry()]
