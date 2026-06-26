"""
state.py — ae-sdd 项目状态管理

state.json 结构（v2）：

Story/Task/Plan 级（txn 级）：
{
  "version": "1",
  "projectKey": "...",
  "phase": "initialized" | "dr-generated" | ...,
  "currentStory": "STORY-001" | null,
  "currentTask": "TASK-001" | null,
  "history": [
    { "phase": "...", "timestamp": "...", "by": "..." }
  ],
  "events": [               # 🆕 v3.4.1 — append-only 流程操作日志
    {
      "seq": 1,
      "ts": "2026-06-26T10:00:00Z",
      "event": "routed-to",       # FlowEventType.value
      "node": "RA",               # FlowNode.value
      "by": "ae-sdd",
      "skill": "requirement-analysis-skill",   # FlowSkill.value（可选）
      "txnName": "STORY-001-BE",  # 子任务标识（可选，PRD 级 state 用）
      "reason": "...",            # 路由依据（可选）
      ...
    }
  ]
}

PRD 级（.auto-engineering/{PRD-ID}/state.json）：
同上结构，events 中用 txnName 字段区分不同子任务的事件，
PRD 自身事件 txnName=null。

events 字段由 flow_enums.FlowEvent 定义结构，
所有字符串值沿用 FlowNode / FlowSkill / FlowEventType 枚举的 .value。
"""
from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from lib.flow_enums import FlowEvent, FlowEventType, FlowNode, FlowSkill  # noqa: F401

# 允许的 phase 流转（v1 简单版）
# 🆕 v3.4.0：新增 ra-generated（RA 需求分析阶段，在 initialized → dr-generated 之间）
#   修复 B3-6：ra 阶段 memory 覆盖（STATE_PHASE_TO_MEMORY_PHASE 加 ra-generated→ra）
PHASE_FLOW = [
    "initialized",       # ae-sdd init 完成
    "ra-generated",      # 🆕 v3.4.0 RA 需求分析完成（进 dr-generate 前置）
    "dr-generated",      # DR 文档生成
    "story-generated",   # Story 文档生成
    "story-reviewed",    # Story Review 通过
    "task-generated",    # Task 文档生成
    "task-reviewed",     # Task Review 通过
    "coding",            # 编码中
    "test-running",      # 测试中
    "code-reviewed",     # CodeReview 通过
    "completed",         # 工程完成
]


def read_state(state_path: Path) -> dict:
    """读 state.json，不存在则返回空模板"""
    if not state_path.is_file():
        return {
            "version": "1",
            "projectKey": None,
            "phase": "initialized",
            "currentStory": None,
            "currentTask": None,
            "history": [],
        }
    return json.loads(state_path.read_text(encoding="utf-8"))


def write_state(state_path: Path, state: dict) -> None:
    """写 state.json（原子写：先写 .tmp 再 rename）"""
    state_path.parent.mkdir(parents=True, exist_ok=True)
    tmp = state_path.with_suffix(".json.tmp")
    tmp.write_text(
        json.dumps(state, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    tmp.replace(state_path)


def record_history(state: dict, phase: str, by: str = "ae-sdd") -> None:
    """追加历史记录"""
    state.setdefault("history", []).append({
        "phase": phase,
        "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "by": by,
    })


def set_phase(state: dict, phase: str, by: str = "ae-sdd") -> bool:
    """
    设置当前 phase + 记录历史。

    Returns:
        True: phase 实际被更新
        False: phase 等于当前值，不重复记录

    Raises:
        ValueError: phase 不在 PHASE_FLOW 中
    """
    if phase not in PHASE_FLOW:
        raise ValueError(f"未知 phase: {phase}（允许: {PHASE_FLOW}）")
    if state.get("phase") == phase:
        # 重复写：跳过 history 累积
        return False
    state["phase"] = phase
    record_history(state, phase, by)
    return True


def next_step_suggestion(state: dict) -> dict:
    """
    根据当前 phase 给出下一步建议（v1 简单版）。

    返回 {"current": phase, "next": ..., "action": ..., "skill": "..."}
    - "current": 当前 phase
    - "next": 下一步要写入的 phase（与 PHASE_FLOW 一致，可直接传给 state write --phase）
    - "action": 建议执行的动作（动词）
    - "skill": 对应的 SKILL 文件
    """
    cur = state.get("phase", "initialized")
    # next 必须与 PHASE_FLOW 中的 phase 名一致，避免 next-step 建议与 state write 不匹配
    mapping = {
        "initialized":     ("ra-generated",     "🆕 v3.4.0 跑需求分析（RA）+ G-RA 门卫",  "requirement-analysis-skill.md"),
        "ra-generated":    ("dr-generated",     "生成 DR（Design Requirement）",        "dr-generate-skill.md"),
        "dr-generated":    ("story-generated",  "生成 Story（从 DR）",                  "story-generate-skill.md"),
        "story-generated": ("story-reviewed",   "执行 Story Review（含 F-Stage 前端契约）", "story-review-skill.md"),
        "story-reviewed":  ("task-generated",   "生成 Task",                            "testcase-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                     "task-generate-skill.md"),
        "task-reviewed":   ("coding",           "生成 CodingPlan + 编码（⑦ 前置）",      "coding-skill.md"),
        "coding":          ("test-running",     "跑测试 + 出具测试报告",                "coding-skill.md"),
        "test-running":    ("code-reviewed",    "出具 Coding 报告 + CodeReview",         "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",          "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                        "—"),
    }
    next_phase, action, skill = mapping.get(cur, ("?", "未知 phase", "?"))
    return {
        "current": cur,
        "next": next_phase,
        "action": action,
        "skill": skill,
    }


# ─── 🆕 v3.4.1 events 操作日志 ───────────────────────────────────────────────


def _now_ts() -> str:
    """返回当前 UTC 时间的 ISO 8601 字符串，格式 2026-06-26T10:00:00Z。"""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _next_seq(state: dict) -> int:
    """返回 events 列表下一个 seq 编号（从 1 开始，自动自增）。"""
    events = state.get("events", [])
    if not events:
        return 1
    return max(e.get("seq", 0) for e in events) + 1


def append_event(state: dict, event: FlowEvent) -> None:
    """向 state["events"] 追加一条事件（append-only，不做去重）。

    自动处理：
      - 若 event.seq <= 0，用 _next_seq() 自动填充
      - 若 event.ts 为空，用当前 UTC 时间填充
      - state 中 events 键不存在时自动初始化为 []

    Args:
        state: read_state() 返回的 dict，会原地修改
        event: FlowEvent 实例（由 flow_enums 工厂函数构造）
    """
    # 自动填充 seq / ts
    if event.seq <= 0:
        event.seq = _next_seq(state)
    if not event.ts:
        event.ts = _now_ts()

    state.setdefault("events", []).append(event.to_dict())


def get_events(
    state: dict,
    *,
    txn_name: Optional[str] = None,
    event_type: Optional[str] = None,
    node: Optional[str] = None,
) -> list[dict]:
    """读取 events，支持按 txnName / event 类型 / node 过滤。

    Args:
        state:      read_state() 返回的 dict
        txn_name:   过滤指定子任务（None = 返回全部）
        event_type: FlowEventType.value 字符串，过滤指定事件类型
        node:       FlowNode.value 字符串，过滤指定节点

    Returns:
        按 seq 升序排列的事件 dict 列表
    """
    events: list[dict] = state.get("events", [])
    if txn_name is not None:
        events = [e for e in events if e.get("txnName") == txn_name]
    if event_type is not None:
        events = [e for e in events if e.get("event") == event_type]
    if node is not None:
        events = [e for e in events if e.get("node") == node]
    return sorted(events, key=lambda e: e.get("seq", 0))
