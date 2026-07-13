"""
review_loop.py — review-loop 编排层状态机（🆕 v3.5.12 第 1 波）

根治「review-loop 公共协议零落地」体系性病根（P0-1/2/4）。

背景：
  review-loop 公共协议（source/skills/cross-cutting/review-loop-skill.md）规定
  「连续 2 轮无新增才退出 + 循环上限 2 轮 + Plan-first」，但 state.py 无 dryCounter
  字段、CLI 无命令、gates 无闸 → root agent 靠心智数轮次，compact/重入后丢失，
  且从不启用多 reviewer（自洽陷阱）。本模块把这套状态机变成可持久化、可机器判定的
  确定性逻辑，root agent 只负责「派活 + 处理存疑」，状态/判定/退出全在 CLI。

核心机制（半自动模式，对齐 ⑥.10 test-verifier 范式）：
  root agent 用 Agent 工具派 reviewer（智能层，CLI 不耦合 runtime）
  CLI 负责状态机：算 Tier → 派活指令 → 收报告 → 验 session 独立 → 算新增 dryCounter
                 → 判退出 → 持久化 reviewState

5 个核心函数（纯逻辑，无 IO，最易测）：
  derive_tier()         机械派生 Tier（吃 RA 规模 + 关键决策清单 → 1/2/3）
  compute_new_findings()锚点去重新增判定（finding 必挂 anchor，集合差集）
  check_session_independence() reviewer session 独立性（G-09B 核心）
  advance_round()       推进一轮：算 dryCounter + 判退出
  verify_exit()         退出条件校验（gates 兜底调用）

IO 函数（通过 state.py read/write_state）：
  start()   初始化/读 reviewState，派生 Tier，输出派活指令
  collect() 收 reviewer 报告，验 session + 算新增 + 推进轮次
  status()  读进度（重入用）
"""
from __future__ import annotations

import re
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

from lib import review_batch

# ─── 常量（与 review-loop-skill.md 协议对齐）─────────────────────────────────
EXIT_DRY_THRESHOLD = 2   # 连续 2 轮无新增才正常退出（协议 1）
MAX_ROUNDS = 2           # 循环上限 2 轮（协议 2）

# 关键决策点清单（机械派生 Tier 用，对齐 agent-orchestration §8.4.1）
# 命中任一 → Tier ≥ 2；命中资金/状态机/权限/全新表/全新 SPI 且大规模 → Tier 3
KEY_DECISION_PATTERNS = {
    "state-machine": [r"状态机", r"状态流转", r"state\s*machine", r"状态转移"],
    "transaction": [r"事务", r"分布式事务", r"transaction"],
    "fund": [r"资金", r"金额", r"退款", r"支付", r"结算", r"fund", r"payment"],
    "permission": [r"权限", r"鉴权", r"授权", r"permission", r"auth"],
    "external-integration": [r"外部集成", r"第三方", r"回调", r"SPI", r"外部接口"],
}


@dataclass
class TierResult:
    """Tier 派生结果（可追溯）"""
    tier: int                  # 1/2/3
    ra_size: str               # 微/小/中/大（来自 RA 5 维评分）
    key_decisions: list        # 命中的关键决策点类型
    rule: str                  # 派生规则说明


@dataclass
class RoundResult:
    """一轮推进结果"""
    round: int
    new_findings: list         # 本轮新增 finding（锚点去重后）
    dry_counter: int           # 累计连续无新增计数
    exit_reason: Optional[str] # None | "normal"(dryCounter≥3) | "escalate"(round>3且仍🔴)
    next_action: str           # "dispatch-reviewers" | "exit-normal" | "escalate-user"


@dataclass
class SessionCheckResult:
    """reviewer session 独立性校验结果（G-09B）"""
    passed: bool
    reason: str
    root_session_id: Optional[str]
    reviewer_session_ids: list
    violations: list           # 违规项（如 ["reviewer-0 sessionId == root"]）


# ════════════════════════════════════════════════════════════════════════════
# 核心 1：Tier 机械派生（治 P0-2）
# ════════════════════════════════════════════════════════════════════════════

def derive_tier(ra_size: str, decision_hints: str = "") -> TierResult:
    """机械派生 reviewerTier（不再 AI 主观判）。

    输入：
      ra_size: RA 5 维评分输出的规模（微/小/中/大）
      decision_hints: 决策线索文本（如 DR 摘要/Story 描述/RA 关键决策段）
                      用来机械匹配关键决策点类型

    派生规则（对齐 agent-orchestration §8.4.1，与 Code Review A/B/C 模式对齐）：
      Tier 1：微/小规模 且 无关键决策 → 单审
      Tier 2：中规模 或 含关键决策（状态机/事务/接口契约/外部集成）→ 双审交叉
      Tier 3：大规模 或 全新表/SPI 或 涉及资金/状态/权限/跨4Task+ → 三审交叉

    幂等：相同输入恒定输出相同 tier（机械可复算）。
    """
    size = (ra_size or "").strip().lower()
    hits: list[str] = []
    for decision_type, patterns in KEY_DECISION_PATTERNS.items():
        if any(re.search(p, decision_hints, re.IGNORECASE) for p in patterns):
            hits.append(decision_type)

    # 高危决策（资金/状态机/权限）+ 大规模 → Tier 3
    high_risk = {"fund", "state-machine", "permission"}
    if size == "大" or (hits and any(h in high_risk for h in hits) and size != "微"):
        # 大规模 或 含高危决策且非微 → Tier 3
        if size == "大" or len(hits) >= 2:
            return TierResult(3, ra_size, hits,
                              "大规模 或 含≥2 关键决策（含高危）→ Tier 3 三审")

    # 含任一关键决策 或 中规模 → Tier 2
    if hits or size == "中":
        return TierResult(2, ra_size, hits,
                          f"含关键决策 {hits} 或 中规模 → Tier 2 双审")

    # 微/小规模 且 无关键决策 → Tier 1
    return TierResult(1, ra_size, hits,
                      "微/小规模 且 无关键决策 → Tier 1 单审")


# ════════════════════════════════════════════════════════════════════════════
# 核心 2：锚点去重新增判定（治过载型 dryCounter 永卡 0）
# ════════════════════════════════════════════════════════════════════════════

# finding 锚点格式：必须含前缀分类，便于集合去重
# 合法前缀：DR§ / FILE: / FIELD: / API: / TASK: / AC:
_ANCHOR_PREFIX_RE = re.compile(r"^(DR§|FILE:|FIELD:|API:|TASK:|AC:)")


def normalize_anchor(anchor: str) -> str:
    """归一化锚点（去首尾空白 + 统一前缀分隔符），便于集合去重。"""
    return (anchor or "").strip()


def validate_anchor(anchor: str) -> bool:
    """校验锚点格式（必须含合法前缀，对齐 story-review 标尺1 禁止裸结论）。"""
    a = normalize_anchor(anchor)
    return bool(_ANCHOR_PREFIX_RE.match(a))


def compute_new_findings(current_findings: list[dict],
                          historical_anchors: set) -> tuple[list[dict], list[dict]]:
    """计算本轮新增 finding（锚点去重）。

    Args:
      current_findings: 本轮 reviewer 报告的 finding 列表，每项含 anchor + severity
      historical_anchors: 历史累计 finding 的 anchor 集合

    Returns:
      (new_findings, rejected)
      new_findings: 锚点不在历史集合 且 锚点格式合法 的新 finding
      rejected: 因锚点格式非法被拒的 finding（对齐 story-review 标尺1）

    锚点缺失/格式非法的 finding → 拒收（防 AI 写裸结论刷 dryCounter）。
    """
    new_findings: list[dict] = []
    rejected: list[dict] = []
    for f in current_findings:
        anchor = normalize_anchor(f.get("anchor", ""))
        if not validate_anchor(anchor):
            rejected.append({**f, "_reject_reason": "锚点缺失或格式非法（需 DR§/FILE:/FIELD:/API:/TASK:/AC: 前缀）"})
            continue
        if anchor not in historical_anchors:
            new_findings.append(f)
    return new_findings, rejected


# ════════════════════════════════════════════════════════════════════════════
# 核心 3：reviewer session 独立性校验（G-09B，治 P0-2 自洽陷阱）
# ════════════════════════════════════════════════════════════════════════════

def check_session_independence(reviewer_session_ids: list[str],
                                root_session_id: str,
                                tier: int) -> SessionCheckResult:
    """校验 reviewer 是否真独立（sessionId ≠ root 且 数量 ≥ Tier 要求）。

    这是堵「root agent 自扮多 reviewer」的核心防线：
      - reviewer 数 < tier 要求 → 阻断（没派够）
      - 任一 reviewer sessionId == root sessionId → 阻断（自扮）

    Args:
      reviewer_session_ids: 本轮各 reviewer 的 sessionId 列表
      root_session_id: root agent 自己的 sessionId（读 session.json）
      tier: 当前 review 节点的 Tier（决定需要几个 reviewer）

    诚实边界：session_id 靠"字符串不等"判定，要做到 runtime 注入不可伪造
    需 Mavis harness 配合。当前足够堵最常见的"完全自扮"偷懒。
    """
    violations: list[str] = []
    needed = max(1, tier)  # Tier 1 需 1 个，Tier 2 需 2 个，Tier 3 需 3 个

    if len(reviewer_session_ids) < needed:
        violations.append(
            f"reviewer 数 {len(reviewer_session_ids)} < Tier {tier} 要求 {needed}")

    for i, sid in enumerate(reviewer_session_ids):
        if root_session_id and sid == root_session_id:
            violations.append(f"reviewer-{i} sessionId == root（自扮多 reviewer）")

    if violations:
        return SessionCheckResult(
            passed=False,
            reason="; ".join(violations),
            root_session_id=root_session_id,
            reviewer_session_ids=reviewer_session_ids,
            violations=violations,
        )
    return SessionCheckResult(
        passed=True,
        reason=f"独立（{len(reviewer_session_ids)} reviewer，全部 ≠ root）",
        root_session_id=root_session_id,
        reviewer_session_ids=reviewer_session_ids,
        violations=[],
    )


# ════════════════════════════════════════════════════════════════════════════
# 核心 4：推进一轮（算 dryCounter + 判退出）
# ════════════════════════════════════════════════════════════════════════════

def advance_round(review_state: dict,
                   current_findings: list[dict],
                   has_red_blocker: bool = False) -> RoundResult:
    """推进一轮：算新增 → 更新 dryCounter → 判退出。

    Args:
      review_state: 当前 reviewLoop 状态 dict（含 round/dryCounter/findings）
      current_findings: 本轮 reviewer 报告的 finding（含 anchor）
      has_red_blocker: 本轮是否仍有 🔴 阻断型未解决（影响 escalate 判定）

    协议对齐（review-loop-skill.md）：
      协议1：本轮有新确认缺陷 → dryCounter 归零；无新增 → dryCounter+1；累计3 → 退出
      协议2：round > MAX_ROUNDS(2) 且仍有 🔴 → escalate 用户

    Returns:
      RoundResult（含更新后的 round/dryCounter/exit_reason/next_action）
    """
    historical_anchors = {normalize_anchor(f.get("anchor", ""))
                          for f in review_state.get("findings", [])
                          if validate_anchor(f.get("anchor", ""))}
    new_findings, _rejected = compute_new_findings(current_findings, historical_anchors)

    new_round = review_state.get("round", 0) + 1
    if new_findings:
        new_dry = 0
    else:
        new_dry = review_state.get("dryCounter", 0) + 1

    # 判退出
    exit_reason: Optional[str] = None
    next_action = "dispatch-reviewers"  # 默认继续派

    if new_dry >= EXIT_DRY_THRESHOLD:
        # 协议1：连续 3 轮无新增 → 正常退出
        exit_reason = "normal"
        next_action = "exit-normal"
    elif new_round > MAX_ROUNDS and has_red_blocker:
        # 协议2：超过循环上限且仍有 🔴 → 升级用户
        exit_reason = "escalate"
        next_action = "escalate-user"

    return RoundResult(
        round=new_round,
        new_findings=new_findings,
        dry_counter=new_dry,
        exit_reason=exit_reason,
        next_action=next_action,
    )


# ════════════════════════════════════════════════════════════════════════════
# 核心 5：退出条件校验（gates 兜底调用）
# ════════════════════════════════════════════════════════════════════════════

def verify_exit(review_state: dict) -> tuple[bool, str]:
    """校验 reviewLoop 是否满足退出条件（gates G-REVIEW-LOOP 兜底调用）。

    Returns:
      (passed, reason)
      passed=True：exitReason ∈ {normal, escalate}，且 normal 要求 dryCounter≥2
      passed=False：未达退出条件，阻断节点切相

    正常退出（normal）：dryCounter ≥ 2
    异常退出（escalate）：round > 2 且有 🔴，已升级用户决策
    """
    if int(review_state.get("schemaVersion", 0) or 0) >= review_batch.SCHEMA_VERSION:
        return review_batch.verify_exit(review_state)

    exit_reason = review_state.get("exitReason")
    dry_counter = review_state.get("dryCounter", 0)
    round_ = review_state.get("round", 0)

    if exit_reason == "normal":
        if dry_counter >= EXIT_DRY_THRESHOLD:
            return True, f"正常退出（连续 {dry_counter} 轮无新增）"
        return False, f"exitReason=normal 但 dryCounter={dry_counter} < {EXIT_DRY_THRESHOLD}（数据不一致）"

    if exit_reason == "escalate":
        return True, f"异常退出（round={round_} 仍有 🔴，已升级用户）"

    return False, f"未达退出条件（exitReason={exit_reason}, dryCounter={dry_counter}, round={round_}）"


# ════════════════════════════════════════════════════════════════════════════
# IO 函数：start / collect / status（通过 state.py read/write_state）
# ════════════════════════════════════════════════════════════════════════════

def _ensure_review_state(state: dict, node: str, ra_size: str,
                          decision_hints: str) -> dict:
    """确保 state["reviewLoop"] 存在；不存在则初始化（含 Tier 派生）。"""
    rl = state.get("reviewLoop")
    if rl and rl.get("node") == node:
        return rl
    tier = derive_tier(ra_size, decision_hints)
    rl = review_batch.create_session(
        node=node,
        tier=tier.tier,
        tier_basis={
            "raSize": tier.ra_size,
            "keyDecisions": tier.key_decisions,
            "rule": tier.rule,
        },
    )
    state["reviewLoop"] = rl
    state["reviewSession"] = rl
    return rl


def start(state: dict, node: str, ra_size: str = "中",
          decision_hints: str = "", input_fingerprint: str = "",
          budgets: Optional[dict] = None) -> dict:
    """初始化/读 reviewState，机械派生 Tier，输出本轮派活指令。

    Args:
      state: read_state() 返回的 dict（会被原地修改）
      node: review 节点名（如 story-review / dr-review / code-review / ra-review）
      ra_size: RA 规模（微/小/中/大）
      decision_hints: 决策线索文本（机械匹配关键决策点）

    Returns:
      {tier, reviewersNeeded, lens, dryCounter, round, action, dispatchHint}
      action="dispatch-reviewers" → root agent 按 dispatchHint 派 N 个 reviewer
    """
    rl = _ensure_review_state(state, node, ra_size, decision_hints)
    if int(rl.get("schemaVersion", 0) or 0) < review_batch.SCHEMA_VERSION:
        rl = review_batch.upgrade_legacy(rl, node=node)
        state["reviewLoop"] = rl
        state["reviewSession"] = rl
    if input_fingerprint and rl.get("inputFingerprint") and input_fingerprint != rl.get("inputFingerprint"):
        rl = review_batch.restart_for_fingerprint(
            rl, input_fingerprint, ruleset_fingerprint=rl.get("rulesetFingerprint", "")
        )
        state["reviewLoop"] = rl
        state["reviewSession"] = rl
    elif input_fingerprint:
        rl["inputFingerprint"] = input_fingerprint
    if budgets:
        rl["budgets"].update({k: int(v) for k, v in budgets.items() if v is not None})
        deadline = review_batch._parse_iso(rl.get("startedAt"))
        if deadline:
            from datetime import timedelta
            rl["deadlineAt"] = review_batch._iso(
                deadline + timedelta(minutes=int(rl["budgets"]["maxWallClockMinutes"]))
            )
    return {
        "schemaVersion": rl.get("schemaVersion", 1),
        "engine": rl.get("engine", "legacy-round-v1"),
        "tier": rl["tier"],
        "reviewersNeeded": rl["tier"],
        "lens": _lens_for_tier(node, rl["tier"]),
        "dryCounter": rl["dryCounter"],
        "round": rl["round"],
        "exitReason": rl["exitReason"],
        "budgets": rl.get("budgets", {}),
        "inputFingerprint": rl.get("inputFingerprint", ""),
        "action": "dispatch-reviewers",
        "dispatchHint": (
            f"派 {rl['tier']} 个 reviewer（视角：{_lens_for_tier(node, rl['tier'])}），"
            f"各 reviewer 用独立 sub-agent session 执行，回传报告时附 sessionId"
        ),
    }


def _lens_for_tier(node: str, tier: int) -> list[str]:
    """按节点 + Tier 给视角切分（对齐各 SKILL 的多 reviewer 视角切分小节）。"""
    # 简化映射，完整视角切分见各节点 SKILL §多 reviewer 视角切分
    lens_map = {
        "story-review": {1: ["全维度"], 2: ["设计实现", "前端契约"], 3: ["设计实现", "前端契约", "数据模型"]},
        "code-review": {1: ["全维度"], 2: ["BE业务实现", "AR架构规范"], 3: ["BE业务实现", "AR架构规范", "QA测试真实性"]},
        "dr-review": {1: ["全维度"], 2: ["业务价值", "架构合理性"], 3: ["业务价值", "架构合理性", "接口契约"]},
        "ra-review": {1: ["全维度"], 2: ["需求完整性", "衍生深度"], 3: ["需求完整性", "衍生深度", "规模风险"]},
    }
    return lens_map.get(node, {1: ["全维度"], 2: ["视角A", "视角B"], 3: ["视角A", "视角B", "视角C"]}).get(tier, ["全维度"])


def collect(state: dict, node: str,
             reviewer_reports: list[dict],
             root_session_id: str,
             has_red_blocker: bool = False,
             input_fingerprint: str = "",
             ruleset_fingerprint: str = "",
             batch_id: str = "",
             strict_roles: bool = False) -> dict:
    """收 reviewer 报告：验 session 独立 → 算新增 dryCounter → 推进轮次 → 持久化。

    Args:
      state: read_state() 返回的 dict（会被原地修改 + 调用方负责 write_state）
      node: review 节点名
      reviewer_reports: [{findings: [...], sessionId: "...", report: "path"}]
                        每个 report 的 findings 含 anchor + severity
      root_session_id: root agent sessionId（读 session.json）
      has_red_blocker: 本轮是否仍有 🔴 阻断型未解决

    Returns:
      {round, newFindings, dryCounter, exitReason, nextAction, sessionCheck, rejected}
      nextAction="dispatch-reviewers" → 继续派（root agent 按 start() 派）
      nextAction="exit-normal" → 正常退出，可切相
      nextAction="escalate-user" → 升级用户
    """
    rl = state.get("reviewSession") or state.get("reviewLoop") or {}
    if int(rl.get("schemaVersion", 0) or 0) >= review_batch.SCHEMA_VERSION:
        tier = int(rl.get("tier") or 1)
        reports_for_check = reviewer_reports
        if batch_id:
            previous = next((b for b in rl.get("batches", []) if b.get("batchId") == batch_id), None)
            if previous:
                existing = {str(r.get("role") or "").upper(): r for r in previous.get("reviewers", []) if r.get("role")}
                for report in reviewer_reports:
                    role = str(report.get("role") or "").upper()
                    if role:
                        existing[role] = report
                role_order = {1: ["GENERAL"], 2: ["BE", "AR"], 3: ["BE", "AR", "QA"]}.get(tier, [])
                reports_for_check = [existing[r] for r in role_order if r in existing]
        session_chk = check_session_independence(
            [r.get("sessionId", "") for r in reports_for_check], root_session_id, tier
        )
        if not session_chk.passed:
            return {
                "schemaVersion": review_batch.SCHEMA_VERSION,
                "round": rl.get("round", 0),
                "dryCounter": rl.get("dryCounter", 0),
                "newFindings": [],
                "nextAction": "blocked-session-not-independent",
                "exitReason": rl.get("exitReason"),
                "sessionCheck": {
                    "passed": False,
                    "reason": session_chk.reason,
                    "violations": session_chk.violations,
                },
                "rejected": [],
            }
        if not strict_roles and reviewer_reports and not any(r.get("role") for r in reviewer_reports):
            role_sets = {1: ["GENERAL"], 2: ["BE", "AR"], 3: ["BE", "AR", "QA"]}
            for report, role in zip(reviewer_reports, role_sets.get(tier, ["GENERAL"])):
                report["role"] = role
        result = review_batch.collect_batch(
            rl,
            reviewer_reports,
            root_session_id,
            input_fingerprint=input_fingerprint,
            ruleset_fingerprint=ruleset_fingerprint,
            batch_id=batch_id,
            has_red_blocker=has_red_blocker,
        )
        state["reviewLoop"] = rl
        state["reviewSession"] = rl
        return result
    if rl:
        rl = review_batch.upgrade_legacy(rl, node=node)
        state["reviewLoop"] = rl
        state["reviewSession"] = rl
        return collect(
            state,
            node,
            reviewer_reports,
            root_session_id,
            has_red_blocker=has_red_blocker,
            input_fingerprint=input_fingerprint,
            ruleset_fingerprint=ruleset_fingerprint,
            batch_id=batch_id,
            strict_roles=strict_roles,
        )
    tier = rl.get("tier", 1)

    # 1. session 独立性校验
    reviewer_sids = [r.get("sessionId", "") for r in reviewer_reports]
    session_chk = check_session_independence(reviewer_sids, root_session_id, tier)

    # session 不独立 → 不推进，返回阻断（root 必须真派）
    if not session_chk.passed:
        return {
            "round": rl.get("round", 0),
            "newFindings": [],
            "dryCounter": rl.get("dryCounter", 0),
            "exitReason": rl.get("exitReason"),
            "nextAction": "blocked-session-not-independent",
            "sessionCheck": {
                "passed": False,
                "reason": session_chk.reason,
                "violations": session_chk.violations,
            },
            "rejected": [],
        }

    # 2. 汇总本轮 finding
    current_findings: list[dict] = []
    for rpt in reviewer_reports:
        for f in rpt.get("findings", []):
            current_findings.append(f)

    # 3. 推进一轮
    result = advance_round(rl, current_findings, has_red_blocker)

    # 4. 持久化到 state（调用方负责 write_state）
    rl["round"] = result.round
    rl["dryCounter"] = result.dry_counter
    rl["exitReason"] = result.exit_reason
    if result.exit_reason:
        from datetime import datetime, timezone
        rl["exitedAt"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    # 累加 finding（去重后的新增）
    rl["findings"] = rl.get("findings", []) + result.new_findings
    rl["reviewers"] = [{"sessionId": r.get("sessionId"),
                         "report": r.get("report")} for r in reviewer_reports]
    state["reviewLoop"] = rl

    # 5. rejected（锚点格式非法）
    historical_anchors = {normalize_anchor(f.get("anchor", ""))
                          for f in rl.get("findings", [])
                          if validate_anchor(f.get("anchor", ""))}
    _, rejected = compute_new_findings(current_findings, historical_anchors)

    return {
        "round": result.round,
        "newFindings": [{"id": f.get("id"), "anchor": f.get("anchor"),
                          "severity": f.get("severity")} for f in result.new_findings],
        "dryCounter": result.dry_counter,
        "exitReason": result.exit_reason,
        "nextAction": result.next_action,
        "sessionCheck": {"passed": True, "reason": session_chk.reason},
        "rejected": [{"anchor": r.get("anchor"), "reason": r.get("_reject_reason")}
                     for r in rejected],
    }


def status(state: dict) -> dict:
    """读 reviewLoop 进度（report-only，重入用）。"""
    rl = state.get("reviewSession") or state.get("reviewLoop") or {}
    if int(rl.get("schemaVersion", 0) or 0) >= review_batch.SCHEMA_VERSION:
        return review_batch.status(rl)
    return {
        "node": rl.get("node"),
        "tier": rl.get("tier"),
        "round": rl.get("round", 0),
        "dryCounter": rl.get("dryCounter", 0),
        "exitReason": rl.get("exitReason"),
        "findingsCount": len(rl.get("findings", [])),
        "reviewers": rl.get("reviewers", []),
    }
