"""
🆕 v3.10.3: memory_gate.py 已废弃（过渡期保留为 passthrough）。

原功能：
  - check_state_transition: state write 转移前校验 memory enter/write/exit 顺序。
  - format_transition_block: 拒绝块格式化。
  - memory_phase_for_state_phase: state phase -> memory phase 映射。
  - is_forward_transition: 按 scale 选子链判定向前转移。

废弃原因：
  新 memory 体系（业务实体树 + compact.md 存储）废弃了 enter/exit 生命周期门禁。
  子流程启动=创建(编译) memory，结束=删除 memory。"活跃"="memory 存在"。
  不再需要 enter/write/exit 顺序校验。

过渡策略：
  - memory_phase_for_state_phase 迁入 memory_store（供 prompt_inject/gate_intercept 过渡期使用）。
  - check_state_transition 改为永远 pass（不再阻断 state write）。
  - format_transition_block 返回空字符串（不再输出拒绝块）。
  - 批 3 重写 prompt_inject/gate_intercept/CLI 后，本文件可彻底删除。
"""
from __future__ import annotations

from pathlib import Path
from typing import Optional

from lib import memory_store


# 从 memory_store re-export（过渡期兼容，供 prompt_inject/gate_intercept 使用）
def memory_phase_for_state_phase(phase: str) -> Optional[str]:
    """🆕 v3.10.3: 委托 memory_store.memory_phase_for_state_phase。"""
    return memory_store.memory_phase_for_state_phase(phase)


def is_forward_transition(
    current_phase: str,
    target_phase: str,
    scale: Optional[str] = None,
) -> bool:
    """过渡期保留：按 scale 选子链判定向前转移。"""
    from lib.state import PHASE_FLOWS, VALID_SCALES
    from lib import state as state_mod
    chain = PHASE_FLOWS[scale] if scale in VALID_SCALES else state_mod.PHASE_FLOW
    try:
        current_idx = chain.index(current_phase)
        target_idx = chain.index(target_phase)
    except ValueError:
        return False
    return target_idx > current_idx


def check_state_transition(
    *,
    ade_sdd: Optional[Path],
    state_data: dict,
    target_phase: str,
    allow_empty: bool = False,
) -> dict:
    """🆕 v3.10.3: 废弃门禁，永远 pass。

    新 memory 体系不再强制 enter/write/exit 顺序。子流程通过 create_memory/clean_memory
    管理生命周期，不需要 state write 时校验 memory 状态。
    """
    return {
        "pass": True,
        "blocked": False,
        "skipped": True,
        "reason": "memory gate deprecated in v3.10.3 (entity-tree memory replaces enter/exit lifecycle)",
    }


def format_transition_block(check: dict) -> str:
    """🆕 v3.10.3: 废弃，返回空字符串（不再输出拒绝块）。"""
    return ""
