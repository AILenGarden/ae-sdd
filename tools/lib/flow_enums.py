"""
flow_enums.py — ae-sdd 流程节点枚举体系

为 state.json 的 events 字段提供类型约束。

三个枚举：
  FlowNode      — 流程节点原语（任务从哪个节点进入 = 任务类型）
  FlowSkill     — SKILL 标识符（与 source/skills/ 目录下文件名对应）
  FlowEventType — 事件类型（操作动词）

一个数据类：
  FlowEvent     — 单条流程事件记录（append-only）

设计原则：
  - 枚举继承 str，JSON 序列化直接得字符串，无需额外转换
  - FlowEvent 存 .value 字符串，运行时不依赖枚举导入也可读
  - output / meta 用 dict 承载可变内容，外层结构保持固定
"""
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


# ─── 枚举定义 ─────────────────────────────────────────────────────────────────


class FlowNode(str, Enum):
    """流程节点原语（入口节点即任务类型）。

    节点即任务类型：任务从流水线哪个节点进入，就是哪类任务。
    流水线从重到轻：PRD -> RA -> DR -> STORY -> PLAN（v3.10.0 砍 Task）

    🆕 v3.9.0 嵌套状态模型：entryNode 升级为"容器选择器"。
      - entryNode=PRD -> state 含 prdState + drState + storyStates
      - entryNode=DR  -> state 含 drState + storyStates（DR 为顶层）
      - entryNode=STORY -> state 含 storyStates（Story 为顶层）
      - entryNode=PLAN/BUG -> flat state（小/微任务，不嵌套）
    详见 state.py ENTRY_NODE_CONTAINERS 与 init_nested_state()。

    🆕 v3.10.0 Route 下移重分级：
      - PRD -> 大（兼容，视为 DR 级）
      - DR  -> 大
      - STORY -> 中
      - PLAN -> 小（CodingPlan 入口）
      - BUG  -> 微（无文档直出 CodingPlan）
      - TASK -> 🟡 deprecated（骨架分解合并进 CodingProcess）
    """

    PRD   = "PRD"    # PRD 级聚合（需求文档入口，v3.9.0 嵌套顶层之一；v3.10.0 兼容视为大）
    RA    = "RA"     # 需求分析（Requirement Analysis）
    DR    = "DR"     # 设计需求（Design Requirement；v3.9.0 嵌套顶层之一；v3.10.0 大任务入口）
    STORY = "STORY"  # Story 级（v3.9.0 嵌套顶层之一；v3.10.0 中任务入口）
    TASK  = "TASK"   # 🟡 v3.10.0 deprecated：Task phase 已移除，保留枚举兼容旧 state
    PLAN  = "PLAN"   # 小任务 / CodingPlan 直出（flat state；v3.10.0 小任务入口）
    BUG   = "BUG"    # 微任务 / 无文档直出 CodingPlan（flat state；v3.10.0 微任务入口）

    def container_fields(self) -> list[str]:
        """🆕 v3.9.0 返回该入口节点应含的子状态容器名列表。

        - PRD/DR/STORY → 返回对应容器列表（如 ["prdState","drState","storyStates"]）
        - RA/TASK/PLAN → 返回空列表（flat state，不嵌套）

        Returns:
            容器名列表；空列表表示该节点走 flat state（R4 微任务）
        """
        # 🆕 v3.9.0 嵌套容器映射（与 state.ENTRY_NODE_CONTAINERS 一致）
        # flat 节点（TASK/PLAN/RA）返回空列表，表示用 flat state 不嵌套。
        mapping = {
            "PRD":   ["prdState", "drStates", "storyStates"],
            "DR":    ["drState", "storyStates"],
            "STORY": ["storyStates"],
        }
        return list(mapping.get(self.value, []))

    @classmethod
    def is_nested_entry(cls, node: str) -> bool:
        """🆕 v3.9.0 判断节点值是否为嵌套入口（PRD/DR/STORY）。

        Args:
            node: FlowNode.value 字符串（如 "PRD"）

        Returns:
            True=该节点可作为嵌套 state 顶层；False=走 flat state
        """
        return node in ("PRD", "DR", "STORY")


class FlowSkill(str, Enum):
    """SKILL 标识符（与 source/skills/ 目录下文件名一一对应）。

    值直接用于 state.json events[].skill 字段，JSON 可读。
    """

    REQUIREMENT_ANALYSIS = "requirement-analysis-skill"
    DR_GENERATE          = "dr-generate-skill"
    DR_REVIEW            = "dr-review-skill"
    DR_UPDATE            = "dr-update-skill"
    STORY_GENERATE       = "story-generate-skill"
    STORY_REVIEW         = "story-review-skill"
    STORY_UPDATE         = "story-update-skill"
    TESTCASE_GENERATE    = "testcase-generate-skill"
    TESTCASE_REVIEW      = "testcase-review-skill"
    TASK_GENERATE        = "task-generate-skill"
    CODING               = "coding-skill"
    CODING_REPORT        = "coding-report-skill"
    TEST_GENERATE        = "test-generate-skill"
    TEST_REVIEW          = "test-review-skill"
    CODE_REVIEW          = "code-review-skill"
    PROPOSAL             = "proposal-skill"
    AE_SDD_UPDATE        = "ae-sdd-update-skill"
    AE_SDD_INSTALL       = "ae-sdd-install-skill"


class FlowEventType(str, Enum):
    """事件类型（操作动词）。

    一对 routed-to + skill-completed 描述一个 SKILL 的完整生命周期：
      routed-to       — 路由完成，SKILL 开始执行
      skill-completed — SKILL 执行完毕，出具产物

    其余事件记录流程中的关键状态变化。
    """

    ROUTED_TO       = "routed-to"       # 路由到某 SKILL（同时意味着该 SKILL 开始执行）
    SKILL_COMPLETED = "skill-completed" # SKILL 执行完毕，出具产物
    GATE_BLOCKED    = "gate-blocked"    # 门禁拦截（G-xx 未通过）
    GATE_CLEARED    = "gate-cleared"    # 门禁通过
    USER_CONFIRMED  = "user-confirmed"  # 用户审核确认（审核点 token）
    PHASE_CHANGED   = "phase-changed"   # state write 触发阶段切换
    REOPENED        = "reopened"        # 任务重入（已完成后再次开启）
    ABORTED         = "aborted"         # 任务中止


# ─── 事件数据类 ───────────────────────────────────────────────────────────────


@dataclass
class FlowEvent:
    """单条流程事件记录（append-only）。

    字段设计原则：
      - 结构固定，所有字段类型固定（str / int / Optional[str/dict]）
      - 字符串值沿用枚举 .value，保证 JSON 可读、不依赖枚举导入
      - output / meta 用 dict 承载可变内容，不破坏外层结构
      - txnName 区分同一 PRD state.json 里不同子任务的事件

    必填字段：
      seq      — 自增序号，便于排序和断链检测
      ts       — ISO 8601 UTC，格式 "2026-06-26T10:00:00Z"
      event    — FlowEventType.value
      node     — FlowNode.value，当前所在节点
      by       — 触发方：FlowSkill.value 或 "ae-sdd" / "user"

    条件必填（对应 event 类型时须填）：
      skill    — event=routed-to / skill-completed 时必填，FlowSkill.value
      gate_id  — event=gate-blocked / gate-cleared 时必填，如 "G-07"
      phase    — event=phase-changed 时必填，目标 phase 名

    可选补充：
      txnName  — 子任务标识（STORY-001-BE / Task-xxx / Plan-xxx）
                 PRD 自身的事件填 None
      from_node — 路由来源节点（FlowNode.value），跨节点路由时有值
      reason   — 路由依据 / 门禁拦截原因 / 重入原因 / 用户确认结论
      output   — skill-completed 时的产物描述
                 建议结构：{"type": str, "path": str, "artifact_id": str（可选）}
      meta     — 其他扩展字段（预留，不用不填）
    """

    # 必填
    seq:       int
    ts:        str
    event:     str   # FlowEventType.value
    node:      str   # FlowNode.value
    by:        str   # FlowSkill.value 或 "ae-sdd" / "user"

    # 条件必填
    skill:     Optional[str] = None   # FlowSkill.value
    gate_id:   Optional[str] = None   # 门禁 ID，如 "G-07" / "G-RA-1"
    phase:     Optional[str] = None   # 目标 phase（event=phase-changed 时）

    # 可选
    txnName:   Optional[str] = None   # 子任务标识
    from_node: Optional[str] = None   # 来源节点（FlowNode.value）
    reason:    Optional[str] = None   # 路由依据 / 拦截原因 / 确认结论
    output:    Optional[dict] = None  # 产物描述
    meta:      Optional[dict] = None  # 扩展字段（预留）

    def to_dict(self) -> dict:
        """序列化为 dict，None 值字段自动过滤（保持 JSON 简洁）。"""
        return {k: v for k, v in self.__dict__.items() if v is not None}


# ─── 工厂函数（常用事件快速构造）────────────────────────────────────────────────


def make_routed_to(
    seq: int,
    ts: str,
    node: FlowNode,
    skill: FlowSkill,
    by: str = "ae-sdd",
    *,
    txn_name: Optional[str] = None,
    from_node: Optional[FlowNode] = None,
    reason: Optional[str] = None,
) -> FlowEvent:
    """构造 routed-to 事件。"""
    return FlowEvent(
        seq=seq,
        ts=ts,
        event=FlowEventType.ROUTED_TO,
        node=node.value,
        by=by,
        skill=skill.value,
        txnName=txn_name,
        from_node=from_node.value if from_node else None,
        reason=reason,
    )


def make_skill_completed(
    seq: int,
    ts: str,
    node: FlowNode,
    skill: FlowSkill,
    *,
    txn_name: Optional[str] = None,
    output_type: Optional[str] = None,
    output_path: Optional[str] = None,
    artifact_id: Optional[str] = None,
    reason: Optional[str] = None,
) -> FlowEvent:
    """构造 skill-completed 事件。"""
    output: Optional[dict] = None
    if output_type or output_path:
        output = {}
        if output_type:
            output["type"] = output_type
        if output_path:
            output["path"] = output_path
        if artifact_id:
            output["artifact_id"] = artifact_id
    return FlowEvent(
        seq=seq,
        ts=ts,
        event=FlowEventType.SKILL_COMPLETED,
        node=node.value,
        by=skill.value,
        skill=skill.value,
        txnName=txn_name,
        output=output,
        reason=reason,
    )


def make_gate_blocked(
    seq: int,
    ts: str,
    node: FlowNode,
    gate_id: str,
    reason: str,
    *,
    txn_name: Optional[str] = None,
) -> FlowEvent:
    """构造 gate-blocked 事件。"""
    return FlowEvent(
        seq=seq,
        ts=ts,
        event=FlowEventType.GATE_BLOCKED,
        node=node.value,
        by="ae-sdd",
        gate_id=gate_id,
        txnName=txn_name,
        reason=reason,
    )


def make_phase_changed(
    seq: int,
    ts: str,
    node: FlowNode,
    phase: str,
    *,
    txn_name: Optional[str] = None,
) -> FlowEvent:
    """构造 phase-changed 事件。"""
    return FlowEvent(
        seq=seq,
        ts=ts,
        event=FlowEventType.PHASE_CHANGED,
        node=node.value,
        by="ae-sdd state write",
        phase=phase,
        txnName=txn_name,
    )


def make_user_confirmed(
    seq: int,
    ts: str,
    node: FlowNode,
    reason: str,
    *,
    txn_name: Optional[str] = None,
) -> FlowEvent:
    """构造 user-confirmed 事件。"""
    return FlowEvent(
        seq=seq,
        ts=ts,
        event=FlowEventType.USER_CONFIRMED,
        node=node.value,
        by="user",
        reason=reason,
        txnName=txn_name,
    )
