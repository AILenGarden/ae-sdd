"""
state.py — ae-sdd 项目状态管理

state.json 结构（v2）：

Story/Task/Plan 级（txn 级）：
{
  "version": "1",
  "projectKey": "...",
  "phase": "initialized" | "dr-generated" | ...,
  "scale": "大"|"中"|"小"|"微",   # 🆕 v3.5.15 任务规模，决定走哪条子链；旧 state 缺失则 _infer_scale 反推
  "entryNode": "BUG"|"CONFIG"|"PRD"|"RA"|... | null,  # 🆕 v3.5.15 入口节点语义（FlowNode.value）
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

🆕 v3.5.15 多入口状态机：4 子链 + scale 路由（详见 PHASE_FLOWS 注释）
  - 大链（11 phase）：initialized→ra→dr→story→...→completed
  - 中链（10 phase）：跳过 DR
  - 小链（8 phase）：跳过 DR/Story
  - 微链（4 phase）：initialized→coding→test-running→completed（BUG/配置类复用）
  - 旧 state 无 scale → _infer_scale 按 completedSteps/phase 反推，默认"大"（最保守）

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
    "大": [   # 重任务：PRD/中大需求 → RA → DR → Story → Task → CodingProcess → Coding 全主干
        "initialized", "ra-generated", "dr-generated", "story-generated", "story-reviewed",
        "task-generated", "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "中": [   # 中任务：跳过 DR，RA → Story → Task → CodingProcess → Coding
        "initialized", "ra-generated", "story-generated", "story-reviewed",
        "task-generated", "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "小": [   # 小任务：跳过 DR/Story，RA → Task → CodingProcess → Coding
        "initialized", "ra-generated", "task-generated", "task-reviewed",
        "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "微": [   # 微任务/BUG/配置类：跳过 RA/DR/Story/Task，CodingProcess（轻量）→ Coding
        "initialized", "coding-process", "coding", "test-running", "completed",
    ],
}

# 合法 scale 集合（与 classify.py SCALE 值一致）
VALID_SCALES = ("大", "中", "小", "微")

# 向后兼容别名：旧代码/测试引用 PHASE_FLOW 时仍可用，等价于大链（最保守主干）。
# 🟡 deprecated：新代码应改用 PHASE_FLOWS[scale]。未来版本删除。
PHASE_FLOW = PHASE_FLOWS["大"]


def read_state(state_path: Path) -> dict:
    """读 state.json，不存在则返回空模板"""
    if not state_path.is_file():
        return {
            "version": "1",
            "projectKey": None,
            "phase": "initialized",
            "scale": None,        # 🆕 v3.5.15 任务规模（大/中/小/微），首次 state write 写入
            "entryNode": None,    # 🆕 v3.5.15 入口节点语义（FlowNode.value，如 BUG/CONFIG/PRD）
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


def _infer_scale(state: dict) -> tuple[str, float, str]:
    """🆕 v3.5.15 旧 state 兼容：无 scale 字段时按 completedSteps/phase 反推规模。

    推断优先级（任一命中即定）：
      1. completedSteps 含 dr/story → 大（走完整主干）
      2. completedSteps 含 story 但无 dr → 中
      3. completedSteps 含 task 但无 story → 小
      4. phase ∈ {coding,test-running,code-reviewed,completed} 且 completedSteps 无 ra → 微
      5. 无法判定 → 默认"大"（最保守，含全主干，避免误放行跳 RA）

    Returns:
        (scale, confidence, reason)；confidence<0.5 时调用方应 warn 提示用户显式 --scale
    """
    completed = state.get("completedSteps") or []
    completed_text = " ".join(completed)
    phase = state.get("phase", "initialized")

    has_dr = any("dr" in (s or "").lower() for s in completed)
    has_story = any("story" in (s or "").lower() for s in completed)
    has_task = any("task" in (s or "").lower() for s in completed)
    has_ra = any("ra" in (s or "").lower() for s in completed) or "ra-generated" == phase

    if has_dr:
        return ("大", 0.9, "completedSteps 含 dr → 大（完整主干）")
    if has_story:
        return ("中", 0.85, "completedSteps 含 story 但无 dr → 中")
    if has_task:
        return ("小", 0.8, "completedSteps 含 task 但无 story → 小")
    # 🟡 v3.5.15 安全策略：phase=coding/test-running/code-reviewed 时无法可靠区分
    #   微链（直跳 coding）vs 大链（走完 task-reviewed 进 coding）——仅凭 phase 会误判。
    #   故 coding 阶段不推微，默认大（最保守，要求用户显式 --scale 才走微链）。
    #   微任务应在首次 state write 时显式带 --scale=微，不靠反推。
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


def set_phase(state: dict, phase: str, by: str = "ae-sdd") -> bool:
    """
    设置当前 phase + 记录历史（🆕 v3.5.15 按 state.scale 选子链校验）。

    Returns:
        True: phase 实际被更新
        False: phase 等于当前值，不重复记录

    Raises:
        ValueError: phase 不在该 state 所在子链中
    """
    scale = _resolve_scale(state)
    chain = PHASE_FLOWS[scale]
    if phase not in chain:
        raise ValueError(
            f"未知 phase: {phase}（scale={scale} 子链允许: {chain}）。"
            f"若切换规模请先 set_scale；全主干参考: {PHASE_FLOWS['大']}"
        )
    if state.get("phase") == phase:
        # 重复写：跳过 history 累积
        return False
    state["phase"] = phase
    record_history(state, phase, by)
    return True


# ─── 🆕 v3.5.15 各子链 next_step mapping ─────────────────────────────────────
# key = scale；每条子链独立 mapping，next 与 PHASE_FLOWS[scale] 一致。
_NEXT_STEP_MAPPINGS: dict[str, dict[str, tuple[str, str, str]]] = {
    "大": {
        "initialized":     ("ra-generated",     "跑需求分析（RA）+ G-RA 门卫",            "requirement-analysis-skill.md"),
        "ra-generated":    ("dr-generated",     "生成 DR（Design Requirement）",          "dr-generate-skill.md"),
        "dr-generated":    ("story-generated",  "生成 Story（从 DR）",                    "story-generate-skill.md"),
        "story-generated": ("story-reviewed",   "执行 Story Review（含 F-Stage 前端契约）", "story-review-skill.md"),
        "story-reviewed":  ("task-generated",   "生成 Task",                              "testcase-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（加载5上下文+调CodingSkill做CodeAnalysis+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "跑测试 + 出具测试报告",                  "coding-skill.md"),
        "test-running":    ("code-reviewed",    "出具 Coding 报告 + CodeReview",           "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
    "中": {
        "initialized":     ("ra-generated",     "跑需求分析（RA）+ G-RA 门卫",            "requirement-analysis-skill.md"),
        "ra-generated":    ("story-generated",  "生成 Story（从 RA，跳过 DR）",            "story-generate-skill.md"),
        "story-generated": ("story-reviewed",   "执行 Story Review（含 F-Stage 前端契约）", "story-review-skill.md"),
        "story-reviewed":  ("task-generated",   "生成 Task",                              "testcase-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（加载5上下文+调CodingSkill做CodeAnalysis+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "跑测试 + 出具测试报告",                  "coding-skill.md"),
        "test-running":    ("code-reviewed",    "出具 Coding 报告 + CodeReview",           "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
    "小": {
        "initialized":     ("ra-generated",     "跑需求分析（RA）+ G-RA 门卫（小任务轻量 RA）", "requirement-analysis-skill.md"),
        "ra-generated":    ("task-generated",   "生成 Task（跳过 DR/Story）",              "task-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（加载5上下文+调CodingSkill做CodeAnalysis+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "跑测试 + 出具测试报告",                  "coding-skill.md"),
        "test-running":    ("code-reviewed",    "出具 Coding 报告 + CodeReview",           "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
    "微": {
        "initialized":     ("coding-process",   "执行 CodingProcess（微任务轻量：加载上下文+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "跑测试 + 出具测试报告",                  "coding-skill.md"),
        "test-running":    ("code-reviewed",    "出具 Coding 报告 + CodeReview",           "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
}


def next_step_suggestion(state: dict) -> dict:
    """
    根据当前 phase 给出下一步建议（🆕 v3.5.15 按 state.scale 选子链 mapping）。

    返回 {"current": phase, "next": ..., "action": ..., "skill": "..."}
    - "current": 当前 phase
    - "next": 下一步要写入的 phase（与 PHASE_FLOWS[scale] 一致，可直接传给 state write --phase）
    - "action": 建议执行的动作（动词）
    - "skill": 对应的 SKILL 文件
    """
    cur = state.get("phase", "initialized")
    scale = _resolve_scale(state)
    mapping = _NEXT_STEP_MAPPINGS[scale]
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


# ─── 🆕 v3.5.12 重入字段写入 helper（治 P1-5 死字段）──────────────────────────
# SKILL.md §流程状态跟踪承诺：currentStep/completedSteps/codingRound 真实读写。
# v3.5.12 前零写入（重入只能靠 phase 粗粒度恢复）。

def set_current_step(state: dict, step: str) -> None:
    """进入新步骤时写 currentStep + 追加 completedSteps。"""
    prev = state.get("currentStep")
    if prev and prev != step:
        completed = state.setdefault("completedSteps", [])
        if prev not in completed:
            completed.append(prev)
    state["currentStep"] = step


def bump_coding_round(state: dict) -> str:
    """开始新一轮 Coding 前累加 codingRound，返回新轮次标识（如 r1/r2/r3）。"""
    cur = state.get("codingRound", 0)
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
