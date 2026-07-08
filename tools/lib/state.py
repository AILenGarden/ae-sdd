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
  - 大链（14 phase）：已有PRD，走全流程 initialized→ra-generated→dr-generated→...→completed
  - 中链（13 phase）：已有DR，跳RA，从DR系列入
  - 小链（12 phase）：已有Story，跳RA+DR，从Story系列入
  - 微链（8 phase）：BUG/调整，从Task系列入（含轻量CodingPlan），跳RA+DR+Story+TestCase；
    🆕 2026-07-03(B1)：加回 code-reviewed，与设计文档"CodeReview 报告不豁免"对齐
  - 🆕 v3.7.0：大/中/小链新增 testcase-generated→testcase-reviewed（TestCase 独立系列，
    story-reviewed 之后、task-generated 之前；微链不受影响，仍跳过整个 TestCase 系列）
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
    "大": [   # 大任务：已有 PRD，走全流程 RA→DR→Story→TestCase→Task→CodingProcess→Coding
        "initialized", "ra-generated", "dr-generated", "story-generated", "story-reviewed",
        "testcase-generated", "testcase-reviewed",
        "task-generated", "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "中": [   # 中任务：已有 DR，跳 RA，从 DR 系列入
        "initialized", "dr-generated", "story-generated", "story-reviewed",
        "testcase-generated", "testcase-reviewed",
        "task-generated", "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "小": [   # 小任务：已有 Story，跳 RA+DR，从 Story 系列入
        "initialized", "story-generated", "story-reviewed",
        "testcase-generated", "testcase-reviewed",
        "task-generated", "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
    ],
    "微": [   # 微任务/BUG/配置类：从 Task 系列入（含轻量 CodingPlan），跳 RA+DR+Story+TestCase
        # 🆕 2026-07-03 一致性修复（B1）：微链加回 code-reviewed phase。
        # 设计文档 conventions.md §3.1 明确"出 CodeReview 报告 ❌不豁免"，
        # 此前微链序列 (...→test-running→completed) 物理跳过 code-reviewed，
        # 导致 gate_intercept 微链 code-reviewed 门禁(G-09/G-CODE-1)声明却不可达（死代码）。
        # 现对齐设计：微任务也必须出 CodeReview 报告，与 coding-process-skill.full.md:180
        # "微任务 CodingPlan 全流程" 一致。
        "initialized", "task-generated", "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
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
            "scale": None,            # 🆕 v3.5.15 任务规模（大/中/小/微），首次 state write 写入
            "entryNode": None,        # 🆕 v3.5.15 入口节点语义（FlowNode.value，如 BUG/CONFIG/PRD）
            "pausedFromPhase": None,  # 🆕 v3.6 paused 前的 phase（resume 时恢复目标）
            "pauseReason": None,      # 🆕 v3.6 暂停原因（level3-escalation|user-rejected|user-manual）
            "correctionCounts": {},   # 🆕 v3.6 各 phase 矫正次数，键=phase 值=int
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
    # 🟡 v3.5.15 安全策略：phase=task-generated/coding 等阶段无法可靠区分
    #   微链（BUG/调整，Task系列入）vs 小链（已有Story，Story系列入）——has_task 对两者均命中。
    #   故 task/coding 阶段反推"小"，置信度 0.8（会告警），要求用户显式 --scale。
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

    🆕 v3.6：`paused` 是元状态，不在 PHASE_FLOWS 子链中，任何 phase 均可跳入。
      - 设置 paused 时自动保存 pausedFromPhase（当前 phase）
      - 从 paused 恢复请用 resume_state()，不要直接 set_phase

    Returns:
        True: phase 实际被更新
        False: phase 等于当前值，不重复记录

    Raises:
        ValueError: phase 不在该 state 所在子链中（paused 除外）
    """
    # 🆕 v3.6 paused 元状态：绕过子链校验，直接写入
    if phase == "paused":
        if state.get("phase") == "paused":
            return False  # 已经是 paused，幂等
        state["pausedFromPhase"] = state.get("phase", "initialized")
        state["phase"] = "paused"
        record_history(state, "paused", by)
        return True

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
        "story-reviewed":  ("testcase-generated", "生成测试用例",                         "testcase-generate-skill.md"),
        "testcase-generated": ("testcase-reviewed", "执行 TestCase Review（TC-1~TC-9）",  "testcase-review-skill.md"),
        "testcase-reviewed": ("task-generated", "生成 Task",                              "task-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（加载5上下文+调CodingSkill做CodeAnalysis+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行 Test 系列（test-generate→test-review，出具并复核测试报告）", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "Test Review 通过后出具 Coding 报告 + CodeReview", "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
    "中": {
        "initialized":     ("dr-generated",     "生成 DR（已有 DR，从 DR 系列入，跳 RA）",  "dr-generate-skill.md"),
        "dr-generated":    ("story-generated",  "生成 Story（从 DR）",                    "story-generate-skill.md"),
        "story-generated": ("story-reviewed",   "执行 Story Review（含 F-Stage 前端契约）", "story-review-skill.md"),
        "story-reviewed":  ("testcase-generated", "生成测试用例",                         "testcase-generate-skill.md"),
        "testcase-generated": ("testcase-reviewed", "执行 TestCase Review（TC-1~TC-9）",  "testcase-review-skill.md"),
        "testcase-reviewed": ("task-generated", "生成 Task",                              "task-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（加载5上下文+调CodingSkill做CodeAnalysis+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行 Test 系列（test-generate→test-review，出具并复核测试报告）", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "Test Review 通过后出具 Coding 报告 + CodeReview", "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
    "小": {
        "initialized":     ("story-generated",  "生成 Story（已有 Story，从 Story 系列入，跳 RA+DR）", "story-generate-skill.md"),
        "story-generated": ("story-reviewed",   "执行 Story Review（含 F-Stage 前端契约）", "story-review-skill.md"),
        "story-reviewed":  ("testcase-generated", "生成测试用例",                         "testcase-generate-skill.md"),
        "testcase-generated": ("testcase-reviewed", "执行 TestCase Review（TC-1~TC-9）",  "testcase-review-skill.md"),
        "testcase-reviewed": ("task-generated", "生成 Task",                              "task-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（加载5上下文+调CodingSkill做CodeAnalysis+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行 Test 系列（test-generate→test-review，出具并复核测试报告）", "test-generate-skill.md"),
        "test-running":    ("code-reviewed",    "Test Review 通过后出具 Coding 报告 + CodeReview", "coding-report-skill.md"),
        "code-reviewed":   ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
    },
    "微": {
        "initialized":     ("task-generated",   "生成 Task（BUG/调整，从 Task 系列入，跳 RA+DR+Story）", "task-generate-skill.md"),
        "task-generated":  ("task-reviewed",    "执行 Task Review",                       "task-generate-skill.md"),
        "task-reviewed":   ("coding-process",   "执行 CodingProcess（微任务轻量：加载上下文+出CodePlan）", "coding-process-skill.md"),
        "coding-process":  ("coding",           "执行 CodingSkill（按 CodePlan 编码）",   "coding-skill.md"),
        "coding":          ("test-running",     "执行 Test 系列（test-generate→test-review，出具并复核测试报告）", "test-generate-skill.md"),
        "test-running":    ("completed",        "等待用户最终确认 → completed",            "（人工审核）"),
        "completed":       ("（已结束）",        "项目工程已完成",                          "—"),
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
        stall_reason: 未通过时的原因（3 轮未决等）
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
    "PRD":   ["prdState", "drState", "storyStates"],
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
) -> dict:
    """初始化一个 v3.9.0 嵌套 state（不写盘，返回 dict 由调用方 write_state）。

    Args:
        project_key:        项目标识（如 "life"）
        entry_node:         顶层节点 PRD/DR/STORY（R2）
        state_machine_id:   state 标识（R6 只以顶层命名，如 "PRD-IM-CS"）
        state_machine_name: 可读名称
        story_ids:          初始 Story 列表（R3，每个建一条子状态记录）
        prd_id:             PRD 标识（entryNode=PRD 时必填）
        dr_id:              DR 标识（entryNode=PRD|DR 时必填）
        parent_prd_id:      溯源父 PRD（entryNode=DR/STORY 且已知上层 PRD）
        parent_dr_id:       溯源父 DR（entryNode=STORY 且已知上层 DR）

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
    state: dict = {
        "version": SCHEMA_VERSION_V2,
        "projectKey": project_key,
        "stateModel": STATE_MODEL_NESTED,
        "entryNode": entry_node,
        "stateMachineId": state_machine_id,
        "stateMachineName": state_machine_name,
        "parentPrdId": parent_prd_id,
        "parentDrId": parent_dr_id,
        "activeStory": story_ids[0] if story_ids else None,
        "activeTask": None,
        "history": [],
        "events": [],
        "createdAt": now,
        "lastUpdated": now,
    }

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
    return (state.get("storyStates") or {}).get(story_id)


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
    sub = (state.get("storyStates") or {}).get(story_id)
    if not sub:
        return False
    if sub.get("phase") == phase:
        return False
    sub["phase"] = phase
    sub["lastUpdated"] = _now_ts()
    state["activeStory"] = story_id
    record_history(state, f"story-{story_id}-phase={phase}", by)
    return True


def add_story_to_nested_state(state: dict, story_id: str,
                               initial_phase: str = "initialized") -> bool:
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
    if story_id in story_states:
        return False
    now = _now_ts()
    story_states[story_id] = {
        "phase": initial_phase,
        "completedSteps": [],
        "codingRound": 0,
        "lastUpdated": now,
        "resetHistory": [],
    }
    if not state.get("activeStory"):
        state["activeStory"] = story_id
    record_history(state, f"story-{story_id}-added", by="ae-sdd")
    return True


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
    sub = (state.get("storyStates") or {}).get(story_id)
    if not sub:
        return False

    now = _now_ts()
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
    state["activeStory"] = story_id
    record_history(state, f"story-{story_id}-reset-to-{STORY_RESET_TARGET_PHASE}", by)
    return True


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
    if story_id not in (state.get("storyStates") or {}):
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
            sub = (state.get("storyStates") or {}).get(active_story)
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


def list_story_ids_in_state(state: dict) -> list[str]:
    """列出 state 内所有 Story ID（nested 返回 storyStates 键，flat 返回 currentStory 单值列表）。

    供 match_state 扫描匹配用。
    """
    if is_nested_state(state):
        return list((state.get("storyStates") or {}).keys())
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
                                design_dir: Path) -> tuple[Optional[Path], Optional[dict]]:
    """🆕 v3.9.3 内部辅助：确保父级 state 存在，必要时递归创建。

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        parent_type: "PRD" 或 "DR"
        parent_id: 父级 ID（如 "DR-005" / "PRD-001"）
        design_dir: design/ 目录（用于递归时验证父级的父级）

    Returns:
        (state_path, state_data) — 父级 state；或 (None, None) 当无法创建
    """
    from lib import paths as paths_mod  # 避免循环

    if parent_type == "PRD":
        hit = paths_mod.find_nested_state_by_prd_id(ade_sdd, parent_id)
        if hit:
            return hit
        # 父级 PRD 无 state → 替它创建
        try:
            st = init_nested_state(
                project_key="",
                entry_node="PRD",
                state_machine_id=f"PRD-{parent_id.replace('PRD-', '', 1)}",
                state_machine_name=parent_id,
                story_ids=None,
                prd_id=parent_id,
            )
            sp = paths_mod.work_item_state_path(ade_sdd, "PRD",
                                                {"prd_feature": parent_id.replace("PRD-", "", 1)})
            write_state(sp, st)
            return (sp, st)
        except Exception:
            return (None, None)
    elif parent_type == "DR":
        hit = paths_mod.find_nested_state_by_dr_id(ade_sdd, parent_id)
        if hit:
            return hit
        # 父级 DR 无 state → 检查 DR 有没有 PRD 父级，递归
        dr_doc = paths_mod._find_design_doc(design_dir, parent_id)
        prd_parent = None
        if dr_doc:
            prd_parent, _ = paths_mod.extract_parent_claim(dr_doc, doc_kind="dr")
        # 先确保 PRD 父级 state 存在
        if prd_parent:
            ok, _ = paths_mod.verify_parent_claim("PRD", prd_parent, design_dir, child_id=parent_id)
            if ok:
                prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", prd_parent, design_dir)
                # PRD 父级 state 准备好后，回到 DR 创建并嵌进 PRD 的 drState
        # 创建 DR 顶层 state（即使有 PRD 父级，DR 仍可独立创建顶层 state，
        #   然后下面 R2 吸收逻辑会把它嵌进 PRD）
        try:
            st = init_nested_state(
                project_key="",
                entry_node="DR",
                state_machine_id=f"DR-{parent_id.replace('DR-', '', 1)}",
                state_machine_name=parent_id,
                story_ids=None,
                dr_id=parent_id,
            )
            sp = paths_mod.work_item_state_path(ade_sdd, "DR",
                                                {"dr_feature": parent_id.replace("DR-", "", 1)})
            write_state(sp, st)
            return (sp, st)
        except Exception:
            return (None, None)
    return (None, None)


def recursive_r2_absorb(ade_sdd: Path, top_node: str, features: dict,
                        design_dir: Path,
                        doc_path: Optional[Path] = None,
                        child_id: str = "") -> tuple[Path, dict]:
    """🆕 v3.9.3 递归向上归入（用户定义的 R2 算法）。

    1. extract_parent_claim 读当前节点文档 → 抽父级声明
    2. verify_parent_claim 验证父级文档存在 + 关联性
    3. 无父级 / 父级文档找不到 → 当前层为顶层
    4. 有父级：
       a) 父级已有 state → 把当前节点加入该 state 的对应容器
       b) 父级无 state → 递归：先 _ensure_parent_nested_state 替父级建
          → 把当前节点嵌进新建的父级 state

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        top_node: 当前节点类型 PRD/DR/STORY/TASK
        features: 当前节点 R6 特征
        design_dir: design/ 目录
        doc_path: 当前节点文档（Story/DR 文档路径），用于抽父级
        child_id: 当前节点 ID（用于关联性验证）

    Returns:
        (state_path, state_data) — 当前节点最终所属嵌套 state
    """
    from lib import paths as paths_mod  # 避免循环

    top_node = (top_node or "").upper()
    if top_node not in ("PRD", "DR", "STORY", "TASK"):
        # 非法顶层 → 当作无父级 STORY 处理
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
        # reason=doc_not_found / relation_mismatch → 视为无父级（不阻塞）
    if parent_prd:
        ok, reason = paths_mod.verify_parent_claim("PRD", parent_prd, design_dir, child_id=child_id)
        if ok:
            valid_parent_prd = parent_prd

    # 3) 无有效父级 → 当前层为顶层
    if not valid_parent_dr and not valid_parent_prd:
        from lib import paths as _p
        sp = _p.work_item_state_path(ade_sdd, top_node, features)
        try:
            # 🆕 v3.9.3 补全 init_nested_state 所需形参
            kwargs = dict(
                project_key="",
                entry_node=top_node,
                state_machine_id=sp.parent.name,
                state_machine_name=sp.parent.name,
            )
            if top_node == "PRD":
                kwargs["prd_id"] = features.get("prd_id") or child_id
            elif top_node == "DR":
                kwargs["dr_id"] = features.get("dr_id") or features.get("dr_feature") or child_id
            elif top_node == "STORY":
                kwargs["story_ids"] = features.get("story_ids")
            st = init_nested_state(**kwargs)
            write_state(sp, st)
            return (sp, st)
        except Exception:
            # 已存在 → 直接读
            if sp.is_file():
                return (sp, read_state(sp))
            raise

    # 4a) Story → 嵌进父级 DR
    if top_node == "STORY" and valid_parent_dr:
        dr_hit = _ensure_parent_nested_state(ade_sdd, "DR", valid_parent_dr, design_dir)
        if dr_hit and dr_hit[0] is not None:
            dr_sp, dr_st = dr_hit
            # 嵌进 DR state 的 storyStates
            story_id = (features.get("story_ids") or [child_id])[0]
            add_story_to_nested_state(dr_st, story_id, initial_phase="story-generated")
            write_state(dr_sp, dr_st)
            return (dr_sp, dr_st)

    # 4b) DR → 嵌进父级 PRD
    if top_node == "DR" and valid_parent_prd:
        prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", valid_parent_prd, design_dir)
        if prd_hit and prd_hit[0] is not None:
            prd_sp, prd_st = prd_hit
            # 嵌进 PRD state 的 drState
            dr_id = features.get("dr_id") or child_id
            if prd_st.get("drState") is None:
                prd_st["drState"] = {"drId": dr_id, "phase": "dr-generated", "lastUpdated": _now_ts()}
                record_history(prd_st, f"absorb-dr-{dr_id}", by="recursive_r2_absorb")
            write_state(prd_sp, prd_st)
            return (prd_sp, prd_st)

    # 4c) Story 有 PRD 但无 DR → 嵌进 PRD（罕见，PRD 直接管 Story）
    if top_node == "STORY" and valid_parent_prd and not valid_parent_dr:
        prd_hit = _ensure_parent_nested_state(ade_sdd, "PRD", valid_parent_prd, design_dir)
        if prd_hit and prd_hit[0] is not None:
            prd_sp, prd_st = prd_hit
            story_id = (features.get("story_ids") or [child_id])[0]
            add_story_to_nested_state(prd_st, story_id, initial_phase="story-generated")
            write_state(prd_sp, prd_st)
            return (prd_sp, prd_st)

    # 5) 兜底：父级 state 创建失败 → 当前层为顶层
    from lib import paths as _p
    sp = _p.work_item_state_path(ade_sdd, top_node, features)
    if sp.is_file():
        return (sp, read_state(sp))
    try:
        st = init_nested_state(
            project_key="",
            entry_node=top_node,
            state_machine_id=sp.parent.name,
            state_machine_name=sp.parent.name,
            story_ids=features.get("story_ids") if top_node == "STORY" else None,
        )
        write_state(sp, st)
        return (sp, st)
    except Exception:
        # 已存在则读
        return (sp, read_state(sp) if sp.is_file() else {})
