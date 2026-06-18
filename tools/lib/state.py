"""
state.py — ae-sdd 项目状态管理

state.json 结构（v1）：
{
  "version": "1",
  "projectKey": "...",
  "phase": "initialized" | "dr-generated" | "story-generated" | ...,
  "currentStory": "STORY-001" | null,
  "currentTask": "TASK-001" | null,
  "history": [
    { "phase": "...", "timestamp": "...", "by": "..." }
  ]
}
"""
from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# 允许的 phase 流转（v1 简单版）
PHASE_FLOW = [
    "initialized",       # ae-sdd init 完成
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
        "initialized":     ("dr-generated",     "生成 DR（Design Requirement）",        "dr-generate-skill.md"),
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
