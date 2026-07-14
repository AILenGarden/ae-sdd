"""
flow_monitor.py — 主流程监管器：偏移检测与矫正消息生成 v1.0（🆕 v3.6）

决策背景（见 source/docs/ae-sdd-design.md §流程偏移检测与矫正）：
  决策 1B：废弃 ◆ STATE: 自报标记，完全依赖产物核查（gates check）
  决策 2B：流程监管器实体 = UserPromptSubmit hook Python 逻辑
  决策 3：paused 新增为一级 phase，Level 3 暂停时写入

漂移类型（B1~B4）：
  B1 跳步漂移：AI 跳过 Review 宣布进入下一系列
  B2 停滞漂移：同一 phase 经过 N 轮，产物始终未通过 gates check
  B3 伪完成漂移：声称完成某阶段但产物门禁未通过（原依赖 ◆ STATE: 自报，v3.6 改产物核查）
  B4 旁路漂移：题外话后 AI 未回到流程（轮次计数检测）

矫正级别：
  Level 1（severity=1）：静默注入，AI 可感知，用户不可见；首次偏移提醒
  Level 2（severity=2）：矫正提示词，AI 收到后须说明修复计划；同一步骤最多 3 次
  Level 3（severity=3）：人工升级，state.phase=paused，流程暂停待用户决策

设计原则：
  - 本模块是纯计算模块，不直接写 state.json（写 state 由 prompt_inject.py 调用 state API 执行）
  - 全流程 try/except，任何异常降级返回 DriftResult(drift_type="none")，不阻断主流程
  - gates check 超时统一 10s，超时降级放行（不误判为偏移）
"""
from __future__ import annotations

import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

from lib import state as state_mod

# ─── 阈值常量 ────────────────────────────────────────────────────────────────
# 矫正次数 >= WARN：停滞告警，severity 升为 2
CORRECTION_THRESHOLD_WARN = 5
# 矫正次数 >= PAUSE：触发 Level 3 暂停
CORRECTION_THRESHOLD_PAUSE = 3
# gates check subprocess 超时（秒）
GATES_CHECK_TIMEOUT = 10


# ─── 数据结构 ─────────────────────────────────────────────────────────────────

@dataclass
class DriftResult:
    """偏移检测结果。

    drift_type:       "none" | "fake-complete" | "stagnation" | "skip-step" | "off-topic"
    severity:         0=无偏移  1=Level1静默  2=Level2矫正  3=Level3暂停
    gate_id:          触发检测的 gate_id（可为空）
    gate_passed:      gates check 是否通过
    gate_message:     gates check 原始输出（截断到 500 字符）
    phase:            检测时的当前 phase
    correction_count: 当前 phase 的历史矫正次数（来自 state.correctionCounts）
    message:          矫正注入文本（由 build_correction_message() 填充后写入）
    """
    drift_type: str
    severity: int
    gate_id: str
    gate_passed: bool
    gate_message: str
    phase: str
    correction_count: int
    message: str = field(default="")


# ─── phase → gate_id 映射 ────────────────────────────────────────────────────

def get_phase_gate_map() -> dict[str, list[str]]:
    """返回 phase -> 应检查的 gate_id 列表映射。

    每轮 UserPromptSubmit 时依据当前 phase 确定要校验哪些门禁，
    以产物是否合规作为"本阶段是否真正完成"的唯一判据（决策 1B）。

    未列出的 phase（initialized / paused / completed）不做产物核查。

    🆕 v3.10.4 双源对齐：本映射是漂移检测用的轻量代表 gate（每 phase 1 个），
    不是切相门禁全量表（那是 gate_intercept.PHASE_ENTRY_GATES）。两者必须不矛盾：
    本表选取的 gate 必须是 PHASE_ENTRY_GATES 对应 phase 的子集，否则会与切相
    门禁判定冲突（如旧 code-reviewed->G-05 在 PHASE_ENTRY_GATES 里已摘除，但本表
    仍残留，导致漂移检测每轮误报 G-05 失败 -- Pbl.md 问题1 根因）。
    v3.10 砍 Task 后：task-generated/task-reviewed 已废弃，改为 testcase 系列；
    code-reviewed 代表 gate 从 G-05 改为 G-09（与 PHASE_ENTRY_GATES 对齐）。
    """
    return {
        "ra-generated":        ["G-RA-1", "G-RA-2", "G-RA-3", "G-RA-4"],
        "dr-generated":        ["G-01"],
        "story-generated":     ["G-02"],
        "story-reviewed":      ["G-03"],
        # 🆕 v3.10：testcase 系列替代 task 系列（砍 Task）
        "testcase-generated":  ["G-04"],
        "testcase-reviewed":   ["G-04"],
        "coding-process":      ["G-08"],   # CodingPlan 文档存在且 14 门禁全过
        "coding":              ["G-CODEPLAN-SRC"],
        # 🆕 v3.10.4：G-05（Task 文档存在）已从 PHASE_ENTRY_GATES 摘除，改用 G-09
        # （测试通过）作为 code-reviewed 的漂移检测代表，消除双源不一致。
        "code-reviewed":       ["G-09"],
        "test-running":        ["G-09"],
    }


# ─── CLI 查找 ─────────────────────────────────────────────────────────────────

def _find_ae_sdd_cli(ade_sdd: Path) -> str:
    """查找 ae-sdd CLI 可执行路径。

    优先顺序：
      1. PATH 中的 ae-sdd
      2. ade_sdd 上溯找 tools/bin/ae-sdd（源码开发环境）
      3. 最终 fallback "ae-sdd"（subprocess 自行报错，降级放行）
    """
    found = shutil.which("ae-sdd")
    if found:
        return found
    # 开发环境：从 .ae-sdd/ 上溯找 tools/bin/ae-sdd
    cur = ade_sdd
    for _ in range(6):
        candidate = cur / "tools" / "bin" / "ae-sdd"
        if candidate.is_file():
            return str(candidate)
        cur = cur.parent
    return "ae-sdd"


def _run_gates_check(gate_id: str, ade_sdd: Path) -> tuple[bool, str]:
    """执行 ae-sdd gates check --only {gate_id}，返回 (passed, message)。

    超时 / 异常 → 返回 (True, "..., 降级放行") 避免误判为偏移。
    """
    cli = _find_ae_sdd_cli(ade_sdd)
    try:
        result = subprocess.run(
            [cli, "gates", "check", "--only", gate_id],
            capture_output=True,
            text=True,
            timeout=GATES_CHECK_TIMEOUT,
            cwd=str(ade_sdd.parent),
            encoding="utf-8",
            errors="replace",
        )
        passed = result.returncode == 0
        msg = (result.stdout or result.stderr or "").strip()
        return passed, msg[:500]  # 截断避免超长注入
    except subprocess.TimeoutExpired:
        return True, f"timeout({GATES_CHECK_TIMEOUT}s)，降级放行"
    except FileNotFoundError:
        return True, "ae-sdd CLI 未找到，降级放行"
    except Exception as e:
        return True, f"exception({type(e).__name__}: {e})，降级放行"


# ─── 核心检测逻辑 ─────────────────────────────────────────────────────────────

def detect_drift(state: dict, ade_sdd: Path) -> DriftResult:
    """检测当前流程是否偏移。

    Layer 1 产物核查（B1/B3）：
        依据 get_phase_gate_map() 跑 gates check，不通过即判定漂移。
        不信任 AI 自报（决策 1B：◆ STATE: 自报已废弃）。

    Layer 3 矫正次数升级（B2/B4）：
        correctionCounts[phase] >= CORRECTION_THRESHOLD_PAUSE → Level 3 升级。

    设计：全流程 try/except，任何异常返回 drift_type="none" 降级放行。

    Args:
        state:    read_state() 返回的 dict（含 correctionCounts 字段）
        ade_sdd:  .ae-sdd/ 目录路径，用于 gates check 时确定工作目录

    Returns:
        DriftResult；drift_type="none" + severity=0 表示无偏移
    """
    try:
        phase = state_mod.get_active_phase(state) or state.get("phase", "initialized")

        # paused / initialized / completed 不做产物核查
        if phase in ("paused", "initialized", "completed"):
            return DriftResult(
                drift_type="none", severity=0, gate_id="", gate_passed=True,
                gate_message="", phase=phase, correction_count=0,
            )

        gate_map = get_phase_gate_map()
        gate_ids = gate_map.get(phase, [])

        # 该 phase 无对应 gate → 不检测，让流程自然推进
        if not gate_ids:
            return DriftResult(
                drift_type="none", severity=0, gate_id="", gate_passed=True,
                gate_message="", phase=phase, correction_count=0,
            )

        # 读矫正次数（来自 state.correctionCounts，不存在默认 0）
        correction_count = state.get("correctionCounts", {}).get(phase, 0)

        # Layer 1：产物核查——检查第一个不通过的 gate
        failing_gate_id = ""
        failing_gate_msg = ""
        for gate_id in gate_ids:
            passed, msg = _run_gates_check(gate_id, ade_sdd)
            if not passed:
                failing_gate_id = gate_id
                failing_gate_msg = msg
                break

        if not failing_gate_id:
            # 全部 gate 通过 → 无偏移
            return DriftResult(
                drift_type="none", severity=0, gate_id="", gate_passed=True,
                gate_message="", phase=phase, correction_count=correction_count,
            )

        # Layer 3：矫正次数已达阈值 → Level 3 暂停
        if correction_count >= CORRECTION_THRESHOLD_PAUSE:
            return DriftResult(
                drift_type="stagnation",
                severity=3,
                gate_id=failing_gate_id,
                gate_passed=False,
                gate_message=failing_gate_msg,
                phase=phase,
                correction_count=correction_count,
            )

        # 普通产物核查失败 → severity 由次数决定
        severity = 2 if correction_count >= 1 else 1
        return DriftResult(
            drift_type="fake-complete",
            severity=severity,
            gate_id=failing_gate_id,
            gate_passed=False,
            gate_message=failing_gate_msg,
            phase=phase,
            correction_count=correction_count,
        )

    except Exception:
        # 任何异常：降级放行，不误判为偏移
        phase = state.get("phase", "unknown") if isinstance(state, dict) else "unknown"
        return DriftResult(
            drift_type="none", severity=0, gate_id="", gate_passed=True,
            gate_message="exception: degraded to pass", phase=phase, correction_count=0,
        )


def should_escalate(state: dict) -> bool:
    """判断当前 phase 的矫正次数是否已达 Level 3 升级阈值。

    供 prompt_inject.py 在写 state 前快速判断是否需要 pause_state()。
    """
    phase = state_mod.get_active_phase(state) or state.get("phase", "initialized")
    count = state.get("correctionCounts", {}).get(phase, 0)
    return count >= CORRECTION_THRESHOLD_PAUSE


# ─── 矫正消息生成 ─────────────────────────────────────────────────────────────

_DRIFT_TYPE_CN: dict[str, str] = {
    "fake-complete": "伪完成漂移",
    "stagnation":    "停滞漂移",
    "skip-step":     "跳步漂移",
    "off-topic":     "旁路漂移",
}

_PHASE_CN: dict[str, str] = {
    "ra-generated":    "RA 需求分析",
    "dr-generated":    "DR 生成",
    "story-generated": "Story 生成",
    "story-reviewed":  "Story Review",
    "task-generated":  "Task 生成",
    "task-reviewed":   "Task Review",
    "coding-process":  "CodingProcess",
    "coding":          "Coding",
    "code-reviewed":   "CodeReview",
    "test-running":    "测试",
    "completed":       "已完成",
    "paused":          "已暂停",
    "initialized":     "初始化",
}


def build_correction_message(drift: DriftResult) -> str:
    """根据 DriftResult 生成矫正注入文本。

    severity=1 → Level 1 静默注入（追加到 additionalContext，用户不可见）
    severity=2 → Level 2 矫正提示词（AI 必须说明修复计划）
    severity=3 → Level 3 暂停文本（含用户决策选项）
    severity=0 / drift_type="none" → 返回空串

    本函数只生成文本，不写 state.json。
    写 state（pause_state）由 prompt_inject.py 调用 state API 执行。
    """
    if drift.severity == 0 or drift.drift_type == "none":
        return ""

    drift_cn = _DRIFT_TYPE_CN.get(drift.drift_type, drift.drift_type)
    phase_cn = _PHASE_CN.get(drift.phase, drift.phase)
    gate_detail = drift.gate_message[:300] if drift.gate_message else "（无详情）"

    if drift.severity == 1:
        return (
            f"[ae-sdd flow-monitor] ⚠️ 流程提示（Level 1）：\n"
            f"当前 phase={drift.phase}（{phase_cn}），产物核查未通过（{drift.gate_id}）。\n"
            f"请确保本阶段产物完整合规后再推进到下一 phase。\n"
            f"（首次偏移提醒，自动矫正中）"
        )

    if drift.severity == 2:
        return (
            f"【主流程监管器 🔴 矫正 — Level 2】\n"
            f"检测到{drift_cn}：{phase_cn} 阶段产物核查未通过。\n"
            f"失败门禁：{drift.gate_id}\n"
            f"门禁详情：{gate_detail}\n"
            f"当前有效 phase 仍为：{drift.phase}\n"
            f"矫正次数：{drift.correction_count}/{CORRECTION_THRESHOLD_PAUSE}（达 {CORRECTION_THRESHOLD_PAUSE} 次将暂停）\n"
            f"\n"
            f"请继续完成 {phase_cn} 阶段，补齐 {drift.gate_id} 要求的产物。\n"
            f"完成后重新跑 gates check 通过才能推进到下一 phase。"
        )

    if drift.severity == 3:
        return (
            f"【主流程监管器 ⏸️ 暂停 — Level 3 人工干预】\n"
            f"检测到{drift_cn}：{phase_cn} 阶段已矫正 {drift.correction_count} 次，仍未收敛。\n"
            f"失败门禁：{drift.gate_id}\n"
            f"门禁详情：{gate_detail}\n"
            f"\n"
            f"流程已暂停（state.phase → paused），请决策：\n"
            f"  · 继续：修复 {drift.gate_id} 产物后说「继续流程」\n"
            f"  · 跳过：说「跳过 {drift.phase} 阶段，进入下一步」（风险自担）\n"
            f"  · 回退：说「回退到上一步重做」\n"
            f"  · 查看：说「查看 {drift.gate_id} 详细失败原因」"
        )

    return ""
