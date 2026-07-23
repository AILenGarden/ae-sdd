"""
state.py — ae-sdd 项目状态管理

state.json 结构（v2）：

Story/Task/Plan 级（txn 级）：
{
  "version": "1",
  "projectKey": "...",
  "phase": "initialized" | "dr-generated" | ... | "paused",
  "scale": "大"|"中"|"小"|"微",   # 🆕 v3.5.15 任务规模，决定走哪条子链；旧 state 缺失则 _infer_scale 反推
  "entryNode": "BUG"|"CONFIG"|"PRD"|"RA"|... | null,  # 🆕 v3.5.15 入口节点语义（FlowNode.value）
  "pausedFromPhase": "coding" | null,   # 🆕 v3.6 paused 前的 phase（resume 时恢复目标）
  "pauseReason": "level3-escalation" | "user-rejected" | "user-manual" | null,  # 🆕 v3.6
  "correctionCounts": { "coding": 2 },  # 🆕 v3.6 各 phase 矫正次数（flow_monitor Level 判定）
  "currentStory": "STORY-001" | null,
  "currentTask": "TASK-001" | null,
  "activeAgents": [ ... ],          # 🆕 v3.5.12 运行中的 sub-agent 列表（agent 生命周期）
  "agentReports": [ ... ],          # 🆕 v3.5.12 已完成 sub-agent 报告
  "fileLocks": {                    # 🆕 v3.8.1 S-3 文件意图锁（防多 agent 并发写同一产物）
    "<相对路径>": {"agentId": "...", "acquiredAt": "ISO8601", "ttlSeconds": 1800}
  },
  "currentPhase": "coding",       # 工作流投影字段：必须由 set_phase / set_story_substate_phase 跟 phase 同步
  "currentStep": "step-4-coding-r1",
  "completedSteps": [ ... ],
  "pendingOutputs": {},           # phase=completed 时必须为空
  "codingRound": 1,               # phase=completed 时必须 >= r1
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

🆕 v3.5.15 多入口状态机：4 子链 + scale 路由（详见 PHASE_FLOWS 注释）
🆕 v3.10.0 砍 Task phase + Route 下移重分级：
  - 移除所有链中的 task-generated/task-reviewed（骨架分解合并进 CodingProcess）
  - 大=DR 入口（原 PRD 层弃用，RA 降为前置条件不在链内生成）
  - 中=Story 入口（原 小链）
  - 小=CodingPlan 入口（新增，直出 CodingPlan）
  - 微=无文档（BUG/配置，直出 CodingPlan）
  - 大链（11 phase）：已有DR initialized->dr-generated->...->completed
  - 中链（10 phase）：已有Story，跳DR，从Story系列入
  - 小链（6 phase）：已有Story+TestCase，直出CodingPlan
  - 微链（6 phase）：BUG/调整，无文档直出CodingPlan
  - 旧 state 无 scale -> _infer_scale 按 completedSteps/phase 反推，默认“大”（最保守）

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

# ─── 🆕 v3.5.15 多入口状态机：4 子链 + scale 路由 ─────────────────────────────
# 旧版（v3.4.x 及之前）：单条线性 PHASE_FLOW，起点硬编码 initialized、首步强制 ra-generated，
#   所有规模共用，导致微任务/小任务/BUG 在状态机里"看不见"——next_step 对微任务给"跑 RA"错误建议；
#   gate_intercept「禁止跨步跳跃」对微任务正经跑 state write --phase coding 会撞墙。
# v3.5.15：按业务层 4 类需求（大/中/小/微）拆 4 条子链，每条只含该规模真实经过的节点。
#   scale 由 classify() 判定，首次 state write --scale 携带写入；旧 state 无 scale → _infer_scale 反推。
#   BUG/配置类 → scale="微" + entryNode=BUG/CONFIG（复用微链，不单独开链）。
PHASE_FLOWS: dict[str, list[str]] = {
    "大": [
        "initialized", "route-selected", "requirement-analyzed", "dr-generated", "story-generated",
        "testcase-generated", "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "中": [
        "initialized", "route-selected", "requirement-analyzed", "story-generated", "testcase-generated",
        "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "小": [
        "initialized", "route-selected", "requirement-analyzed", "coding-process", "coding",
        "test-running", "code-reviewed", "completed",
    ],
    "微": [
        "initialized", "route-selected", "requirement-analyzed", "coding-process", "coding",
        "test-running", "code-reviewed", "completed",
    ],
}

COMPACT_PHASE_FLOWS: dict[str, list[str]] = {
    "大": ["initialized", "route-selected", "requirement-analyzed", "dr-generated", "story-generated",
           "coding-process", "coding", "test-running", "code-reviewed", "completed"],
    "中": ["initialized", "route-selected", "requirement-analyzed", "story-generated", "coding-process", "coding",
           "test-running", "code-reviewed", "completed"],
    "小": ["initialized", "route-selected", "requirement-analyzed", "coding-process", "coding",
           "test-running", "code-reviewed", "completed"],
    "微": ["initialized", "route-selected", "requirement-analyzed", "coding-process", "coding",
           "test-running", "code-reviewed", "completed"],
}

VALID_DESIGN_ROUTES = ("DR", "STORY", "CODING_PLAN")

# 合法 scale 集合（与 classify.py SCALE 值一致）
VALID_SCALES = ("大", "中", "小", "微")

# 向后兼容别名：旧代码/测试引用 PHASE_FLOW 时仍可用，等价于大链（最保守主干）。
# 🟡 deprecated：新代码应改用 PHASE_FLOWS[scale]。未来版本删除。
PHASE_FLOW = PHASE_FLOWS["大"]


def _default_execution_plan() -> dict:
    return {
        "goal": "",
        "changedPaths": [],
        "verification": [],
        "risks": [],
        "sourceReads": [],
        "approved": False,
        "approvedAt": None,
        "approvedBy": None,
    }


def _default_review_state() -> dict:
    return {
        "status": "pending",
        "findings": [],
        "reviewedPaths": [],
        "evidenceIds": [],
        "updatedAt": None,
    }


def phase_flows_for_state(value: dict) -> dict[str, list[str]]:
    return COMPACT_PHASE_FLOWS if value.get("processPolicy") == "compact" else PHASE_FLOWS


def phase_chain_for_state(value: dict) -> list[str]:
    """Return the selected route's phase chain, with legacy phase compatibility."""
    scale = _resolve_scale(value)
    base = list(phase_flows_for_state(value)[scale])
    decision = value.get("routeDecision") or {}
    selected = decision.get("selectedDesign") or value.get("selectedDesign")
    if selected == "DR":
        return [phase for phase in base if phase != "story-generated"]
    if selected == "STORY":
        return [phase for phase in base if phase != "dr-generated"]
    if selected == "CODING_PLAN":
        return [phase for phase in base if phase not in {
            "dr-generated", "story-generated", "testcase-generated",
        }]
    return base


def ensure_process_state(value: dict) -> dict:
    """Backfill compact process state for legacy and nested Work Items."""
    plan = value.setdefault("executionPlan", _default_execution_plan())
    for key, default in _default_execution_plan().items():
        plan.setdefault(key, default)
    review = value.setdefault("review", _default_review_state())
    for key, default in _default_review_state().items():
        review.setdefault(key, default)
    value.setdefault("routeDecision", {})
    value.setdefault("requirementSpec", {})
    return value


def set_design_route(state: dict, selected_design: str, *, reason: str = "",
                     by: str = "user") -> dict:
    """Persist the post-analysis design selection used by dynamic phase routing."""
    selected = str(selected_design or "").strip().upper()
    if selected not in VALID_DESIGN_ROUTES:
        raise ValueError(f"未知 design route: {selected}（允许: {VALID_DESIGN_ROUTES}）")
    decision = state.setdefault("routeDecision", {})
    decision["selectedDesign"] = selected
    decision["reason"] = str(reason or "").strip()
    decision["selectedBy"] = by
    decision["selectedAt"] = _now_ts()
    state["selectedDesign"] = selected
    record_history(state, f"design-route-selected:{selected}", by)
    return decision


def read_state(state_path: Path) -> dict:
    """读 state.json，不存在则返回空模板"""
    if not state_path.is_file():
        return ensure_process_state({
            "version": "1",
            "projectKey": None,
            "phase": "initialized",
            "scale": None,            # 🆕 v3.5.15 任务规模（大/中/小/微），首次 state write 写入
            "entryNode": None,        # 🆕 v3.5.15 入口节点语义（FlowNode.value，如 BUG/CONFIG/PRD）
            "pausedFromPhase": None,  # 🆕 v3.6 paused 前的 phase（resume 时恢复目标）
            "pauseReason": None,      # 🆕 v3.6 暂停原因（level3-escalation|user-rejected|user-manual）
            "correctionCounts": {},   # 🆕 v3.6 各 phase 矫正次数，键=phase 值=int
            "currentStory": None,
            "currentTask": None,
            "history": [],
        })
    return ensure_process_state(json.loads(state_path.read_text(encoding="utf-8")))


def write_state(state_path: Path, state: dict) -> None:
    """写 state.json（原子写：先写 .tmp 再 rename）"""
    ensure_process_state(state)
    validate_state_invariants(state)
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


def set_execution_plan(state: dict, *, goal: str, changed_paths: list[str],
                       verification: list[dict], risks: list[str],
                       source_reads: Optional[list[str]] = None,
                       by: str = "ae-sdd") -> dict:
    """Persist the compact pre-coding plan; updating it clears approval."""
    ensure_process_state(state)
    normalized_paths = [str(item).replace("\\", "/") for item in changed_paths if str(item).strip()]
    plan = {
        "goal": str(goal or "").strip(),
        "changedPaths": list(dict.fromkeys(normalized_paths)),
        "verification": list(verification or []),
        "risks": [str(item).strip() for item in (risks or []) if str(item).strip()],
        "sourceReads": list(dict.fromkeys(
            str(item).replace("\\", "/") for item in (source_reads or []) if str(item).strip()
        )),
        "approved": False,
        "approvedAt": None,
        "approvedBy": None,
    }
    state["executionPlan"] = plan
    record_history(state, "execution-plan-updated", by)
    return plan


def approve_execution_plan(state: dict, *, by: str = "user") -> dict:
    """Approve a complete compact execution plan."""
    ensure_process_state(state)
    plan = state["executionPlan"]
    missing = [
        name for name, value in (
            ("goal", plan.get("goal")),
            ("changedPaths", plan.get("changedPaths")),
            ("verification", plan.get("verification")),
        ) if not value
    ]
    if missing:
        raise ValueError(f"executionPlan 缺必填字段: {missing}")
    plan["approved"] = True
    plan["approvedAt"] = _now_ts()
    plan["approvedBy"] = by
    record_history(state, "execution-plan-approved", by)
    return plan


def record_review(state: dict, *, status: str, findings: list[dict],
                  reviewed_paths: Optional[list[str]] = None,
                  evidence_ids: Optional[list[str]] = None,
                  by: str = "ae-sdd") -> dict:
    """Record findings-only review state; no Markdown report is generated."""
    if status not in {"pending", "passed", "changes_required"}:
        raise ValueError(f"未知 review status: {status}")
    normalized_findings = list(findings or [])
    invalid_findings = [
        item for item in normalized_findings
        if not isinstance(item, dict) or not str(item.get("severity") or "").strip()
    ]
    if invalid_findings:
        raise ValueError("review findings 必须是包含 severity 的对象")
    if status == "passed" and normalized_findings:
        raise ValueError("review status=passed 时 findings 必须为空")
    if status == "changes_required" and not normalized_findings:
        raise ValueError("review status=changes_required 时 findings 不能为空")
    review = {
        "status": status,
        "findings": normalized_findings,
        "reviewedPaths": list(dict.fromkeys(str(item) for item in (reviewed_paths or []))),
        "evidenceIds": list(dict.fromkeys(str(item) for item in (evidence_ids or []))),
        "updatedAt": _now_ts(),
    }
    state["review"] = review
    record_history(state, f"review-{status}", by)
    return review


def _infer_scale(state: dict) -> tuple[str, float, str]:
    """🆕 v3.5.15 旧 state 兼容：无 scale 字段时按 completedSteps/phase 反推规模。
    🆕 v3.10.0 Route 下移重分级后更新推断优先级。

    推断优先级（任一命中即定）：
      1. completedSteps 含 dr -> 大（DR 入口，完整主干）
      2. completedSteps 含 story 但无 dr -> 中（Story 入口）
      3. completedSteps 含 coding 但无 story -> 小（CodingPlan 入口）
      4. phase ∈ {coding,test-running,code-reviewed,completed} 且 completedSteps 无 story -> 微
      5. 无法判定 -> 默认“大”（最保守，含全主干）

    Returns:
        (scale, confidence, reason)；confidence<0.5 时调用方应 warn 提示用户显式 --scale
    """
    completed = state.get("completedSteps") or []
    completed_text = " ".join(completed)
    phase = state.get("phase", "initialized")

    has_ra = any("ra" in (s or "").lower() for s in completed)
    has_dr = any("dr" in (s or "").lower() for s in completed)
    has_story = any("story" in (s or "").lower() for s in completed)
    has_coding = any("coding" in (s or "").lower() for s in completed)

    if has_ra or has_dr:
        return ("大", 0.9, "completedSteps 含 ra/dr -> 大（4 loop 完整主干）")
    if has_story:
        return ("中", 0.85, "completedSteps 含 story 但无 ra/dr -> 中（Story 入口）")
    if has_coding:
        # 🟡 v3.10.0：coding 阶段无法可靠区分小链（CodingPlan 入口）vs 微链（无文档）
        #   反推“小”置信度 0.8（会告警），要求用户显式 --scale=微。
        return ("小", 0.8, "completedSteps 含 coding 但无 story -> 小（CodingPlan 入口）")
    # 默认最保守
    return ("大", 0.3, "无法判定规模，默认大（最保守，需用户显式 --scale）")


def _resolve_scale(state: dict) -> str:
    """🆕 v3.5.15 解析 state 的 scale：有则用，无则 _infer_scale 推断。

    推断时把推断结果回写 state["scale"]（避免每次重复推断）。
    """
    scale = state.get("scale")
    if scale in VALID_SCALES:
        return scale
    scale, _conf, _reason = _infer_scale(state)
    state["scale"] = scale  # 回写，后续调用直接命中
    return scale


def set_scale(state: dict, scale: str, entry_node: Optional[str] = None) -> None:
    """🆕 v3.5.15 写入 scale + entryNode（首次 state write --scale 时调用）。

    Args:
        state: state dict（原地修改）
        scale: 大/中/小/微
        entry_node: 入口节点语义（FlowNode.value，如 BUG/CONFIG/PRD/RA/DR/STORY/TASK/PLAN），可选

    Raises:
        ValueError: scale 不在 VALID_SCALES 中
    """
    if scale not in VALID_SCALES:
        raise ValueError(f"未知 scale: {scale}（允许: {VALID_SCALES}）")
    state["scale"] = scale
    if entry_node is not None:
        state["entryNode"] = entry_node


_CODING_STARTED_PHASES = {"coding", "test-running", "code-reviewed", "completed"}
_TERMINAL_PHASE = "completed"
_TERMINAL_STEP = "completed"


def _set_if_changed(state: dict, key: str, value) -> bool:
    if state.get(key) == value:
        return False
    state[key] = value
    return True


def _pending_outputs_empty(value) -> bool:
    if value is None:
        return True
    if value is False:
        return True
    if isinstance(value, (dict, list, tuple, set, str)):
        return len(value) == 0
    return False


def _empty_pending_outputs_like(value):
    if isinstance(value, list):
        return []
    if isinstance(value, tuple):
        return []
    return {}


def _clear_pending_outputs(state: dict) -> bool:
    empty_value = _empty_pending_outputs_like(state.get("pendingOutputs"))
    return _set_if_changed(state, "pendingOutputs", empty_value)


def _coding_round_number(value) -> int:
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (int, float)):
        return int(value)
    if isinstance(value, str):
        text = value.strip().lower()
        if text.startswith("r") and text[1:].isdigit():
            return int(text[1:])
        if text.isdigit():
            return int(text)
    return 0


def _ensure_coding_round_at_least_started(state: dict) -> bool:
    if _coding_round_number(state.get("codingRound")) >= 1:
        return False
    state["codingRound"] = 1
    return True


def _sync_phase_projection(state: dict, phase: str) -> bool:
    """Keep lifecycle phase and workflow projection fields in sync."""
    changed = False
    changed |= _set_if_changed(state, "currentPhase", phase)
    if phase == "paused":
        return changed
    changed |= set_current_step(state, _TERMINAL_STEP if phase == _TERMINAL_PHASE else phase)
    if phase in _CODING_STARTED_PHASES:
        changed |= _ensure_coding_round_at_least_started(state)
    if phase == _TERMINAL_PHASE:
        changed |= _clear_pending_outputs(state)
    return changed


def _validate_workflow_state_projection(state: dict, label: str) -> None:
    if state.get("phase") != _TERMINAL_PHASE:
        return
    errors = []
    if state.get("currentPhase") != _TERMINAL_PHASE:
        errors.append("currentPhase must be completed")
    if state.get("currentStep") != _TERMINAL_STEP:
        errors.append("currentStep must be completed")
    if not _pending_outputs_empty(state.get("pendingOutputs")):
        errors.append("pendingOutputs must be empty")
    if _coding_round_number(state.get("codingRound")) < 1:
        errors.append("codingRound must be >= r1")
    if errors:
        joined = "; ".join(errors)
        raise ValueError(
            f"state invariant violation ({label}): phase=completed requires "
            f"currentPhase=completed, currentStep=completed, empty pendingOutputs, "
            f"and codingRound>=r1; violations: {joined}"
        )


def _iter_story_projection_records(state: dict):
    seen: set[int] = set()
    story_states = state.get("storyStates") or {}
    if isinstance(story_states, dict):
        for story_id, sub in story_states.items():
            if isinstance(sub, dict) and id(sub) not in seen:
                seen.add(id(sub))
                yield f"storyStates.{story_id}", sub
    dr_states = state.get("drStates") or {}
    if isinstance(dr_states, dict):
        for dr_id, dr_state in dr_states.items():
            if not isinstance(dr_state, dict):
                continue
            nested_story_states = dr_state.get("storyStates") or {}
            if not isinstance(nested_story_states, dict):
                continue
            for story_id, sub in nested_story_states.items():
                if isinstance(sub, dict) and id(sub) not in seen:
                    seen.add(id(sub))
                    yield f"drStates.{dr_id}.storyStates.{story_id}", sub


def validate_state_invariants(state: dict) -> None:
    """Reject contradictory terminal workflow state before persistence."""
    if not isinstance(state, dict):
        return
    if state.get("stateModel") != "nested" or any(
        key in state for key in ("currentPhase", "currentStep", "pendingOutputs", "codingRound")
    ):
        _validate_workflow_state_projection(state, "root")
    for label, sub in _iter_story_projection_records(state):
        _validate_workflow_state_projection(sub, label)


def set_phase(state: dict, phase: str, by: str = "ae-sdd") -> bool:
    """
    设置当前 phase + 记录历史（🆕 v3.5.15 按 state.scale 选子链校验）。

    🆕 v3.6：`paused` 是元状态，不在 PHASE_FLOWS 子链中，任何 phase 均可跳入。
      - 设置 paused 时自动保存 pausedFromPhase（当前 phase）
      - 从 paused 恢复请用 resume_state()，不要直接 set_phase

    Returns:
        True: phase 或同步投影字段实际被更新
        False: phase 与同步投影均未变化，不重复记录

    Raises:
        ValueError: phase 不在该 state 所在子链中（paused 除外）
    """
    # 🆕 v3.6 paused 元状态：绕过子链校验，直接写入
    if phase == "paused":
        if state.get("phase") == "paused":
            changed = _sync_phase_projection(state, "paused")
            validate_state_invariants(state)
            return changed  # 已经是 paused，幂等
        state["pausedFromPhase"] = state.get("phase", "initialized")
        state["phase"] = "paused"
        record_history(state, "paused", by)
        _sync_phase_projection(state, "paused")
        validate_state_invariants(state)
        return True

    scale = _resolve_scale(state)
    chain = phase_chain_for_state(state)
    if phase not in chain:
        raise ValueError(
            f"未知 phase: {phase}（scale={scale} 子链允许: {chain}）。"
            f"若切换规模请先 set_scale；全主干参考: {phase_flows_for_state(state)['大']}"
        )
    if state.get("phase") == phase:
        # 重复写：跳过 history 累积
        changed = _sync_phase_projection(state, phase)
        validate_state_invariants(state)
        return changed
    state["phase"] = phase
    record_history(state, phase, by)
    _sync_phase_projection(state, phase)
    validate_state_invariants(state)
    return True


# ─── 🆕 v3.5.15 各子链 next_step mapping ─────────────────────────────────────
# key = scale；每条子链独立 mapping，next 与 PHASE_FLOWS[scale] 一致。
_NEXT_STEP_MAPPINGS: dict[str, dict[str, tuple[str, str, str]]] = {
    "大": {
        "initialized":     ("route-selected", "记录任务路由决策", "ae-sdd classify"),
        "route-selected":  ("requirement-analyzed", "生成并确认本次任务需求说明书", "requirement-analysis-skill.md"),
        "requirement-analyzed": ("dr-generated", "按分析结论进入 DR 设计", "dr-generate-skill.md"),
        "ra-generated":    ("dr-generated", "从 legacy RA 进入 DR 设计", "dr-generate-skill.md"),
        "dr-generated":    ("story-generated",  "生成 Story（从 DR）",                    "story-generate-skill.md"),
        "story-generated": ("coding-process",  "生成紧凑 executionPlan，在对话中确认；验证矩阵默认内嵌 Story", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按已确认 executionPlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行测试并记录 evidence，不生成 TestReport", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "执行 Review；通过记状态，失败只记 findings", "code-review-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 -> completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "-"),
    },
    "中": {
        "initialized":     ("route-selected", "记录任务路由决策", "ae-sdd classify"),
        "route-selected":  ("requirement-analyzed", "生成并确认本次任务需求说明书", "requirement-analysis-skill.md"),
        "requirement-analyzed": ("story-generated", "按分析结论进入 Story 设计", "story-generate-skill.md"),
        "story-generated": ("coding-process",  "生成紧凑 executionPlan，在对话中确认", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按已确认 executionPlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行测试并记录 evidence，不生成 TestReport", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "执行 Review；通过记状态，失败只记 findings", "code-review-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 -> completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "-"),
    },
    "小": {
        "initialized":     ("route-selected", "记录任务路由决策", "ae-sdd classify"),
        "route-selected":  ("requirement-analyzed", "生成紧凑需求说明书", "requirement-analysis-skill.md"),
        "requirement-analyzed": ("coding-process", "按分析结论直接设计并确认 executionPlan", "coding-process-skill.md"),
        "story-generated": ("coding-process", "从 legacy Story-lite 生成 executionPlan", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按已确认 executionPlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行测试并记录 evidence，不生成 TestReport", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "执行 Review；通过记状态，失败只记 findings", "code-review-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 -> completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "-"),
    },
    "微": {
        "initialized":     ("route-selected", "记录任务路由决策", "ae-sdd classify"),
        "route-selected":  ("requirement-analyzed", "生成 Story-lite 深度的需求说明书", "requirement-analysis-skill.md"),
        "requirement-analyzed": ("coding-process", "直接设计并确认极简 executionPlan", "coding-process-skill.md"),
        "story-generated": ("coding-process", "从 legacy Story-lite 生成 executionPlan", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按已确认 executionPlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行最小测试并记录 evidence", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "执行 findings-only Review", "code-review-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 -> completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "-"),
    },
}

_LEGACY_NEXT_STEP_OVERRIDES: dict[str, dict[str, tuple[str, str, str]]] = {
    "大": {
        "story-generated": ("testcase-generated", "生成独立 TestCase（legacy）", "testcase-generate-skill.md"),
        "testcase-generated": ("coding-process", "生成 legacy CodingPlan", "coding-process-skill.md"),
        "test-running": ("code-reviewed", "Test Review 通过后生成 legacy 报告 + CodeReview", "coding-report-skill.md"),
    },
    "中": {
        "initialized": ("dr-generated", "执行 legacy DR generate+review loop", "dr-generate-skill.md"),
        "dr-generated": ("story-generated", "生成 Story（从 DR）", "story-generate-skill.md"),
        "story-generated": ("testcase-generated", "生成 legacy TestCase", "testcase-generate-skill.md"),
        "testcase-generated": ("coding-process", "生成 legacy CodingPlan", "coding-process-skill.md"),
        "test-running": ("code-reviewed", "Test Review 通过后生成 legacy Coding 报告 + CodeReview", "coding-report-skill.md"),
    },
    "小": {
        "initialized": ("coding-process", "执行 legacy CodingProcess（已有 Story+TestCase）", "coding-process-skill.md"),
        "test-running": ("code-reviewed", "Test Review 通过后生成 legacy Coding 报告 + CodeReview", "coding-report-skill.md"),
    },
    "微": {
        "initialized": ("coding-process", "执行 legacy 微任务 CodingProcess", "coding-process-skill.md"),
        "test-running": ("code-reviewed", "Test Review 通过后生成 legacy Coding 报告 + CodeReview", "coding-report-skill.md"),
    },
}


def next_step_suggestion(state: dict) -> dict:
    """
    根据当前 phase 给出下一步建议（🆕 v3.5.15 按 state.scale 选子链 mapping）。
    🆕 v3.6：paused 元状态特殊处理——返回恢复目标 phase。

    返回 {"current": phase, "next": ..., "action": ..., "skill": "..."}
    - "current": 当前 phase
    - "next": 下一步要写入的 phase（与 PHASE_FLOWS[scale] 一致，可直接传给 state write --phase）
    - "action": 建议执行的动作（动词）
    - "skill": 对应的 SKILL 文件
    """
    cur = get_active_phase(state) or state.get("phase", "initialized")

    # 🆕 v3.6 paused 元状态：建议恢复到暂停前的 phase
    if cur == "paused":
        paused_from = state.get("pausedFromPhase", "initialized")
        pause_reason = state.get("pauseReason", "unknown")
        phase_cn = {
            "level3-escalation": "Level 3 矫正次数超限",
            "user-rejected":     "用户拒绝本系列产物",
            "user-manual":       "用户手动暂停",
        }.get(pause_reason, pause_reason)
        return {
            "current": "paused",
            "next": paused_from,
            "action": f"流程已暂停（{phase_cn}），恢复后继续 {paused_from} 阶段",
            "skill": "（说「继续流程」或 ae-sdd state write --resume 恢复）",
        }

    scale = _resolve_scale(state)
    mapping = _NEXT_STEP_MAPPINGS[scale]
    if state.get("processPolicy") != "compact":
        mapping = {**mapping, **_LEGACY_NEXT_STEP_OVERRIDES.get(scale, {})}
    chain = phase_chain_for_state(state)
    if cur in chain and chain.index(cur) < len(chain) - 1:
        next_phase = chain[chain.index(cur) + 1]
        mapped = mapping.get(cur)
        if mapped and mapped[0] == next_phase:
            _, action, skill = mapped
        else:
            action, skill = {
                "route-selected": ("记录任务路由决策", "ae-sdd classify"),
                "requirement-analyzed": ("生成并确认本次任务需求说明书", "requirement-analysis-skill.md"),
                "dr-generated": ("按需求分析结论生成并审查 DR", "dr-generate-skill.md"),
                "story-generated": ("按需求分析或 DR 结论生成 Story", "story-generate-skill.md"),
                "coding-process": ("生成并确认紧凑 executionPlan", "coding-process-skill.md"),
                "coding": ("执行已批准 executionPlan", "coding-skill.md"),
                "test-running": ("运行验证并记录 evidence", "test-generate-skill.md"),
                "code-reviewed": ("执行 findings-only Review", "code-review-skill.md"),
                "completed": ("等待用户最终确认", "（人工审核）"),
            }.get(next_phase, (f"进入 {next_phase}", "?"))
    elif cur == "completed":
        next_phase, action, skill = "（已结束）", "项目工程已完成", "-"
    else:
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


# ─── 🆕 v3.5.4 PRD 4 层 AND 校验（抽自 ae-sdd CLI cmd_state_prd_check_complete）─────
# HS-7 物理拦截复用：gate_intercept 拦 prd-complete 时实时跑此函数，不依赖"上次证据"
def check_prd_4_layers(prd_state: dict) -> dict:
    """校验 PRD 完成判定的 4 层 AND（G-PRD-1~4），不改状态。

    抽自 ae-sdd CLI 的 cmd_state_prd_check_complete，供 gate_intercept（HS-7 物理拦截）
    与 CLI 命令共用，确保"prd-complete 前必须 4 层 AND 全过"在 CLI 与 hook 双轨一致。

    Args:
        prd_state: PRD 级 state.json 的 dict（read_state 或 _read_prd_state 结果）

    Returns:
        { prdId, G-PRD-1..4: {label, pass, missing}, all_pass }
        all_pass=True 表示 4 层全过（可执行 prd-complete）
    """
    story_ids = prd_state.get("storyIds", [])

    # G-PRD-1: Story 全部完成
    g1_missing = []
    for s in story_ids:
        if not s.get("codeReviewReport"):
            g1_missing.append(f"{s.get('storyId')}: 缺 codeReviewReport")
        if not s.get("sevenBisPassed"):
            g1_missing.append(f"{s.get('storyId')}: ⑦bis 未通过")
        if not s.get("userConfirmedAt"):
            g1_missing.append(f"{s.get('storyId')}: 用户未确认")

    # G-PRD-2: ⑦bis 全通过
    g2_missing = [f"{s.get('storyId')}: ⑦bis matrix 有 🔴 断链" for s in story_ids
                  if not s.get("sevenBisPassed")]

    # G-PRD-3: 跨 Story 残留风险已闭环
    cross_deps = prd_state.get("crossStoryDeps", [])
    risks = prd_state.get("crossStoryResidualRisks", [])
    g3_missing = []
    for d in cross_deps:
        if not d.get("verifiedAt"):
            g3_missing.append(f"跨 Story 依赖 {d.get('fromStory')}→{d.get('toStory')} 未验证")
    for r in risks:
        if not r.get("mitigationPlan"):
            g3_missing.append(f"风险 {r.get('riskId')}: 缺 mitigationPlan")
        if r.get("severity") == "🔴" and not r.get("dueDate"):
            g3_missing.append(f"风险 {r.get('riskId')}: 🔴 风险必须设 dueDate")

    # G-PRD-4: PRD 级人工审核通过
    prd_review = prd_state.get("prdReview", {})
    g4_missing = []
    if not prd_review.get("confirmedAt"):
        g4_missing.append("PRD 级人工审核未确认（🔍 审核点 5）")
    if not prd_review.get("confirmedBy"):
        g4_missing.append("PRD 级人工审核缺确认人")

    result = {
        "prdId": prd_state.get("prdId", "unknown"),
        "G-PRD-1": {"label": "Story 全部完成", "pass": not g1_missing, "missing": g1_missing},
        "G-PRD-2": {"label": "Story ⑦bis 全通过", "pass": not g2_missing, "missing": g2_missing},
        "G-PRD-3": {"label": "跨 Story 残留风险已闭环", "pass": not g3_missing, "missing": g3_missing},
        "G-PRD-4": {"label": "PRD 级人工审核通过", "pass": not g4_missing, "missing": g4_missing},
    }
    result["all_pass"] = all(result[k]["pass"] for k in ("G-PRD-1", "G-PRD-2", "G-PRD-3", "G-PRD-4"))
    return result


# ─── 🆕 v3.5.12 多 Agent state 写入 helper（治 P0-11 死字段）──────────────────
# SKILL.md §🤖 多 Agent 状态共享承诺：启动 sub-agent 写 activeAgents、完成移 agentReports。
# v3.5.12 前这些字段零写入（死字段），AI 靠记忆维护。本节补真写入路径。

def register_agent(state: dict, agent_id: str, role: str, session_id: str,
                    sub_task: str = "") -> None:
    """启动 sub-agent 时写 activeAgents（原地修改 state，调用方负责 write_state）。"""
    agents = state.setdefault("activeAgents", [])
    # 幂等：同 agentId 不重复追加
    if any(a.get("agentId") == agent_id for a in agents):
        return
    agents.append({
        "agentId": agent_id,
        "role": role,
        "sessionId": session_id,
        "status": "running",
        "startedAt": _now_ts(),
        "currentSubTask": sub_task,
    })


def complete_agent(state: dict, agent_id: str, report_path: str = "",
                    summary: str = "") -> None:
    """sub-agent 完成时：从 activeAgents 移除 + 移入 agentReports。"""
    agents = state.setdefault("activeAgents", [])
    reports = state.setdefault("agentReports", [])
    # 从 activeAgents 移除
    state["activeAgents"] = [a for a in agents if a.get("agentId") != agent_id]
    # 移入 agentReports
    if any(r.get("agentId") == agent_id for r in reports):
        return
    reports.append({
        "agentId": agent_id,
        "reportPath": report_path,
        "summary": summary,
        "completedAt": _now_ts(),
    })


# ─── 🆕 v3.8.1 S-3：文件意图锁（防多 sub-agent 并发写同一产物）──────────────────
# SKILL.md §🤖 多 Agent 任务分配机制："禁止多个 sub-agent 并发写同一文件/同一目录"。
# v3.8.1 前该规则仅文档，无工具强制。本节提供基于 state.json 的中央意图锁：
#   - activeAgents 记录 agent 生命周期（谁在跑），fileLocks 记录文件意图（谁要写哪个文件）
#   - 两者职责正交：一个 agent 可持多把锁，一把锁只属一个 agent
#   - 锁检查由 gate_intercept 的 PreToolUse hook 在 Write/Edit 前调用（不在 write_state 内，
#     避免 state.json 自写时自锁死锁）
#   - TTL 30 分钟防 agent 崩溃后死锁（复用 AGENTS.md 分布式锁 TTL 语义），惰性失效
FILE_LOCK_TTL_SECONDS = 1800  # 锁默认有效期 30 分钟（常量，禁止魔法值）


def _file_lock_expired(lock_info: dict, now_ts: str) -> bool:
    """判断锁是否已过 TTL。now_ts 为当前 ISO8601 UTC 时间戳。"""
    acquired = lock_info.get("acquiredAt")
    if not acquired:
        return True  # 无 acquiredAt 视作无效锁，允许失效
    try:
        acquired_dt = datetime.fromisoformat(acquired.replace("Z", "+00:00"))
        now_dt = datetime.fromisoformat(now_ts.replace("Z", "+00:00"))
        ttl = int(lock_info.get("ttlSeconds", FILE_LOCK_TTL_SECONDS))
        return (now_dt - acquired_dt).total_seconds() > ttl
    except Exception:
        return True  # 时间解析异常视作过期（惰性失效，防脏数据死锁）


def check_file_lock(state: dict, path: str) -> Optional[dict]:
    """检查路径是否被锁。TTL 过期自动视作未锁（惰性失效，不写回 state）。

    Args:
        state: read_state() 返回的 dict（只读，不修改）
        path:  产物文件相对路径（相对 project_dir，正斜杠分隔）

    Returns:
        持锁信息 dict（含 agentId/acquiredAt/ttlSeconds）或 None（未锁/已过期）
    """
    locks = state.get("fileLocks") or {}
    lock_info = locks.get(path)
    if not lock_info:
        return None
    if _file_lock_expired(lock_info, _now_ts()):
        return None  # 过期视作未锁（惰性失效，调用方不感知过期细节）
    return lock_info


def acquire_file_lock(state: dict, path: str, agent_id: str,
                      ttl_seconds: int = FILE_LOCK_TTL_SECONDS) -> tuple[bool, str]:
    """获取文件意图锁（原地修改 state，调用方负责 write_state）。

    冲突时返回 (False, reason)，reason 含持锁 agentId 便于排查。
    TTL 过期的旧锁会被新 agent 抢占（防崩溃 agent 死锁）。

    Args:
        state:       read_state() 返回的 dict（原地修改）
        path:        产物文件相对路径
        agent_id:    申请锁的 sub-agent 标识
        ttl_seconds: 锁有效期（默认 30 分钟）

    Returns:
        (success, reason)：success=True 时 reason 为空；失败时 reason 含持锁方信息
    """
    locks = state.setdefault("fileLocks", {})
    existing = locks.get(path)
    if existing and existing.get("agentId") == agent_id:
        return True, ""  # 幂等：同 agent 重复获取
    if existing and not _file_lock_expired(existing, _now_ts()):
        holder = existing.get("agentId", "unknown")
        return False, f"文件 {path} 已被 agent {holder} 持锁，禁止并发写"
    # 无锁或旧锁过期 → 抢占
    locks[path] = {
        "agentId": agent_id,
        "acquiredAt": _now_ts(),
        "ttlSeconds": int(ttl_seconds),
    }
    return True, ""


def release_file_lock(state: dict, path: str, agent_id: str) -> bool:
    """释放文件意图锁（仅持锁者能释放，原地修改 state）。

    Args:
        state:    read_state() 返回的 dict（原地修改）
        path:     产物文件相对路径
        agent_id: 释放锁的 agent 标识（须与持锁 agentId 一致）

    Returns:
        True=实际释放；False=未持锁/非持锁者/锁不存在
    """
    locks = state.get("fileLocks") or {}
    existing = locks.get(path)
    if not existing or existing.get("agentId") != agent_id:
        return False
    del locks[path]
    state["fileLocks"] = locks
    return True


# ─── 🆕 v3.8.0 自动化联审共识 state 写入 helper ───────────────────────────────
# SKILL.md §🚀 自动化模式：审核点走 Tier 3 联审共识，结果写 reviewConsensus[point]。
# G-AUTO-CONSENSUS 门禁校验本字段：passed=true + reviewer 独立性（复用 G-09B）。
def register_review_consensus(state: dict, point: float, tier: int,
                              passed: bool, rounds: int,
                              reviewers: list = None,
                              stall_reason: str = "") -> None:
    """写联审共识结果到 state.reviewConsensus[point]（原地修改，调用方负责 write_state）。

    Args:
        point: 审核点编号（1/1.5/2/2.5/4/5）
        tier: reviewer Tier（自动化模式固定 3）
        passed: 联审共识是否通过
        rounds: 矫正轮次
        reviewers: reviewer 报告摘要列表 [{agentId, role, verdict, sessionId}...]
        stall_reason: 未通过时的原因（2 轮未决等）
    """
    rc = state.setdefault("reviewConsensus", {})
    rc[str(point)] = {
        "point": point,
        "tier": tier,
        "passed": bool(passed),
        "rounds": int(rounds),
        "reviewers": reviewers or [],
        "stallReason": stall_reason,
        "recordedAt": _now_ts(),
    }


def get_review_consensus(state: dict, point: float) -> Optional[dict]:
    """读 reviewConsensus[point]，不存在返回 None。"""
    rc = state.get("reviewConsensus") or {}
    return rc.get(str(point))


# ─── 🆕 v3.5.12 重入字段写入 helper（治 P1-5 死字段）──────────────────────────
# SKILL.md §流程状态跟踪承诺：currentStep/completedSteps/codingRound 真实读写。
# v3.5.12 前零写入（重入只能靠 phase 粗粒度恢复）。

def set_current_step(state: dict, step: str) -> bool:
    """进入新步骤时写 currentStep + 追加 completedSteps。"""
    changed = False
    prev = state.get("currentStep")
    if prev and prev != step:
        completed = state.get("completedSteps")
        if not isinstance(completed, list):
            completed = []
            state["completedSteps"] = completed
            changed = True
        if prev not in completed:
            completed.append(prev)
            changed = True
    if state.get("currentStep") != step:
        state["currentStep"] = step
        changed = True
    return changed


def bump_coding_round(state: dict) -> str:
    """开始新一轮 Coding 前累加 codingRound，返回新轮次标识（如 r1/r2/r3）。"""
    cur = _coding_round_number(state.get("codingRound", 0))
    new_round = cur + 1
    state["codingRound"] = new_round
    return f"r{new_round}"


# ─── 🆕 v3.5.12 PRD 子系统写入 helper（治 P0-9/10 PRD 死字段）────────────────
# document-storage §3.5 PRD schema 承诺 8 字段，v3.5.12 前零写入方（prd-init 命令
# 都不存在）。本节补真写入路径 + prdStatus 5 态闭环。

# PRD 状态机 5 态（对齐 document-storage §3.5）
PRD_STATUS_FLOW = [
    "in_progress",                  # PRD 进行中（prd-init 写入）
    "prd_complete_pending_user",    # 4层AND全过，等用户审核点5
    "awaiting_compact",             # prd-complete 写入，等 compact
    "compacted",                    # compact 成功（v3.5.12 补写入）
    "prd_aborted",                  # PRD 中止（v3.5.12 补写入）
]


def prd_init(state: dict, prd_id: str, prd_title: str = "",
              story_ids: list = None, size_budget: dict = None) -> None:
    """初始化 PRD 级 state（对应 CLI `ae-sdd state prd-init`）。

    写入 PRD schema 8 字段的初始值，让 check_prd_4_layers 有真实输入。
    """
    state["prdId"] = prd_id
    state["prdTitle"] = prd_title
    state["storyIds"] = story_ids or []
    state["crossStoryDeps"] = []
    state["crossStoryResidualRisks"] = []
    state["sizeBudget"] = size_budget or {}
    state["prdReview"] = {}
    state["gateRegistry"] = {f"G-PRD-{i}": "pending" for i in range(1, 5)}
    state["compactHistory"] = []
    state["prdStatus"] = "in_progress"


def add_story_to_prd(state: dict, story_id: str, dr_id: str = "",
                      doc_path: str = "") -> None:
    """Story 完成时把 Story 信息加入 PRD state（对应 Story 完成 hook）。"""
    story_ids = state.setdefault("storyIds", [])
    if not any(s.get("storyId") == story_id for s in story_ids):
        story_ids.append({
            "storyId": story_id,
            "drId": dr_id,
            "docPath": doc_path,
            "codeReviewReport": None,
            "sevenBisPassed": False,
            "userConfirmedAt": None,
        })


def record_story_completion(state: dict, story_id: str,
                             code_review_report: str,
                             seven_bis_passed: bool,
                             user_confirmed_at: str) -> None:
    """记录 Story 完成（codeReviewReport + sevenBisPassed + userConfirmedAt）。

    这三个字段是 check_prd_4_layers 的 G-PRD-1 依赖，v3.5.12 前无写入方。
    """
    for s in state.get("storyIds", []):
        if s.get("storyId") == story_id:
            s["codeReviewReport"] = code_review_report
            s["sevenBisPassed"] = seven_bis_passed
            s["userConfirmedAt"] = user_confirmed_at
            return


def add_cross_story_dep(state: dict, from_story: str, to_story: str,
                         dep_type: str = "") -> None:
    """记录跨 Story 依赖（check_prd_4_layers 的 G-PRD-3 依赖）。"""
    deps = state.setdefault("crossStoryDeps", [])
    if not any(d.get("fromStory") == from_story and d.get("toStory") == to_story
               for d in deps):
        deps.append({"fromStory": from_story, "toStory": to_story,
                     "type": dep_type, "verifiedAt": None})


def verify_cross_story_dep(state: dict, from_story: str, to_story: str,
                            verified_at: str) -> None:
    """标记跨 Story 依赖已验证（G-PRD-3 闭环）。"""
    for d in state.get("crossStoryDeps", []):
        if d.get("fromStory") == from_story and d.get("toStory") == to_story:
            d["verifiedAt"] = verified_at


def add_residual_risk(state: dict, risk_id: str, severity: str,
                       mitigation_plan: str = "", due_date: str = "") -> None:
    """记录跨 Story 残留风险（G-PRD-3 依赖）。"""
    risks = state.setdefault("crossStoryResidualRisks", [])
    if not any(r.get("riskId") == risk_id for r in risks):
        risks.append({"riskId": risk_id, "severity": severity,
                      "mitigationPlan": mitigation_plan, "dueDate": due_date})


def confirm_prd_review(state: dict, confirmed_by: str = "user",
                        open_questions: list = None) -> None:
    """记录 PRD 级审核点 5 确认（G-PRD-4 依赖）。"""
    state["prdReview"] = {
        "confirmedAt": _now_ts(),
        "confirmedBy": confirmed_by,
        "storytoldAt": _now_ts(),
        "openQuestions": open_questions or [],
    }


def update_gate_registry(state: dict, gate_results: dict) -> None:
    """更新 G-PRD-1~4 闸状态（check_prd_4_layers 实时算后写回）。"""
    gr = state.setdefault("gateRegistry", {})
    for gid in ("G-PRD-1", "G-PRD-2", "G-PRD-3", "G-PRD-4"):
        if gid in gate_results:
            gr[gid] = "pass" if gate_results[gid].get("pass") else "fail"


def set_prd_status(state: dict, status: str) -> bool:
    """设置 prdStatus（5 态闭环，对齐 PRD_STATUS_FLOW）。

    v3.5.12 修复：补全 compacted/prd_aborted/prd_complete_pending_user 写入，
    原本只有 awaiting_compact 能写（状态机残缺）。
    """
    if status not in PRD_STATUS_FLOW:
        raise ValueError(f"未知 prdStatus: {status}（允许: {PRD_STATUS_FLOW}）")
    state["prdStatus"] = status
    # compacted 时追加 compactHistory
    if status == "compacted":
        state.setdefault("compactHistory", []).append({
            "compactedAt": _now_ts(),
            "status": "compacted",
        })
    return True


# ─── 🆕 v3.8.1 S-5：PRD compact 前置 helper（runtime 差异化） ──────────────────
# 治 S-5 缺口：cmd_state_prd_complete 接收 --runtime 但从不分支；state.py 无 prd_complete。
# 本 helper 封装"生成 summary.md + 流转 prdStatus → awaiting_compact + 返回 runtime 差异化提示"，
# 消除 CLI 与 state 库职责分散。实际 compact 由各 runtime 的 hook/session 协议执行（保持协议骨架）。
# runtime 差异表（对齐 2026-07-02 remediation plan §1.4）：
#   mavis       → summary.md + mavis session rotate --handoff-file 指令
#   claude-code → summary.md + 写 .ae-sdd/compact-trigger 文件（UserPromptSubmit hook 读取注入 /compact）
#   codex       → summary.md + 标注"待调研"（codex 无 compact 机制）
RUNTIME_COMPACT_HINTS: dict[str, str] = {
    "mavis": "下一步：mavis session rotate --handoff-file {summary_path}",
    "claude-code": "下一步：已写 compact-trigger 文件，UserPromptSubmit hook 将注入 /compact 指令",
    "codex": "下一步：codex 无原生 compact 机制（待调研），summary.md 已生成供人工衔接",
}


def prd_complete(state: dict, prd_id: str, runtime: str,
                 project_root: Path) -> dict:
    """执行 PRD compact 前置：生成 summary.md + 流转 prdStatus → awaiting_compact。

    Args:
        state:        PRD 级 state.json 的 dict（read_state 结果，原地修改）
        prd_id:       PRD 标识
        runtime:      目标 runtime（mavis / claude-code / codex）
        project_root: 项目根路径（用于定位 .auto-engineering/{prd_id}/summary.md）

    Returns:
        {"summaryPath": str, "runtimeHint": str, "compactTrigger": bool}
        compactTrigger=True 仅 claude-code（写了 trigger 文件）
    """
    prd_dir = project_root / ".auto-engineering" / prd_id
    prd_dir.mkdir(parents=True, exist_ok=True)
    summary_path = prd_dir / "summary.md"

    # 生成 summary.md（compact 交接件，供下一 runtime/session 续接上下文）
    summary_content = _build_prd_summary(state, prd_id, runtime)
    summary_path.write_text(summary_content, encoding="utf-8")

    # 流转 prdStatus → awaiting_compact（等 runtime compact hook 触发 → compacted）
    if state.get("prdStatus") != "compacted":
        set_prd_status(state, "awaiting_compact")
    state["lastUpdated"] = _now_ts()

    # runtime 差异化：claude-code 写 compact-trigger 文件
    compact_trigger = False
    if runtime == "claude-code":
        trigger_file = project_root / ".ae-sdd" / "compact-trigger"
        trigger_file.parent.mkdir(parents=True, exist_ok=True)
        trigger_file.write_text(
            json.dumps({"prdId": prd_id, "summaryPath": str(summary_path),
                        "triggeredAt": _now_ts()}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        compact_trigger = True

    hint_template = RUNTIME_COMPACT_HINTS.get(runtime, "")
    runtime_hint = hint_template.format(summary_path=str(summary_path))

    return {
        "summaryPath": str(summary_path),
        "runtimeHint": runtime_hint,
        "compactTrigger": compact_trigger,
    }


def _build_prd_summary(state: dict, prd_id: str, runtime: str) -> str:
    """构造 PRD compact summary.md 内容（compact 交接件）。

    摘取 PRD state 的关键信息（prdId/title/storyIds/prdStatus/事件数），
    供下一 runtime/session 快速续接上下文，不重复完整 state.json。
    """
    story_ids = [s.get("storyId", "") if isinstance(s, dict) else str(s)
                 for s in state.get("storyIds", [])]
    events = state.get("events", [])
    return (
        f"# PRD {prd_id} Compact Summary\n\n"
        f"- **prdId**: {prd_id}\n"
        f"- **prdTitle**: {state.get('prdTitle', '')}\n"
        f"- **runtime**: {runtime}\n"
        f"- **prdStatus**: {state.get('prdStatus', 'unknown')}\n"
        f"- **storyIds**: {', '.join(story_ids) or '(无)'}\n"
        f"- **events 数**: {len(events)}\n"
        f"- **generatedAt**: {_now_ts()}\n\n"
        f"## 交接说明\n\n"
        f"本文件由 `ae-sdd state prd-complete --runtime {runtime}` 生成，"
        f"供 {runtime} runtime 续接上下文。完整 PRD state 见同目录 state.json。\n"
    )


# ─── 🆕 v3.10.3 subprocessAgent 管理（3层Agent模型）─────────────────────────
# 主流程会话委托子流程Agent（物理独立 session）接管单个系列（RA/DR/Story/TestCase/Coding）。
# subprocessAgents[] 记录在 state.json 中，供主流程监管 + prompt_inject 角色感知注入。

import uuid as _uuid


def register_subprocess_agent(
    state: dict,
    *,
    series_type: str,
    entity_id: str,
    memory_entity_type: str = "",
    session_id: str = "",
    deadline: str = "",
) -> dict:
    """注册子流程Agent，返回新 agent 记录。

    Args:
        state: state.json dict（原地修改）
        series_type: ra/dr/story/testcase/coding
        entity_id: 业务实体 ID（如 STORY-001-BE）
        memory_entity_type: memory 实体类型（默认 = series_type）
        session_id: 物理独立 session ID
        deadline: 截止时间 ISO8601
    """
    if series_type not in ("ra", "dr", "story", "testcase", "coding"):
        raise ValueError(f"unknown series_type: {series_type}")
    agents = state.setdefault("subprocessAgents", [])
    agent_id = f"spa-{_uuid.uuid4().hex[:8]}"
    record = {
        "agentId": agent_id,
        "seriesType": series_type,
        "entityId": entity_id,
        "memoryEntityType": memory_entity_type or series_type,
        "memoryPath": f".ae-sdd/memory/{memory_entity_type or series_type}/{entity_id}/",
        "status": "running",
        "startedAt": _now_ts(),
        "deadline": deadline,
        "sessionId": session_id,
        "deliverables": [],
    }
    agents.append(record)
    state["lastUpdated"] = _now_ts()
    return record


def update_subprocess_agent(state: dict, agent_id: str, **updates) -> dict:
    """更新子流程Agent状态（如 status/deliverables/currentSeries）。

    返回更新后的 agent 记录。找不到则 raise KeyError。
    """
    agents = state.get("subprocessAgents", [])
    for agent in agents:
        if agent.get("agentId") == agent_id:
            agent.update(updates)
            agent["lastUpdated"] = _now_ts()
            state["lastUpdated"] = _now_ts()
            return agent
    raise KeyError(f"subprocessAgent not found: {agent_id}")


def collect_subprocess_agent(state: dict, agent_id: str, *, deliverables: list | None = None) -> dict:
    """子流程Agent完成回传：标记 completed + 记录交付物。

    返回更新后的 agent 记录。找不到则 raise KeyError。
    """
    agents = state.get("subprocessAgents", [])
    for agent in agents:
        if agent.get("agentId") == agent_id:
            agent["status"] = "completed"
            agent["completedAt"] = _now_ts()
            if deliverables:
                agent["deliverables"] = deliverables
            state["lastUpdated"] = _now_ts()
            return agent
    raise KeyError(f"subprocessAgent not found: {agent_id}")


def list_subprocess_agents(state: dict, *, status: str = "") -> list[dict]:
    """列出子流程Agent。status 非空时按状态过滤。"""
    agents = state.get("subprocessAgents", [])
    if status:
        return [a for a in agents if a.get("status") == status]
    return list(agents)


def get_active_subprocess_agent(state: dict) -> dict | None:
    """获取当前活跃（running）的子流程Agent。无则返回 None。"""
    for agent in state.get("subprocessAgents", []):
        if agent.get("status") == "running":
            return agent
    return None


# ─── 🆕 v3.10.3 compact-trigger 读端（补齐 state.py:974 写端）──────────────


def read_compact_trigger(project_root: Path) -> dict | None:
    """读 .ae-sdd/compact-trigger 文件。

    返回 {"prdId":..., "summaryPath":..., "triggeredAt":...} 或 None（不存在）。
    🆕 v3.10.3: 补齐 prd_complete() 写端的读端（之前只写不读）。
    """
    trigger_file = project_root / ".ae-sdd" / "compact-trigger"
    if not trigger_file.is_file():
        return None
    try:
        return json.loads(trigger_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def clear_compact_trigger(project_root: Path) -> bool:
    """清除 compact-trigger 文件（防重复触发）。返回是否清除成功。"""
    trigger_file = project_root / ".ae-sdd" / "compact-trigger"
    if trigger_file.is_file():
        trigger_file.unlink()
        return True
    return False


# ─── 🆕 v3.6 主流程监管器：paused 状态 + 矫正计数 API ─────────────────────────
# 配合 flow_monitor.py（偏移检测）和 prompt_inject.py（监管器主逻辑）使用。
# 设计：本节只提供状态读写 API，偏移判定逻辑在 flow_monitor.py（职责分离）。

def pause_state(state: dict, pause_reason: str, by: str = "ae-sdd") -> None:
    """暂停流程：记录当前 phase → pausedFromPhase，phase 置为 paused。

    幂等：已是 paused 时直接返回（不重复记录 history）。

    Args:
        state:        read_state() 返回的 dict（原地修改）
        pause_reason: 暂停原因，建议用以下常量：
                      "level3-escalation" — Level 3 矫正次数超限
                      "user-rejected"     — 用户拒绝本系列产物（审核点 ❌）
                      "user-manual"       — 用户手动暂停
        by:           操作者标识（写入 history.by）
    """
    if state.get("phase") == "paused":
        return  # 幂等：已暂停
    state["pausedFromPhase"] = state.get("phase", "initialized")
    state["pauseReason"] = pause_reason
    state["phase"] = "paused"
    record_history(state, f"paused({pause_reason})", by)


def resume_state(state: dict, by: str = "user") -> str:
    """从 paused 恢复：还原到 pausedFromPhase，清除 pause 相关字段。

    幂等：若当前不是 paused，直接返回当前 phase，不修改 state。

    Args:
        state: read_state() 返回的 dict（原地修改）
        by:    操作者标识（写入 history.by）

    Returns:
        恢复到的 phase 名称（即 pausedFromPhase；若未在 paused 则返回当前 phase）
    """
    if state.get("phase") != "paused":
        return state.get("phase", "initialized")
    resume_to = state.get("pausedFromPhase", "initialized")
    state["phase"] = resume_to
    state.pop("pausedFromPhase", None)
    state.pop("pauseReason", None)
    record_history(state, f"resumed-to-{resume_to}", by)
    return resume_to


def increment_correction(state: dict, phase: str) -> int:
    """矫正计数 +1，返回新计数。

    供 prompt_inject.py 在检测到偏移且决定注入矫正时调用。
    新计数由 flow_monitor.should_escalate() / DriftResult.correction_count 读取，
    决定是否升级到 Level 3。

    Args:
        state: read_state() 返回的 dict（原地修改）
        phase: 当前 phase 名称（用作 correctionCounts 的 key）

    Returns:
        递增后的新计数（int）
    """
    counts = state.setdefault("correctionCounts", {})
    counts[phase] = counts.get(phase, 0) + 1
    return counts[phase]


def get_correction_count(state: dict, phase: str) -> int:
    """读取指定 phase 的矫正次数（不存在返回 0）。

    供 flow_monitor.detect_drift() 读取历史次数，判断偏移 severity。
    """
    return state.get("correctionCounts", {}).get(phase, 0)


def reset_correction_count(state: dict, phase: str) -> None:
    """重置指定 phase 的矫正次数为 0。

    用于用户 ⚠️ 反馈后（带意见重跑 sub-step 2）：重置计数，重新开始矫正轮次。
    """
    state.setdefault("correctionCounts", {})[phase] = 0


# ─── 🆕 v3.9.0 嵌套状态模型（Nested State Model）──────────────────────────────
# 治本缺口：v3.8.x 前是扁平"每 WorkItem 一个 state.json"，导致：
#   1. 新任务不自动开 state，镜像死锁旧任务（STORY-002-BE/coding 污染 Story-003/004/005）
#   2. 无"主流程+子系列流程"嵌套，一个 PRD 下 N 个 Story 各自独立 state，无聚合
#   3. 命名单段 ID，无法表达顶层主体归属
#
# v3.9.0 嵌套模型（R1-R7，详见 CHANGELOG/2026-07-06-v3.9.0-nested-state-model.md）：
#   R1 单文件嵌套：一个 state.json 内含主流程所有子系列（prdState/drState/storyStates{N}）
#   R2 任意节点出发 + 向上归入：DR/Story 优先归入已存在的上层 state；entryNode 决定容器
#   R3 子状态容器：prdState/drState/storyStates{N}，按 entryNode 选填
#   R4 Bug/微任务不改 Story → 独立扁平 state（stateModel="flat"，保留 v1 行为）
#   R5 改已管理 Story → relocate 回所属 state + 只重置该 Story 子状态到 story-generated
#   R6 只以顶层主体命名：PRD-{特征} / DR-{特征} / Story-{特征}（多 Story 合并）
#   R7 /ae-sdd 路由时自动：分析需求特征 → 匹配现有 state → 找不到则以当前主体为顶层新建
#
# 与 v1 的关系：v1 扁平 schema 保留可读（stateModel="flat" 或缺省）；
#   v2 嵌套 schema 通过 stateModel="nested" 标识。所有读取点先判 stateModel 分流。

# stateModel 合法值
STATE_MODEL_NESTED = "nested"
STATE_MODEL_FLAT = "flat"
VALID_STATE_MODELS = (STATE_MODEL_NESTED, STATE_MODEL_FLAT)

# schema 版本号
SCHEMA_VERSION_V1 = "1"  # 扁平（v3.8.x 及之前）
SCHEMA_VERSION_V2 = "2"  # 嵌套（v3.9.0+）

# R5 重置目标 phase：改已管理 Story 时，该 Story 子状态重置到这里
# 含义：Story 系列重新出发（Story→TestCase→Task→Coding 链路重走）
STORY_RESET_TARGET_PHASE = "story-generated"

# entryNode → 应含的子状态容器名（R2/R3）
# entryNode=PRD → 含 prdState + drState + storyStates
# entryNode=DR  → 含 drState + storyStates（无 prdState，DR 是顶层）
# entryNode=STORY → 含 storyStates（无 prdState/drState，Story 是顶层）
ENTRY_NODE_CONTAINERS: dict[str, list[str]] = {
    "PRD":   ["prdState", "drStates", "storyStates"],
    "DR":    ["drState", "storyStates"],
    "STORY": ["storyStates"],
}


def is_nested_state(state: dict) -> bool:
    """判断 state 是否为 v3.9.0 嵌套模型。

    判定依据：stateModel == "nested"。
    旧 v1 state 无此字段或值为 "flat" → 返回 False。
    """
    return state.get("stateModel") == STATE_MODEL_NESTED


def init_nested_state(
    project_key: str,
    entry_node: str,
    state_machine_id: str,
    state_machine_name: str,
    story_ids: Optional[list[str]] = None,
    prd_id: Optional[str] = None,
    dr_id: Optional[str] = None,
    parent_prd_id: Optional[str] = None,
    parent_dr_id: Optional[str] = None,
    state_uuid: Optional[str] = None,
) -> dict:
    """初始化一个 v3.9.0 嵌套 state（不写盘，返回 dict 由调用方 write_state）。

    Args:
        project_key:        项目标识（如 "life"）
        entry_node:         顶层节点 PRD/DR/STORY（R2）
        state_machine_id:   state 业务标识（R6 纯业务名，如 "PRD-IM-CS"）
        state_machine_name: 可读名称
        story_ids:          初始 Story 列表（R3，每个建一条子状态记录）
        prd_id:             PRD 标识（entryNode=PRD 时必填）
        dr_id:              DR 标识（entryNode=PRD|DR 时必填）
        parent_prd_id:      溯源父 PRD（entryNode=DR/STORY 且已知上层 PRD）
        parent_dr_id:       溯源父 DR（entryNode=STORY 且已知上层 DR）
        state_uuid:         🆕 v3.10.1 随机 UUID（创建时生成）。传入则：
                            stateMachineId 拼为 ``{uuid}-{业务名}``，另写
                            stateMachineName=业务名、stateUuid=uuid 两个冗余字段，
                            供按业务名查找匹配。不传则保持旧行为（向后兼容）。

    Returns:
        v2 嵌套 state dict（含 version="2" / stateModel="nested" / 按 entry_node 选填容器）

    Raises:
        ValueError: entry_node 不在 ENTRY_NODE_CONTAINERS，或必填容器缺关键 ID
    """
    if entry_node not in ENTRY_NODE_CONTAINERS:
        raise ValueError(
            f"未知 entryNode: {entry_node}（允许: {list(ENTRY_NODE_CONTAINERS)}）"
        )

    now = _now_ts()
    # 🆕 v3.10.1：state_uuid 传入时 stateMachineId 带 UUID 前缀，stateMachineName 存纯业务名
    if state_uuid:
        full_state_machine_id = f"{state_uuid}-{state_machine_id}"
    else:
        full_state_machine_id = state_machine_id
    state: dict = {
        "version": SCHEMA_VERSION_V2,
        "projectKey": project_key,
        "stateModel": STATE_MODEL_NESTED,
        "processPolicy": "compact",
        "entryNode": entry_node,
        "stateMachineId": full_state_machine_id,
        "stateMachineName": state_machine_id if state_uuid else state_machine_name,
        "parentPrdId": parent_prd_id,
        "parentDrId": parent_dr_id,
        "activeStory": story_ids[0] if story_ids else None,
        "activeTask": None,
        "history": [],
        "events": [],
        "createdAt": now,
        "lastUpdated": now,
        "executionPlan": _default_execution_plan(),
        "review": _default_review_state(),
    }
    if state_uuid:
        state["stateUuid"] = state_uuid

    containers = ENTRY_NODE_CONTAINERS[entry_node]

    # R3：按 entryNode 选填子状态容器
    if "prdState" in containers:
        if not prd_id:
            raise ValueError("entryNode=PRD 必须提供 prd_id")
        state["prdState"] = {
            "prdId": prd_id,
            "phase": "initialized",
            "completedSteps": [],
            "lastUpdated": now,
        }

    if "drStates" in containers:
        state["drStates"] = {}

    if "drState" in containers:
        if not dr_id:
            raise ValueError(f"entryNode={entry_node} 必须提供 dr_id")
        state["drState"] = {
            "drId": dr_id,
            "phase": "initialized",
            "docPath": None,
            "completedSteps": [],
            "lastUpdated": now,
        }

    if "storyStates" in containers:
        state["storyStates"] = {}
        for sid in (story_ids or []):
            state["storyStates"][sid] = {
                "phase": "initialized",
                "completedSteps": [],
                "codingRound": 0,
                "lastUpdated": now,
                "resetHistory": [],
            }

    record_history(state, f"nested-state-init(entryNode={entry_node})", by="ae-sdd")
    return state


def _new_story_substate(initial_phase: str = "initialized") -> dict:
    now = _now_ts()
    return {
        "phase": initial_phase,
        "completedSteps": [],
        "codingRound": 0,
        "lastUpdated": now,
        "resetHistory": [],
    }


def _iter_nested_story_substates(state: dict, story_id: str) -> list[dict]:
    hits: list[dict] = []
    for dr_state in (state.get("drStates") or {}).values():
        if isinstance(dr_state, dict):
            sub = (dr_state.get("storyStates") or {}).get(story_id)
            if isinstance(sub, dict):
                hits.append(sub)
    sub = (state.get("storyStates") or {}).get(story_id)
    if isinstance(sub, dict) and all(sub is not h for h in hits):
        hits.append(sub)
    return hits


def ensure_dr_substate(state: dict, dr_id: str,
                       initial_phase: str = "initialized",
                       doc_path: Optional[str] = None) -> dict:
    """Ensure a DR child substate exists under a PRD-root nested state."""
    if not is_nested_state(state):
        raise ValueError("ensure_dr_substate requires nested state")
    if not dr_id:
        raise ValueError("dr_id is required")
    now = _now_ts()
    if state.get("prdState") is not None:
        dr_states = state.setdefault("drStates", {})
        dr_state = dr_states.get(dr_id)
        if not isinstance(dr_state, dict):
            dr_state = {
                "drId": dr_id,
                "phase": initial_phase,
                "docPath": doc_path,
                "completedSteps": [],
                "lastUpdated": now,
                "storyStates": {},
            }
            dr_states[dr_id] = dr_state
            record_history(state, f"dr-{dr_id}-added", by="ae-sdd")
        else:
            dr_state.setdefault("drId", dr_id)
            dr_state.setdefault("phase", initial_phase)
            dr_state.setdefault("completedSteps", [])
            dr_state.setdefault("storyStates", {})
            if doc_path and not dr_state.get("docPath"):
                dr_state["docPath"] = doc_path
            dr_state["lastUpdated"] = now
        return dr_state

    dr_state = state.setdefault("drState", {})
    dr_state.setdefault("drId", dr_id)
    dr_state.setdefault("phase", initial_phase)
    dr_state.setdefault("completedSteps", [])
    dr_state["lastUpdated"] = now
    return dr_state


def get_story_substate(state: dict, story_id: str) -> Optional[dict]:
    """读取嵌套 state 内指定 Story 的子状态记录。

    Args:
        state: 嵌套 state dict（若是 flat state 返回 None）
        story_id: Story 标识

    Returns:
        子状态 dict（含 phase/completedSteps/codingRound/lastUpdated/resetHistory）或 None
    """
    if not is_nested_state(state):
        return None
    hits = _iter_nested_story_substates(state, story_id)
    return hits[0] if hits else None


def get_story_document_binding(state: dict, story_id: str = "") -> dict[str, str]:
    """Return the native StoryName/document path binding for one Story."""
    target_story = (story_id or get_active_story(state)
                    or state.get("currentStory") or "").strip()
    if is_nested_state(state):
        sub = get_story_substate(state, target_story) if target_story else None
        return {
            "storyName": str((sub or {}).get("storyName") or ""),
            "docPath": str((sub or {}).get("docPath") or ""),
        }
    return {
        "storyName": str(state.get("storyName") or ""),
        "docPath": str(state.get("storyDocPath") or ""),
    }


def bind_story_document(
    state: dict,
    story_id: str,
    *,
    story_name: str,
    doc_path: str,
    by: str = "ae-sdd state bind-story-doc",
) -> bool:
    """Persist an exact StoryName/path binding without changing Story identity."""
    target_story = (story_id or "").strip()
    normalized_name = (story_name or "").strip()
    if normalized_name.lower().endswith(".md"):
        normalized_name = normalized_name[:-3]
    if (not target_story or not normalized_name or not (doc_path or "").strip()
            or "/" in normalized_name or "\\" in normalized_name
            or ".." in normalized_name):
        raise ValueError("story_id, basename-only story_name and doc_path are required")

    now = _now_ts()
    if is_nested_state(state):
        substates = _iter_nested_story_substates(state, target_story)
        if not substates:
            raise ValueError(f"Story {target_story} is not managed by this nested state")
        changed = False
        for sub in substates:
            if (sub.get("storyName") != normalized_name
                    or sub.get("docPath") != doc_path):
                sub["storyName"] = normalized_name
                sub["docPath"] = doc_path
                sub["lastUpdated"] = now
                changed = True
    else:
        current_story = str(state.get("currentStory") or "").strip()
        if current_story and current_story != target_story:
            raise ValueError(
                f"Story {target_story} does not match flat state currentStory {current_story}"
            )
        changed = (state.get("storyName") != normalized_name
                   or state.get("storyDocPath") != doc_path)
        if changed:
            state["storyName"] = normalized_name
            state["storyDocPath"] = doc_path
            state["lastUpdated"] = now

    if changed:
        record_history(state, f"story-{target_story}-document-bound", by=by)
        state["lastUpdated"] = now
    return changed


def set_story_substate_phase(state: dict, story_id: str, phase: str,
                              by: str = "ae-sdd") -> bool:
    """设置嵌套 state 内指定 Story 子状态的 phase（R5 各 Story 独立流转）。

    Args:
        state: 嵌套 state dict（原地修改）
        story_id: 目标 Story
        phase: 新 phase（须在该 Story 所属链路内，校验交给调用方）
        by: 操作者

    Returns:
        True=实际更新；False=phase 未变或 story_id 不存在

    Raises:
        ValueError: state 不是嵌套模型
    """
    if not is_nested_state(state):
        raise ValueError("set_story_substate_phase 仅适用于 nested state")
    subs = _iter_nested_story_substates(state, story_id)
    if not subs:
        return False
    now = _now_ts()
    phase_changed = False
    projection_changed = False
    for sub in subs:
        if sub.get("phase") != phase:
            sub["phase"] = phase
            phase_changed = True
        if _sync_phase_projection(sub, phase):
            projection_changed = True
        if phase_changed or projection_changed:
            sub["lastUpdated"] = now
        _validate_workflow_state_projection(sub, f"storyStates.{story_id}")
    changed = phase_changed or projection_changed
    if not changed:
        return False
    state["activeStory"] = story_id
    if phase_changed:
        record_history(state, f"story-{story_id}-phase={phase}", by)
    return True


def add_story_to_nested_state(state: dict, story_id: str,
                               initial_phase: str = "initialized",
                               parent_dr_id: Optional[str] = None) -> bool:
    """向嵌套 state 的 storyStates 新增一条 Story 子状态记录。

    用于 R7 归入场景：Story 被归入已存在的 PRD/DR state 时调用。

    Args:
        state: 嵌套 state dict（原地修改）
        story_id: Story 标识
        initial_phase: 初始 phase（默认 initialized）

    Returns:
        True=新增成功；False=已存在（幂等不覆盖）
    """
    if not is_nested_state(state):
        raise ValueError("add_story_to_nested_state 仅适用于 nested state")
    story_states = state.setdefault("storyStates", {})
    changed = False
    if story_id not in story_states:
        story_states[story_id] = _new_story_substate(initial_phase)
        changed = True
    if parent_dr_id:
        dr_state = ensure_dr_substate(state, parent_dr_id, initial_phase="dr-generated")
        dr_story_states = dr_state.setdefault("storyStates", {})
        if story_id not in dr_story_states:
            dr_story_states[story_id] = dict(story_states[story_id])
            changed = True
    if not state.get("activeStory"):
        state["activeStory"] = story_id
    if changed:
        record_history(state, f"story-{story_id}-added", by="ae-sdd")
    return changed


def reset_story_substate(state: dict, story_id: str,
                          by: str = "ae-sdd") -> bool:
    """R5：重置指定 Story 子状态到 STORY_RESET_TARGET_PHASE（story-generated）。

    只重置该 Story 的子状态，兄弟 Story 子状态不动。
    保留 resetHistory（追加一条重置记录），清空 completedSteps 与 codingRound。

    Args:
        state: 嵌套 state dict（原地修改）
        story_id: 要重置的 Story
        by: 操作者

    Returns:
        True=重置成功；False=story_id 不在 storyStates 内

    Raises:
        ValueError: state 不是嵌套模型
    """
    if not is_nested_state(state):
        raise ValueError("reset_story_substate 仅适用于 nested state")
    subs = _iter_nested_story_substates(state, story_id)
    if not subs:
        return False

    now = _now_ts()
    for sub in subs:
        old_phase = sub.get("phase", "initialized")
    # 追加重置历史（保留审计轨迹，不清空）
        sub.setdefault("resetHistory", []).append({
            "resetAt": now,
            "fromPhase": old_phase,
            "toPhase": STORY_RESET_TARGET_PHASE,
            "by": by,
        })
        sub["phase"] = STORY_RESET_TARGET_PHASE
        sub["completedSteps"] = []
        sub["codingRound"] = 0
        sub["lastUpdated"] = now
        # 🆕 v3.9.21 产物作废信号：通知下游（task-generate 等）强制全量重生成，
        # 不依赖 LLM 文本比对判定"跳过/更新"。一次性，由 consume_artifact_invalidation 消费清除。
        sub["artifactInvalidated"] = {
            "at": now,
            "by": by,
            "scopes": ["TASK", "TESTCASE", "CODING_PLAN"],
            "reason": "story-substate-reset",
        }
    state["activeStory"] = story_id
    record_history(state, f"story-{story_id}-reset-to-{STORY_RESET_TARGET_PHASE}", by)
    return True


def consume_artifact_invalidation(state: dict, story_id: str) -> Optional[dict]:
    """v3.9.21：读取并清除该 Story 的产物作废信号（一次性消费）。

    reset_story_substate 重置时会写入 artifactInvalidated，通知下游 skill
    （task-generate 等）强制全量重生成。本函数供 skill/gate 读取后调用，
    拿到记录即代表"该 Story 下游产物需强制全量重新生成"，消费后字段清零防累积。

    Args:
        state:    嵌套 state dict（原地修改）
        story_id: 目标 Story

    Returns:
        作废记录 dict（含 at/by/scopes/reason）或 None（无信号/非 nested/story 不存在）

    Note:
        调用方应在完成全量重生成后调用本函数。若调用方只读不消费，
        信号会持续保留——但 task-generate 多次全量重生成无副作用
        （save_doc 无条件覆盖），故"多读一次"是安全的，仅重复劳动。
    """
    if not is_nested_state(state):
        return None
    subs = _iter_nested_story_substates(state, story_id)
    if not subs:
        return None
    rec = None
    for sub in subs:
        inv = sub.get("artifactInvalidated")
        if inv:
            rec = inv
            sub["artifactInvalidated"] = None  # 消费即清除，防累积
    if rec:
        state["activeStory"] = story_id
        record_history(state, f"story-{story_id}-invalidation-consumed", "ae-sdd")
    return rec


def set_active_story(state: dict, story_id: str) -> bool:
    """切换嵌套 state 的 activeStory 指针（路由切换当前聚焦 Story）。

    Args:
        state: 嵌套 state dict（原地修改）
        story_id: 要聚焦的 Story（必须在 storyStates 内）

    Returns:
        True=切换成功；False=story_id 不在 storyStates 内
    """
    if not is_nested_state(state):
        raise ValueError("set_active_story 仅适用于 nested state")
    if not get_story_substate(state, story_id):
        return False
    state["activeStory"] = story_id
    state["lastUpdated"] = _now_ts()
    return True


def get_active_phase(state: dict) -> str:
    """获取当前活跃 phase（兼容 v1 flat / v2 nested）。

    - nested state：返回 activeStory 子状态的 phase（若无 activeStory 返回 prdState/drState phase）
    - flat state：返回顶层 phase

    供 prompt_inject / gate_intercept 等需要"当前 phase"的 hook 统一调用。
    """
    if is_nested_state(state):
        active_story = state.get("activeStory")
        if active_story:
            sub = get_story_substate(state, active_story)
            if sub:
                return sub.get("phase", "initialized")
        # 无 activeStory，回退到 prdState/drState phase
        if state.get("prdState"):
            return state["prdState"].get("phase", "initialized")
        if state.get("drState"):
            return state["drState"].get("phase", "initialized")
        return "initialized"
    return state.get("phase", "initialized")


def get_active_story(state: dict) -> Optional[str]:
    """获取当前 activeStory（nested）或 currentStory（flat），统一接口。"""
    if is_nested_state(state):
        return state.get("activeStory")
    return state.get("currentStory")


def is_work_item_completed(state: dict) -> bool:
    """判断整条 work-item 是否已全部完结（兼容 v1 flat / v2 nested）。

    nested state 下 get_active_phase() 只反映 activeStory 指向的那一个 Story；
    某 Story 完成后 activeStory 不会自动前移到下一个未完成 Story，导致
    get_active_phase()=="completed" 时其余 Story 仍可能未完成。这里改为聚合
    _iter_story_projection_records() 遍历到的全部 Story 子状态，仅当全部
    completed 才判定整条 work-item 完结；无任何 Story 子状态时回退到
    get_active_phase()（对应仅有 prdState/drState、尚未拆出 Story 的场景）。
    """
    if not is_nested_state(state):
        return state.get("phase") == "completed"
    records = list(_iter_story_projection_records(state))
    if records:
        return all(sub.get("phase") == "completed" for _, sub in records)
    return get_active_phase(state) == "completed"


def list_story_ids_in_state(state: dict) -> list[str]:
    """列出 state 内所有 Story ID（nested 返回 storyStates 键，flat 返回 currentStory 单值列表）。

    供 match_state 扫描匹配用。
    """
    if is_nested_state(state):
        ids = set((state.get("storyStates") or {}).keys())
        for dr_state in (state.get("drStates") or {}).values():
            if isinstance(dr_state, dict):
                ids.update((dr_state.get("storyStates") or {}).keys())
        return sorted(ids)
    cs = state.get("currentStory")
    return [cs] if cs else []


# ─── 🆕 v3.9.3 R2 强制向上归入（递归算法）────────────────────────────────
# 用户定义的递归算法（CHANGELOG 2026-07-07）：
#   1. 读当前节点文档 → extract_parent_claim 抽父级声明
#   2. verify_parent_claim 验证父级文档存在 + 关联性
#   3. 无父级 / 父级文档找不到 → 视为无父级，当前层为顶层
#   4. 有父级：
#      a) 父级已有 state（find_nested_state_by_* 命中）→ 嵌进对应容器
#      b) 父级无 state → 递归：先替父级创建 state，再嵌
#   5. 这是从叶子向上"补全缺失祖先"的递归过程


def _ensure_parent_nested_state(ade_sdd: Path, parent_type: str, parent_id: str,
                                design_dir: Path,
                                generate_uuid: bool = False) -> tuple[Optional[Path], Optional[dict]]:
    """🆕 v3.9.3 内部辅助：确保父级 state 存在，必要时递归创建。

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        parent_type: "PRD" 或 "DR"
        parent_id: 父级 ID（如 "DR-005" / "PRD-001"）
        design_dir: design/ 目录（用于递归时验证父级的父级）
        generate_uuid: 🆕 v3.10.1 是否为新建父级 state 生成 UUID 前缀（透传自调用方）

    Returns:
        (state_path, state_data) - 父级 state；或 (None, None) 当无法创建
    """
    from lib import paths as paths_mod  # 避免循环

    if parent_type == "PRD":
        hit = paths_mod.find_nested_state_by_prd_id(ade_sdd, parent_id)
        if hit:
            return hit
        # 父级 PRD 无 state -> 替它创建（🆕 v3.10.1 带 UUID 前缀）
        try:
            prd_feature = parent_id.replace("PRD-", "", 1)
            new_uuid = paths_mod.generate_state_uuid() if generate_uuid else None
            biz_name = paths_mod.build_state_machine_name("PRD", {"prd_feature": prd_feature})
            kwargs = dict(
                project_key="",
                entry_node="PRD",
                state_machine_id=biz_name,
                state_machine_name=parent_id,
                story_ids=None,
                prd_id=parent_id,
            )
            if new_uuid:
                kwargs["state_uuid"] = new_uuid
            st = init_nested_state(**kwargs)
            sp = paths_mod.work_item_state_path(ade_sdd, "PRD",
                                                {"prd_feature": prd_feature},
                                                state_uuid=new_uuid)
            write_state(sp, st)
            return (sp, st)
        except Exception:
            return (None, None)
    elif parent_type == "DR":
        hit = paths_mod.find_nested_state_by_dr_id(ade_sdd, parent_id)
        if hit:
            return hit
        # 父级 DR 无 state -> 检查 DR 有没有 PRD 父级，递归
        dr_doc = paths_mod._find_design_doc(design_dir, parent_id)
        prd_parent = None
        if dr_doc:
            prd_parent, _ = paths_mod.extract_parent_claim(dr_doc, doc_kind="dr")
        # 先确保 PRD 父级 state 存在
        if prd_parent:
            ok, _ = paths_mod.verify_parent_claim("PRD", prd_parent, design_dir, child_id=parent_id)
            if ok:
                prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", prd_parent, design_dir,
                                                      generate_uuid=generate_uuid)
                if prd_hit and prd_hit[0] is not None:
                    prd_sp, prd_st = prd_hit
                    ensure_dr_substate(
                        prd_st,
                        parent_id,
                        initial_phase="dr-generated",
                        doc_path=str(dr_doc) if dr_doc else None,
                    )
                    write_state(prd_sp, prd_st)
                    return (prd_sp, prd_st)
                # PRD 父级 state 准备好后，回到 DR 创建并嵌进 PRD 的 drState
        # 创建 DR 顶层 state（即使有 PRD 父级，DR 仍可独立创建顶层 state，
        #   然后下面 R2 吸收逻辑会把它嵌进 PRD）（🆕 v3.10.1 带 UUID 前缀）
        try:
            dr_feature = parent_id.replace("DR-", "", 1)
            new_uuid = paths_mod.generate_state_uuid() if generate_uuid else None
            biz_name = paths_mod.build_state_machine_name("DR", {"dr_feature": dr_feature})
            kwargs = dict(
                project_key="",
                entry_node="DR",
                state_machine_id=biz_name,
                state_machine_name=parent_id,
                story_ids=None,
                dr_id=parent_id,
            )
            if new_uuid:
                kwargs["state_uuid"] = new_uuid
            st = init_nested_state(**kwargs)
            sp = paths_mod.work_item_state_path(ade_sdd, "DR",
                                                {"dr_feature": dr_feature},
                                                state_uuid=new_uuid)
            write_state(sp, st)
            return (sp, st)
        except Exception:
            return (None, None)
    return (None, None)


def recursive_r2_absorb(ade_sdd: Path, top_node: str, features: dict,
                        design_dir: Path,
                        doc_path: Optional[Path] = None,
                        child_id: str = "",
                        generate_uuid: bool = False) -> tuple[Path, dict]:
    """🆕 v3.9.3 递归向上归入（用户定义的 R2 算法）。

    1. extract_parent_claim 读当前节点文档 -> 抽父级声明
    2. verify_parent_claim 验证父级文档存在 + 关联性
    3. 无父级 / 父级文档找不到 -> 当前层为顶层
    4. 有父级：
       a) 父级已有 state -> 把当前节点加入该 state 的对应容器
       b) 父级无 state -> 递归：先 _ensure_parent_nested_state 替父级建
          -> 把当前节点嵌进新建的父级 state

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        top_node: 当前节点类型 PRD/DR/STORY/TASK
        features: 当前节点 R6 特征
        design_dir: design/ 目录
        doc_path: 当前节点文档（Story/DR 文档路径），用于抽父级
        child_id: 当前节点 ID（用于关联性验证）
        generate_uuid: 🆕 v3.10.1 是否为新建顶层 state 生成 UUID 前缀。
                       True=cmd_state_new 入口（用户创建），生成 UUID；
                       False=默认（测试/内部调用），保持旧行为无 UUID。

    Returns:
        (state_path, state_data) - 当前节点最终所属嵌套 state
    """
    from lib import paths as paths_mod  # 避免循环

    top_node = (top_node or "").upper()
    if top_node not in ("PRD", "DR", "STORY", "TASK"):
        # 非法顶层 -> 当作无父级 STORY 处理
        top_node = "STORY"

    # 1) 读当前节点文档抽父级
    parent_prd: Optional[str] = None
    parent_dr: Optional[str] = None
    if doc_path and doc_path.is_file():
        if top_node == "STORY":
            parent_prd, parent_dr = paths_mod.extract_parent_claim(doc_path, doc_kind="story")
        elif top_node == "DR":
            parent_prd, _ = paths_mod.extract_parent_claim(doc_path, doc_kind="dr")

    # 2) 验证父级
    valid_parent_dr: Optional[str] = None
    valid_parent_prd: Optional[str] = None
    if parent_dr:
        ok, reason = paths_mod.verify_parent_claim("DR", parent_dr, design_dir, child_id=child_id)
        if ok:
            valid_parent_dr = parent_dr
        # reason=doc_not_found / relation_mismatch -> 视为无父级（不阻塞）
    if parent_prd:
        ok, reason = paths_mod.verify_parent_claim("PRD", parent_prd, design_dir, child_id=child_id)
        if ok:
            valid_parent_prd = parent_prd

    # 3) 无有效父级 -> 当前层为顶层
    if not valid_parent_dr and not valid_parent_prd:
        from lib import paths as _p
        new_uuid = _p.generate_state_uuid() if generate_uuid else None
        sp = _p.work_item_state_path(ade_sdd, top_node, features, state_uuid=new_uuid)
        # 🆕 v3.10.1：state_machine_id 用纯业务名（build_state_machine_name），
        #   不再用 sp.parent.name（可能含 UUID 前缀）；UUID 由 state_uuid 参数注入
        biz_name = _p.build_state_machine_name(top_node, features)
        try:
            kwargs = dict(
                project_key="",
                entry_node=top_node,
                state_machine_id=biz_name,
                state_machine_name=biz_name,
            )
            if top_node == "PRD":
                kwargs["prd_id"] = features.get("prd_id") or child_id
            elif top_node == "DR":
                kwargs["dr_id"] = features.get("dr_id") or features.get("dr_feature") or child_id
            elif top_node == "STORY":
                kwargs["story_ids"] = features.get("story_ids")
            if new_uuid:
                kwargs["state_uuid"] = new_uuid
            st = init_nested_state(**kwargs)
            write_state(sp, st)
            return (sp, st)
        except Exception:
            # 已存在 -> 直接读
            if sp.is_file():
                return (sp, read_state(sp))
            raise

    # 4a) Story → 嵌进父级 DR
    if top_node == "STORY" and valid_parent_dr:
        story_id = (features.get("story_ids") or [child_id])[0]
        dr_doc = paths_mod._find_design_doc(design_dir, valid_parent_dr)
        dr_parent_prd = valid_parent_prd
        if dr_doc:
            claimed_prd, _ = paths_mod.extract_parent_claim(dr_doc, doc_kind="dr")
            if claimed_prd:
                ok, _ = paths_mod.verify_parent_claim("PRD", claimed_prd, design_dir, child_id=valid_parent_dr)
                if ok:
                    dr_parent_prd = claimed_prd
        if dr_parent_prd:
            prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", dr_parent_prd, design_dir,
                                                  generate_uuid=generate_uuid)
            if prd_hit and prd_hit[0] is not None:
                prd_sp, prd_st = prd_hit
                ensure_dr_substate(
                    prd_st,
                    valid_parent_dr,
                    initial_phase="dr-generated",
                    doc_path=str(dr_doc) if dr_doc else None,
                )
                add_story_to_nested_state(
                    prd_st,
                    story_id,
                    initial_phase="story-generated",
                    parent_dr_id=valid_parent_dr,
                )
                write_state(prd_sp, prd_st)
                return (prd_sp, prd_st)
        dr_hit = _ensure_parent_nested_state(ade_sdd, "DR", valid_parent_dr, design_dir,
                                             generate_uuid=generate_uuid)
        if dr_hit and dr_hit[0] is not None:
            dr_sp, dr_st = dr_hit
            # 嵌进 DR state 的 storyStates
            story_id = (features.get("story_ids") or [child_id])[0]
            add_story_to_nested_state(dr_st, story_id, initial_phase="story-generated")
            write_state(dr_sp, dr_st)
            return (dr_sp, dr_st)

    # 4b) DR → 嵌进父级 PRD
    if top_node == "DR" and valid_parent_prd:
        prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", valid_parent_prd, design_dir,
                                              generate_uuid=generate_uuid)
        if prd_hit and prd_hit[0] is not None:
            prd_sp, prd_st = prd_hit
            # 嵌进 PRD state 的 drState
            dr_id = features.get("dr_id") or child_id
            ensure_dr_substate(prd_st, dr_id, initial_phase="dr-generated")
            write_state(prd_sp, prd_st)
            return (prd_sp, prd_st)

    # 4c) Story 有 PRD 但无 DR → 嵌进 PRD（罕见，PRD 直接管 Story）
    if top_node == "STORY" and valid_parent_prd and not valid_parent_dr:
        prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", valid_parent_prd, design_dir,
                                              generate_uuid=generate_uuid)
        if prd_hit and prd_hit[0] is not None:
            prd_sp, prd_st = prd_hit
            story_id = (features.get("story_ids") or [child_id])[0]
            add_story_to_nested_state(prd_st, story_id, initial_phase="story-generated")
            write_state(prd_sp, prd_st)
            return (prd_sp, prd_st)

    # 5) 兜底：父级 state 创建失败 → 当前层为顶层
    from lib import paths as _p
    new_uuid = _p.generate_state_uuid() if generate_uuid else None
    sp = _p.work_item_state_path(ade_sdd, top_node, features, state_uuid=new_uuid)
    if sp.is_file():
        return (sp, read_state(sp))
    try:
        biz_name = _p.build_state_machine_name(top_node, features)
        kwargs = dict(
            project_key="",
            entry_node=top_node,
            state_machine_id=biz_name,
            state_machine_name=biz_name,
            story_ids=features.get("story_ids") if top_node == "STORY" else None,
        )
        if new_uuid:
            kwargs["state_uuid"] = new_uuid
        st = init_nested_state(**kwargs)
        write_state(sp, st)
        return (sp, st)
    except Exception:
        # 已存在则读
        return (sp, read_state(sp) if sp.is_file() else {})
