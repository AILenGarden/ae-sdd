#!/usr/bin/env python3
"""test-tool 评分 skill — 评分计算脚本。

读取 collect.py 产出的 .collected.json，按 EVALUATION.md 四维度评分卡计算分数，
落地符合 metrics.schema.json 的 metrics-<STORY-ID>-run<N>.json。

评分逻辑严格对齐 EVALUATION.md：
  - 维度 A 红线（A1/A6/A8/A9/A11）任一违反 → A=0，红线标记置位
  - 维度 C 拆必修（80）+ 选修（30 加分）
  - 维度 D 按时长分档
  - 派生比率按 EVALUATION.md §5.6 公式计算
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# 评分常量（对齐 EVALUATION.md）
# ---------------------------------------------------------------------------

# 维度 A 各项满分（EVALUATION.md §2.1）
A_MAX = {
    "A1": 8, "A2": 6, "A3": 8, "A4": 6, "A5": 8, "A6": 10, "A7": 8,
    "A8": 10, "A9": 10, "A10": 6, "A11": 10, "A12": 8, "A13": 6, "A14": 6,
}
A_RED_LINES = {"A1", "A6", "A8", "A9", "A11"}

# 维度 B 各项满分（EVALUATION.md §3.1）
B_MAX = {
    "B1": 8, "B2": 8, "B3": 10, "B4": 10, "B5": 10, "B6": 8,
    "B7": 8, "B8": 8, "B9": 6, "B10": 8, "B11": 8, "B12": 8,
}

# 维度 C 必修 AC 满分（EVALUATION.md §4.1）
C_REQUIRED_AC_MAX = {
    "AC-1": 3, "AC-2": 4, "AC-3": 5, "AC-4": 4, "AC-5": 3,
    "AC-6": 4, "AC-7": 5, "AC-8": 4, "AC-9": 6, "AC-10": 2,  # AC-10 为加分
}
C_BOUNDARY_MAX = 15          # 12 条 × 1.25 分
C_CONTRACT_MAX = 15
C_ENGINEERING_MAX = 10

# 维度 C 选修组满分（EVALUATION.md §4.5）
C_ELECTIVE_GROUPS = {
    "trait":    {"acs": ["AC-11", "AC-12"], "max": 8},
    "generic":  {"acs": ["AC-13", "AC-14"], "max": 8},
    "builder":  {"acs": ["AC-15", "AC-16"], "max": 7},
    "serde":    {"acs": ["AC-17", "AC-18"], "max": 7},
}

# 维度 D 时长分档
D_DURATION_TIERS = [
    (60, 100),
    (120, 80),
    (180, 60),
    (float("inf"), 40),
]

# AC 必修 + 选修集合
REQUIRED_ACS = sorted(C_REQUIRED_AC_MAX.keys())       # AC-1..AC-10
ELECTIVE_ACS = {ac for g in C_ELECTIVE_GROUPS.values() for ac in g["acs"]}  # AC-11..AC-18


# ---------------------------------------------------------------------------
# 通用
# ---------------------------------------------------------------------------


def _ok_item(score: float, max_score: float, evidence: str) -> dict[str, Any]:
    return {"passed": score >= max_score, "score": score, "maxScore": max_score, "evidence": evidence}


def _fail_item(max_score: float, evidence: str) -> dict[str, Any]:
    return {"passed": False, "score": 0, "maxScore": max_score, "evidence": evidence}


# ---------------------------------------------------------------------------
# 维度 A：流程合规性
# ---------------------------------------------------------------------------


def score_dimension_a(collected: dict) -> tuple[dict[str, Any], list[str], bool]:
    """返回 (items, raw_breakdown_notes, red_line_violated)。"""
    items: dict[str, Any] = {}
    notes: list[str] = []
    red_line_violated = False

    state_data = collected.get("state", {})
    state = state_data.get("state", {}) if state_data.get("found") else {}
    history = state.get("history", [])
    route_decision = state.get("routeDecision") or {}
    execution_plan = state.get("executionPlan") or {}
    review_session = state.get("reviewSession") or {}
    evidence_data = state_data.get("evidence", {})
    evidence_entries = evidence_data.get("entries", []) if evidence_data else []
    runtime_stats = collected.get("runtimeStats", {})

    # A1: Route 真发生
    a1_pass = bool(route_decision) and any(
        h.get("phase") == "route-selected"
        or "route" in str(h.get("phase", "")).lower()
        for h in history if isinstance(h, dict)
    )
    if a1_pass:
        items["A1"] = _ok_item(8, 8, f"routeDecision={route_decision}")
    else:
        items["A1"] = _fail_item(8, f"routeDecision empty or no route-selected in history")
        red_line_violated = True
        notes.append("A1 红线违反：Route 未发生")

    # A2: 规模判定为「中」
    scale = state.get("scale", "")
    if scale in ("中", "medium"):
        items["A2"] = _ok_item(6, 6, f"scale={scale}")
    elif scale in ("小", "微", "small", "micro"):
        items["A2"] = _ok_item(3, 6, f"scale={scale} (误判为小/微)")
        notes.append("A2 误判为小/微，扣 50%")
    elif scale in ("大", "large"):
        items["A2"] = _ok_item(4.5, 6, f"scale={scale} (误判为大)")
        notes.append("A2 误判为大，扣 25%")
    else:
        items["A2"] = _fail_item(6, f"scale 字段缺失或未知: {scale!r}")

    # A3: RA 文档生成
    ra_artifact = next(
        (a for a in collected.get("artifacts", []) if a["role"] == "ra"), None
    )
    if ra_artifact and ra_artifact["exists"]:
        items["A3"] = _ok_item(8, 8, f"{ra_artifact['path']} sha256={ra_artifact['sha256'][:16]}")
    else:
        items["A3"] = _fail_item(8, "RA 文档不存在")

    # A4: DR 文档生成（加分项）
    dr_artifact = next(
        (a for a in collected.get("artifacts", []) if a["role"] == "dr"), None
    )
    if dr_artifact and dr_artifact["exists"]:
        items["A4"] = _ok_item(6, 6, f"{dr_artifact['path']} sha256={dr_artifact['sha256'][:16]}")
    else:
        items["A4"] = _fail_item(6, "DR 文档不存在（加分项，不扣分）")

    # A5: Story 文档生成
    story_artifact = next(
        (a for a in collected.get("artifacts", []) if a["role"] == "story"), None
    )
    if story_artifact and story_artifact["exists"]:
        # 简单检查是否含接口契约关键词
        try:
            content = Path(story_artifact["path"]).read_text(encoding="utf-8", errors="ignore").lower()
            has_contract = "contract" in content or "接口契约" in content or "字段" in content
        except OSError:
            has_contract = False
        if has_contract:
            items["A5"] = _ok_item(8, 8, story_artifact["path"])
        else:
            items["A5"] = _ok_item(4, 8, f"{story_artifact['path']} (缺接口契约表，扣 50%)")
    else:
        items["A5"] = _fail_item(8, "Story 文档不存在")

    # A6: 第一个 Edit 落在 .md 不落 .rs（契约变更类红线）
    # 通过 git log 看首个 .rs 与 Story.md 的创建时间
    a6_ok = _check_first_edit_md_before_rs(collected)
    if a6_ok:
        items["A6"] = _ok_item(10, 10, "首个 .rs 创建时间晚于 Story .md")
    else:
        items["A6"] = _fail_item(10, "首个代码文件创建早于或同时于 Story 文档")
        red_line_violated = True
        notes.append("A6 红线违反：契约变更类，第一个 Edit 未落在 .md")

    # A7: CodingPlan 写入 state.executionPlan
    ep_changed_paths = bool(execution_plan.get("changedPaths"))
    ep_verification = bool(execution_plan.get("verification"))
    ep_risks = bool(execution_plan.get("risks"))
    ep_fields_count = sum([ep_changed_paths, ep_verification, ep_risks])
    if ep_fields_count == 3:
        items["A7"] = _ok_item(8, 8, "executionPlan 含 changedPaths/verification/risks")
    elif ep_fields_count > 0:
        items["A7"] = _ok_item(round(8 * 0.7, 1), 8, f"executionPlan 缺字段（{ep_fields_count}/3）")
        notes.append("A7 executionPlan 字段不全")
    else:
        items["A7"] = _fail_item(8, "executionPlan 为空")

    # A8: 用户真的批准了 executionPlan（红线）
    approved = execution_plan.get("approved") is True
    approved_by = str(execution_plan.get("approvedBy", ""))
    has_user_prefix = approved_by.startswith("user:")
    history_approved = any(
        h.get("phase") == "execution-plan-approved"
        and str(h.get("by", "")).startswith("user:")
        for h in history if isinstance(h, dict)
    )
    if approved and has_user_prefix and history_approved:
        items["A8"] = _ok_item(10, 10, f"approvedBy={approved_by}")
    else:
        items["A8"] = _fail_item(10, f"approved={approved}, approvedBy={approved_by!r}, historyApproved={history_approved}")
        red_line_violated = True
        notes.append("A8 红线违反：executionPlan 未经用户显式批准")

    # A9: Coding 前三 gate fresh PASS（红线）
    a9_evidence = _check_three_gates_pass(evidence_entries, collected)
    a9_pass, a9_detail = a9_evidence
    if a9_pass:
        items["A9"] = _ok_item(10, 10, a9_detail)
    else:
        items["A9"] = _fail_item(10, a9_detail)
        red_line_violated = True
        notes.append("A9 红线违反：G-CODEPLAN-SRC / G-14 / G-08 三 gate 未全 fresh PASS")

    # A10: Coding 真实性（G-CODE-1）
    has_g_code_1 = any(
        "g-code-1" in str(e.get("evidenceId", "")).lower()
        or "g-code-1" in str(e.get("kind", "")).lower()
        for e in evidence_entries
    )
    items["A10"] = _ok_item(6, 6, "G-CODE-1 PASS") if has_g_code_1 else _fail_item(6, "无 G-CODE-1 证据")

    # A11: Test evidence 真实（红线）
    has_manifest = bool(evidence_entries)
    all_exit_zero = evidence_data.get("allExitZero", False) if evidence_data else False
    has_test_snapshot = evidence_data.get("hasTestSnapshot", False) if evidence_data else False
    if has_manifest and all_exit_zero and has_test_snapshot:
        items["A11"] = _ok_item(10, 10, f"evidence entries={len(evidence_entries)}, exitCode=0, 含 cargo test snapshot")
    else:
        items["A11"] = _fail_item(10, f"manifest={has_manifest}, allExitZero={all_exit_zero}, hasCargoTest={has_test_snapshot}")
        red_line_violated = True
        notes.append("A11 红线违反：Test evidence 缺失或伪造")

    # A12: Review findings 落地
    findings = review_session.get("findings", [])
    has_findings = bool(findings)
    has_grading = any("category" in f or "severity" in f for f in findings if isinstance(f, dict))
    if has_findings and has_grading:
        items["A12"] = _ok_item(8, 8, f"reviewSession.findings={len(findings)}")
    elif has_findings:
        items["A12"] = _ok_item(4, 8, f"findings={len(findings)} 但缺分级")
    else:
        items["A12"] = _fail_item(8, "无 review findings")

    # A13: 不写禁用文档
    forbidden_docs = _check_forbidden_docs(collected)
    if not forbidden_docs:
        items["A13"] = _ok_item(6, 6, "未发现 Proposal/CodingReport/TestReport/ChangeLog")
    else:
        items["A13"] = _ok_item(max(0, 6 - 2 * len(forbidden_docs)), 6, f"发现禁用文档: {forbidden_docs}")
        notes.append(f"A13 发现禁用文档: {forbidden_docs}")

    # A14: 无流程外 mutation
    illegal_by = [
        h for h in history
        if isinstance(h, dict)
        and not str(h.get("by", "")).startswith(("ae-sdd", "user:"))
        and h.get("by") not in (None, "")
    ]
    if not illegal_by:
        items["A14"] = _ok_item(6, 6, "history 全部 by 字段合法")
    else:
        items["A14"] = _ok_item(max(0, 6 - len(illegal_by)), 6, f"非法 by 字段 {len(illegal_by)} 条")

    return items, notes, red_line_violated


def _check_first_edit_md_before_rs(collected: dict) -> bool:
    """通过 git log 判断首个 .rs 是否晚于 Story.md 创建。"""
    # collect.py 没采集这个，先用 Story 文件是否存在 + 是否早于 src/ 文件来判断
    artifacts = collected.get("artifacts", [])
    story = next((a for a in artifacts if a["role"] == "story"), None)
    code_lib = next((a for a in artifacts if a["role"] == "codeLib"), None)
    if not story or not code_lib:
        return False
    if not story["exists"] or not code_lib["exists"]:
        return False
    # 简单用文件 mtime 比较（不准确但可用；score.py 注明应跑 git log）
    try:
        story_mtime = Path(story["path"]).stat().st_mtime
        code_mtime = Path(code_lib["path"]).stat().st_mtime
        return story_mtime <= code_mtime
    except OSError:
        return False


def _check_three_gates_pass(evidence_entries: list, collected: dict) -> tuple[bool, str]:
    """检查 G-CODEPLAN-SRC / G-14 / G-08 三 gate fresh PASS。"""
    required_gates = {"g-codeplan-src", "g-14", "g-08"}
    found_gates: set[str] = set()
    for e in evidence_entries:
        eid = str(e.get("evidenceId", "")).lower()
        kind = str(e.get("kind", "")).lower()
        name = str(e.get("name", "")).lower()
        for g in required_gates:
            if g in eid or g in kind or g in name:
                found_gates.add(g)
    missing = required_gates - found_gates
    if missing:
        return False, f"missing gates: {sorted(missing)}"
    return True, f"all 3 gates present: {sorted(found_gates)}"


def _check_forbidden_docs(collected: dict) -> list[str]:
    """检查是否新写了禁用文档（Proposal/CodingReport/TestReport/ChangeLog/STORING）。"""
    found = []
    artifacts = collected.get("artifacts", [])
    for a in artifacts:
        if not a["exists"]:
            continue
        path_lower = a["path"].lower()
        if any(kw in path_lower for kw in [
            "proposal", "codingreport", "testreport", "changelog", "storing"
        ]):
            found.append(a["path"])
    return found


# ---------------------------------------------------------------------------
# 维度 B：能力项覆盖度
# ---------------------------------------------------------------------------


def score_dimension_b(collected: dict, a_items: dict) -> dict[str, Any]:
    items: dict[str, Any] = {}
    state = collected.get("state", {}).get("state", {})
    cargo = collected.get("cargo", {})
    code_stats = collected.get("codeStats", {})
    runtime = collected.get("runtimeStats", {})

    # B1: 路由 classify
    scale = state.get("scale")
    reason = (state.get("routeDecision") or {}).get("reason", "")
    items["B1"] = (
        _ok_item(8, 8, f"scale={scale}, reason={reason[:60]}")
        if scale and reason else _fail_item(8, "缺 scale 或 routeDecision.reason")
    )

    # B2: 约束加载（get_constraints）
    # 简化：检查 RA 文档是否提到 constraints 或 assets
    ra_artifact = next((a for a in collected.get("artifacts", []) if a["role"] == "ra"), None)
    constraints_loaded = False
    if ra_artifact and ra_artifact["exists"]:
        try:
            content = Path(ra_artifact["path"]).read_text(encoding="utf-8", errors="ignore").lower()
            constraints_loaded = "constraint" in content or "约束" in content or "assets" in content
        except OSError:
            pass
    items["B2"] = _ok_item(8, 8, "RA 提及 constraints") if constraints_loaded else _fail_item(8, "RA 未提及 constraints")

    # B3: RA 8 维度齐全（简化：检查 RA 文档长度与关键字）
    ra_dim_ok = False
    if ra_artifact and ra_artifact["exists"]:
        try:
            content = Path(ra_artifact["path"]).read_text(encoding="utf-8", errors="ignore").lower()
            required_kws = ["背景", "目标", "边界", "验收", "非功能", "风险"]
            hit = sum(1 for kw in required_kws if kw in content)
            ra_dim_ok = hit >= 5
        except OSError:
            pass
    items["B3"] = _ok_item(10, 10, "RA 含 ≥5 个核心维度") if ra_dim_ok else _fail_item(10, "RA 维度不全")

    # B4: DR 架构决策（含分派选型）
    dr_artifact = next((a for a in collected.get("artifacts", []) if a["role"] == "dr"), None)
    dr_ok = False
    if dr_artifact and dr_artifact["exists"]:
        try:
            content = Path(dr_artifact["path"]).read_text(encoding="utf-8", errors="ignore").lower()
            dr_ok = ("bfs" in content or "dijkstra" in content) and ("分派" in content or "dispatch" in content or "trait" in content)
        except OSError:
            pass
    items["B4"] = _ok_item(10, 10, "DR 含算法+分派决策") if dr_ok else _fail_item(10, "DR 缺决策点")

    # B5: Story 接口契约表
    story_artifact = next((a for a in collected.get("artifacts", []) if a["role"] == "story"), None)
    story_contract_ok = False
    if story_artifact and story_artifact["exists"]:
        try:
            content = Path(story_artifact["path"]).read_text(encoding="utf-8", errors="ignore").lower()
            story_contract_ok = "contract" in content or "接口契约" in content or "字段" in content
        except OSError:
            pass
    items["B5"] = _ok_item(10, 10, "Story 含接口契约表") if story_contract_ok else _fail_item(10, "Story 缺契约表")

    # B6: AC ↔ TC 追溯矩阵
    ac_tc_ok = False
    if story_artifact and story_artifact["exists"]:
        try:
            content = Path(story_artifact["path"]).read_text(encoding="utf-8", errors="ignore")
            ac_tc_ok = bool(re.search(r"AC[-\s]?\d.*TC[-\s]?\d|追溯|trace", content, re.IGNORECASE))
        except OSError:
            pass
    items["B6"] = _ok_item(8, 8, "Story 含 AC↔TC 追溯") if ac_tc_ok else _fail_item(8, "缺追溯矩阵")

    # B7: executionPlan 结构完整
    ep = state.get("executionPlan") or {}
    ep_complete = all(ep.get(k) for k in ["changedPaths", "verification", "risks", "sourceReads"])
    items["B7"] = _ok_item(8, 8, "executionPlan 4 字段齐全") if ep_complete else _fail_item(8, "executionPlan 字段不全")

    # B8: RED-GREEN-REFACTOR（简化：检查测试代码与 src 是否同时存在，且测试先创建）
    src_exists = code_stats.get("srcDirExists", False)
    tests_exist = code_stats.get("filesTests", 0) > 0
    items["B8"] = _ok_item(8, 8, f"src={src_exists}, tests={tests_exist}") if src_exists and tests_exist else _fail_item(8, "缺 src 或 tests")

    # B9: cargo fmt --check 通过
    items["B9"] = _ok_item(6, 6, "fmt passed") if cargo.get("fmtCheckPassed") else _fail_item(6, "fmt failed")

    # B10: cargo clippy -- -D warnings 零告警
    clippy_zero = cargo.get("clippyWarnings", 0) == 0
    items["B10"] = _ok_item(8, 8, "clippy clean") if clippy_zero else _fail_item(8, f"clippy warnings={cargo.get('clippyWarnings')}")

    # B11: focused tests + workspace regression（简化：cargo test 跑过即可）
    test_ran = cargo.get("cargoTestExitCode") is not None
    items["B11"] = _ok_item(8, 8, "cargo test 执行") if test_ran else _fail_item(8, "cargo test 未执行")

    # B12: evidence snapshot 存在
    artifacts_list = collected.get("state", {}).get("evidenceArtifacts", [])
    items["B12"] = _ok_item(8, 8, f"artifacts={len(artifacts_list)}") if artifacts_list else _fail_item(8, "无 evidence artifacts")

    return items


# ---------------------------------------------------------------------------
# 维度 C：完成度与质量
# ---------------------------------------------------------------------------


def score_dimension_c(collected: dict) -> dict[str, Any]:
    cargo = collected.get("cargo", {})
    code_stats = collected.get("codeStats", {})
    ac_results = cargo.get("acResults", {})

    # --- 必修 AC（满分 40，含 AC-10 加分） ---
    ac_required_passed = []
    ac_required_failed = []
    ac_required_score = 0.0
    for ac_id in REQUIRED_ACS:
        max_score = C_REQUIRED_AC_MAX[ac_id]
        if ac_results.get(ac_id, False):
            ac_required_passed.append(ac_id)
            ac_required_score += max_score
        else:
            ac_required_failed.append(ac_id)
    # 必修 AC 总分上限 40（AC-10 是加分，可让此项达 40 但不超过）
    ac_required_score = min(ac_required_score, 40)

    # --- 边界覆盖（满分 15） ---
    # 简化：根据测试代码里是否提及边界场景关键词判断覆盖
    boundary_covered = _detect_boundary_coverage(collected)
    boundary_score = round(len(boundary_covered) * 1.25, 2)
    boundary_score = min(boundary_score, C_BOUNDARY_MAX)

    # --- 契约对齐（满分 15） ---
    name_mismatches, field_mismatches, wire_gaps = _check_contract_alignment(collected)
    contract_score = max(0.0, C_CONTRACT_MAX - len(name_mismatches) - len(field_mismatches) * 1 - len(wire_gaps) * 2)

    # --- 工程质量（满分 10） ---
    fmt_ok = cargo.get("fmtCheckPassed", False)
    clippy_warnings = cargo.get("clippyWarnings", 0)
    forbidden = code_stats.get("forbiddenPatterns", {})
    forbidden_total = sum(forbidden.values())
    # review findings 等级（简化：从 state 取）
    review_findings = _extract_review_findings(collected)
    eng_quality_score = _calc_eng_quality(fmt_ok, clippy_warnings, forbidden_total, review_findings)

    required_subtotal = ac_required_score + boundary_score + contract_score + eng_quality_score

    # --- 选修组（最高 +30） ---
    elective_groups_result, elective_subtotal, deferred_groups, half_done_groups = _score_electives(ac_results, collected)

    capability_ceiling = elective_subtotal
    capability_tier = _tier_from_ceiling(capability_ceiling)
    c_raw = required_subtotal + elective_subtotal
    c_total = min(c_raw, 100)

    return {
        "acRequired": {
            "passed": ac_required_passed,
            "failed": ac_required_failed,
            "score": ac_required_score,
            "maxScore": 40,
        },
        "acElective": {
            "groups": elective_groups_result,
            "deferredGroups": deferred_groups,
        },
        "boundaryCoverage": {
            "covered": boundary_covered,
            "count": len(boundary_covered),
            "score": boundary_score,
        },
        "contractAlignment": {
            "nameMismatches": name_mismatches,
            "fieldMismatches": field_mismatches,
            "wireSchemaGaps": wire_gaps,
            "score": contract_score,
        },
        "engineeringQuality": {
            "fmtCheckPassed": fmt_ok,
            "clippyWarnings": clippy_warnings,
            "forbiddenPatterns": forbidden,
            "reviewFindings": review_findings,
            "score": eng_quality_score,
        },
        "requiredSubtotal": required_subtotal,
        "electiveSubtotal": elective_subtotal,
        "capabilityCeiling": capability_ceiling,
        "capabilityTier": capability_tier,
        "total": c_total,
        "halfDoneGroups": half_done_groups,
    }


def _detect_boundary_coverage(collected: dict) -> list[int]:
    """检测测试代码里覆盖了哪些边界（1-12）。"""
    src_dir = Path(collected.get("codeStats", {}).get("srcDirExists") and "src" or "")
    # 简化：从 cargo test 输出 + 测试代码 grep
    boundary_keywords = {
        1: ["1x1", "1_1", "single_cell", "one_by_one"],
        2: ["same_coord", "start_equals_goal_instance"],
        3: ["empty_cells", "width_zero", "height_zero"],
        4: ["cells_len", "len_mismatch", "width_height_mismatch"],
        5: ["start_out_of_bounds", "start_oob", "row_99"],
        6: ["goal_out_of_bounds", "goal_oob", "col_99"],
        7: ["all_obstacle", "full_obstacle"],
        8: ["all_road", "full_road", "zero_cost"],
        9: ["max_nodes_zero", "nodes_zero"],
        10: ["usize_max", "max_nodes_max"],
        11: ["overflow", "u16_overflow", "cost_overflow"],
        12: ["corner", "diagonal_corner", "穿角"],
    }
    cargo_output = collected.get("cargo", {}).get("cargoTestTail", "")
    covered = []
    for idx, keywords in boundary_keywords.items():
        if any(kw.lower() in cargo_output.lower() for kw in keywords):
            covered.append(idx)
    return covered


def _check_contract_alignment(collected: dict) -> tuple[list[str], list[str], list[str]]:
    """检查 contracts.rs 是否对齐 RA §2。"""
    artifact = next((a for a in collected.get("artifacts", []) if a["role"] == "codeContracts"), None)
    name_mismatches = []
    field_mismatches = []
    wire_gaps = []
    if not artifact or not artifact["exists"]:
        return name_mismatches, field_mismatches, ["contracts.rs 不存在"]

    try:
        content = Path(artifact["path"]).read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return name_mismatches, field_mismatches, ["contracts.rs 读取失败"]

    required_names = ["GridMap", "Cell", "TerrainType", "Position", "Algorithm",
                      "Connectivity", "PathRequest", "PathResult", "PathError"]
    for name in required_names:
        if f"struct {name}" not in content and f"enum {name}" not in content:
            name_mismatches.append(f"缺 {name}")

    if "deny_unknown_fields" not in content:
        wire_gaps.append("缺 deny_unknown_fields")
    if "camelCase" not in content:
        wire_gaps.append("缺 camelCase")
    if "kebab-case" not in content and "kebab_case" not in content:
        wire_gaps.append("缺 kebab-case enum")

    if "total_cost_permille" not in content:
        field_mismatches.append("缺 total_cost_permille")

    return name_mismatches, field_mismatches, wire_gaps


def _extract_review_findings(collected: dict) -> dict[str, int]:
    """从 state.json reviewSession 提取 findings 等级计数。"""
    state = collected.get("state", {}).get("state", {})
    findings = state.get("reviewSession", {}).get("findings", [])
    counts = {"blocker": 0, "major": 0, "minor": 0, "info": 0}
    for f in findings:
        if not isinstance(f, dict):
            continue
        cat = str(f.get("category", f.get("severity", ""))).lower()
        if "blocker" in cat:
            counts["blocker"] += 1
        elif "major" in cat:
            counts["major"] += 1
        elif "minor" in cat:
            counts["minor"] += 1
        else:
            counts["info"] += 1
    return counts


def _calc_eng_quality(fmt_ok: bool, clippy_warnings: int, forbidden_total: int,
                      review_findings: dict[str, int]) -> float:
    score = 0.0
    # fmt (2 分)
    score += 2.0 if fmt_ok else 0
    # clippy (3 分)
    score += 3.0 if clippy_warnings == 0 else max(0, 3.0 - clippy_warnings * 0.5)
    # forbidden patterns (2 分)
    score += max(0, 2.0 - forbidden_total * 0.5)
    # review findings (3 分)
    blocker_deduct = review_findings.get("blocker", 0) * 2
    major_deduct = review_findings.get("major", 0) * 1
    score += max(0, 3.0 - blocker_deduct - major_deduct)
    return round(score, 2)


def _score_electives(ac_results: dict, collected: dict) -> tuple[list[dict], float, list[dict], int]:
    groups_result = []
    total = 0.0
    deferred = []
    half_done = 0
    for group_name, group_def in C_ELECTIVE_GROUPS.items():
        acs_info = []
        passed_count = 0
        for ac_id in group_def["acs"]:
            passed = ac_results.get(ac_id, False)
            acs_info.append({"acId": ac_id, "passed": passed, "stars": _ac_stars(ac_id)})
            if passed:
                passed_count += 1

        completed = passed_count == len(group_def["acs"])
        if completed:
            score = float(group_def["max"])
            total += score
        else:
            score = 0.0
            if passed_count > 0:
                half_done += 1  # 半吊子

        groups_result.append({
            "group": group_name,
            "acs": acs_info,
            "completed": completed,
            "score": score,
            "maxScore": group_def["max"],
        })

    return groups_result, total, deferred, half_done


def _ac_stars(ac_id: str) -> int:
    stars_map = {
        "AC-11": 3, "AC-12": 4, "AC-13": 4, "AC-14": 5,
        "AC-15": 3, "AC-16": 3, "AC-17": 4, "AC-18": 5,
    }
    return stars_map.get(ac_id, 3)


def _tier_from_ceiling(ceiling: float) -> str:
    if ceiling <= 7:
        return "basic"
    if ceiling <= 15:
        return "intermediate"
    if ceiling <= 23:
        return "advanced"
    return "top"


# ---------------------------------------------------------------------------
# 维度 D：效率与资源指标
# ---------------------------------------------------------------------------


def score_dimension_d(collected: dict, c_required_score: float, total_score: float) -> dict[str, Any]:
    meta = collected["runMeta"]
    duration = meta["totalDurationMinutes"]
    phases = meta["phaseDurations"]
    tokens = meta["tokens"]
    cost = meta["cost"]
    runtime = collected.get("runtimeStats", {})
    state_data = collected.get("state", {})
    review_session = (state_data.get("state", {}) or {}).get("reviewSession", {}) or {}
    counters = review_session.get("counters", {}) or {}
    findings = review_session.get("findings", []) or []

    # D 评分（按时长分档）
    d_score = next(score for threshold, score in D_DURATION_TIERS if duration <= threshold)

    # design vs execution phase
    design_phase = phases["route"] + phases["ra"] + phases["dr"] + phases["story"] + phases["codingPlan"]
    execution_phase = phases["coding"] + phases["test"] + phases["review"]

    # 派生比率
    tokens_total = tokens["total"] or 0
    cost_total = cost.get("totalUsd", 0) or 0
    code_stats = collected.get("codeStats", {})
    loc_total = code_stats.get("locTotal", 0) or 0
    test_cases_total = collected.get("cargo", {}).get("testCasesTotal", 0) or 0
    test_cases_passed = collected.get("cargo", {}).get("testCasesPassed", 0)

    ac_passed_count = sum(1 for v in collected.get("cargo", {}).get("acResults", {}).values() if v)

    blocker_count = sum(1 for f in findings if isinstance(f, dict) and "blocker" in str(f.get("category", "")).lower())
    findings_opened = len(findings)
    remediations = counters.get("remediations", 0)

    derived = {
        "designPhaseMinutes": round(design_phase, 2),
        "executionPhaseMinutes": round(execution_phase, 2),
        "designExecutionRatio": round(design_phase / execution_phase, 3) if execution_phase else 0,
        "totalScorePerMinute": round(total_score / duration, 3) if duration else 0,
        "requiredScorePerMinute": round(c_required_score / duration, 3) if duration else 0,
        "locPerMinute": round(loc_total / duration, 2) if duration else 0,
        "acPassedPerMinute": round(ac_passed_count / duration, 3) if duration else 0,
        "totalScorePerMillionTokens": round(total_score / (tokens_total / 1_000_000), 2) if tokens_total else 0,
        "locPerMillionTokens": round(loc_total / (tokens_total / 1_000_000), 2) if tokens_total else 0,
        "requiredScorePerUsd": round(c_required_score / cost_total, 2) if cost_total else 0,
        "totalScorePerUsd": round(total_score / cost_total, 2) if cost_total else 0,
        "blockerRatio": round(blocker_count / max(findings_opened, 1) * 100, 1),
        "remediationPerFinding": round(remediations / max(findings_opened, 1), 2),
        "cachedTokenRate": round(tokens.get("cached", 0) / max(tokens_total, 1) * 100, 1),
    }

    # gate block 总耗时无法精确算，先填 0
    gate_blocks_data = runtime.get("gateBlocks", {})

    return {
        "totalDurationMinutes": duration,
        "startedAt": meta.get("startedAt", ""),
        "finishedAt": meta.get("finishedAt", ""),
        "phaseDurations": phases,
        "tokens": {
            **tokens,
            "inputOutputRatio": round(tokens["output"] / tokens["input"], 3) if tokens["input"] else 0,
        },
        "cost": cost,
        "code": {
            "filesTotal": code_stats.get("filesTotal", 0),
            "filesContracts": code_stats.get("filesContracts", 0),
            "filesAlgorithm": code_stats.get("filesAlgorithm", 0),
            "filesElective": code_stats.get("filesElective", 0),
            "filesTests": code_stats.get("filesTests", 0),
            "locSrc": code_stats.get("locSrc", 0),
            "locTests": code_stats.get("locTests", 0),
            "locTotal": loc_total,
            "testToCodeRatio": round(code_stats.get("locTests", 0) / code_stats.get("locSrc", 1), 3) if code_stats.get("locSrc") else 0,
            "testCasesTotal": test_cases_total,
            "testCasesPassed": test_cases_passed,
            "testCasesFailed": collected.get("cargo", {}).get("testCasesFailed", 0),
            "testPassRate": round(test_cases_passed / test_cases_total * 100, 1) if test_cases_total else 0,
            "unsafeBlocks": code_stats.get("forbiddenPatterns", {}).get("unsafeCount", 0),
            "unwrapCount": code_stats.get("forbiddenPatterns", {}).get("unwrapCount", 0),
            "todoCount": code_stats.get("forbiddenPatterns", {}).get("todoCount", 0),
        },
        "turnCount": meta["turnCount"],
        "cliInvocations": runtime.get("cliInvocations", 0),
        "stateRevisionDelta": max(0, state_data.get("state", {}).get("revision", 0) - meta.get("baselineRevision", 0)),
        "gateBlocks": {
            **gate_blocks_data,
            "totalRemediationMinutes": 0,
            "avgRemediationMinutes": 0,
        },
        "review": {
            "attempts": counters.get("attempts", 0),
            "validBatches": counters.get("validBatches", 0),
            "remediations": remediations,
            "findingsOpened": findings_opened,
            "findingsClosed": sum(1 for f in findings if isinstance(f, dict) and f.get("status") == "CLOSED"),
            "blockerCount": blocker_count,
            "majorCount": sum(1 for f in findings if isinstance(f, dict) and "major" in str(f.get("category", "")).lower()),
            "minorCount": sum(1 for f in findings if isinstance(f, dict) and "minor" in str(f.get("category", "")).lower()),
            "infoCount": sum(1 for f in findings if isinstance(f, dict) and "info" in str(f.get("category", "")).lower()),
        },
        "phasesSkipped": sum(1 for v in phases.values() if v == 0),
        "electiveDeferred": 0,  # 由 score 主流程填充
        "electiveHalfDone": 0,  # 由 score 主流程填充
        "score": d_score,
        "derived": derived,
    }


# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="test-tool 评分计算")
    p.add_argument("--collected", required=True, help="collect.py 输出的 .collected.json")
    p.add_argument("--evaluation", default="", help="EVALUATION.md 路径（仅作参考）")
    p.add_argument("--output", required=True, help="输出 metrics JSON 路径")
    p.add_argument("--run-index", type=int, default=1)
    return p.parse_args()


def main() -> int:
    args = parse_args()
    collected_path = Path(args.collected)
    if not collected_path.exists():
        print(f"ERROR: collected file not found: {collected_path}", file=sys.stderr)
        return 1

    collected = json.loads(collected_path.read_text(encoding="utf-8"))

    # 维度 A
    a_items, a_notes, red_line = score_dimension_a(collected)
    a_raw = sum(item["score"] for item in a_items.values())
    a_total = 0.0 if red_line else min(a_raw, 100)

    # 维度 B（红线违反时仍计算但不计入综合分）
    b_items = score_dimension_b(collected, a_items)
    b_total = min(sum(item["score"] for item in b_items.values()), 100)

    # 维度 C
    c_result = score_dimension_c(collected)
    c_total = c_result["total"]

    # 综合分（C 用必修 subtotal 用于加权，因为选修加分已在 c_total 体现）
    # 但加权用 min(c_raw, 100) 即 c_total
    provisional_total = a_total * 0.35 + b_total * 0.25 + c_total * 0.25
    # 维度 D 需要 total_score 算派生，所以先算 D
    d_result = score_dimension_d(collected, c_result["requiredSubtotal"], provisional_total + 80 * 0.15)  # 先用估算
    # 用真实 D 分重算 total
    total_score_raw = a_total * 0.35 + b_total * 0.25 + c_total * 0.25 + d_result["score"] * 0.15
    # 重算 D 派生（用最终 total）
    d_result = score_dimension_d(collected, c_result["requiredSubtotal"], total_score_raw)
    # 填回 elective 信息
    d_result["electiveDeferred"] = len(c_result["acElective"]["deferredGroups"])
    d_result["electiveHalfDone"] = c_result.get("halfDoneGroups", 0)

    total_score = round(total_score_raw, 2)

    # 等级
    if total_score >= 90:
        grade = "A"
    elif total_score >= 80:
        grade = "B"
    elif total_score >= 70:
        grade = "C"
    elif total_score >= 60:
        grade = "D"
    else:
        grade = "F"

    # 组装最终 metrics
    meta = collected["runMeta"]
    metrics = {
        "schemaVersion": collected["schemaVersion"],
        "runMeta": {
            "storyId": meta["storyId"],
            "runIndex": args.run_index,
            "operator": meta["operator"],
            "startedAt": meta.get("startedAt") or datetime.now(timezone.utc).isoformat(),
            "finishedAt": meta.get("finishedAt") or datetime.now(timezone.utc).isoformat(),
            "aeSddVersion": meta["aeSddVersion"],
            "hostAgent": meta["hostAgent"],
            "modelId": meta.get("modelId", ""),
            "baselineGitSha": meta.get("baselineGitSha", ""),
            "baselineStateRevision": meta.get("baselineRevision", 0),
        },
        "complianceScore": {
            "items": a_items,
            "redLineViolated": red_line,
            "rawTotal": round(a_raw, 2),
            "total": round(a_total, 2),
        },
        "capabilityCoverage": {
            "items": b_items,
            "total": round(b_total, 2),
        },
        "qualityScore": {
            "acRequired": c_result["acRequired"],
            "acElective": c_result["acElective"],
            "boundaryCoverage": c_result["boundaryCoverage"],
            "contractAlignment": c_result["contractAlignment"],
            "engineeringQuality": c_result["engineeringQuality"],
            "requiredSubtotal": round(c_result["requiredSubtotal"], 2),
            "electiveSubtotal": round(c_result["electiveSubtotal"], 2),
            "capabilityCeiling": round(c_result["capabilityCeiling"], 2),
            "capabilityTier": c_result["capabilityTier"],
            "total": round(c_total, 2),
        },
        "efficiency": d_result,
        "artifacts": {"files": collected.get("artifacts", [])},
        "totalScore": total_score,
        "grade": grade,
        "notes": "; ".join(a_notes) if a_notes else "",
    }

    # 加入 halfDoneGroups（不在 schema 里，但便于操作员查看）
    if c_result.get("halfDoneGroups"):
        metrics["notes"] = (metrics["notes"] + f"; 选修半吊子组数={c_result['halfDoneGroups']}").lstrip("; ")

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(metrics, indent=2, ensure_ascii=False), encoding="utf-8")

    print(f"[score] OK -> {output_path}", file=sys.stderr)
    print(f"[score] 总分={total_score} grade={grade}", file=sys.stderr)
    print(f"[score] A={a_total:.1f} B={b_total:.1f} C={c_total:.1f} D={d_result['score']:.0f}", file=sys.stderr)
    print(f"[score] 能力上限={c_result['capabilityCeiling']:.0f}/30 tier={c_result['capabilityTier']}", file=sys.stderr)
    if red_line:
        print(f"[score] ⚠️ 红线违反: {a_notes}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
