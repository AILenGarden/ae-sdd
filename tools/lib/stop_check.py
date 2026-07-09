"""
stop_check.py — Stop hook v2.0（🆕 v3.6 精简）

v2.0 变更（2026-06-30，决策 1B）：
  - 废弃 ◆ STATE: / ◆ LOADED: 自报标记检测（"防君子不防小人，可谎报"，已由 flow_monitor 产物核查替代）
  - 废弃 _verify_gate_claims()（gate 自报交叉验证）、_check_loaded_marker()（LOADED 标记）
  - check_output() 职责简化为：
      1. 检测空响应 / 结构性错误（AI 截断）→ 重试
      2. 检测 PRD compact 失败（HS-8，保留）
      3. 人工审核点对话呈现格式粗筛（自动化模式跳过）→ 重试
      4. 其余情况放行（流程合规性已由 UserPromptSubmit hook + flow_monitor 接管）
  - 保留：MAX_RETRY 防无限循环 / extract_last_assistant_text() 工具函数

v1.4 变更（2026-06-27）：
  - CLI 入口优先使用 Claude Code Stop hook 的 last_assistant_message 字段
  - last_assistant_message 不存在时，保留 transcript_path 解析作为旧版回退

v1.3 变更（2026-06-22）：
  - 修复历史状态头误判：只检查 transcript 最后一段 AI 响应，不全文搜索
  - 支持多种 transcript 格式（JSONL / 纯文本带角色标记 / 回退全文）
"""
from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Optional

# 防无限循环：最大重试次数
MAX_RETRY = 2

# 重试计数文件名（放在 .ae-sdd/ 下）
_RETRY_FILE_NAME = ".stop_retry_count"

# 人工审核点格式校验只在响应显式进入审核点时触发，避免普通 phase 输出误伤。
_REVIEW_POINT_PHASES: dict[str, set[str]] = {
    "1": {"story-generated", "story-reviewed", "testcase-reviewed"},
    "1.5": {"story-reviewed", "testcase-reviewed"},
    "2": {"task-generated", "task-reviewed"},
    "2.5": {"task-reviewed", "coding-process"},
    "4": {"code-reviewed"},
}

_REVIEW_POINT_MARKER_RE = re.compile(
    r"(?:审核点|人工审核点|review\s*point)\s*(1\.5|2\.5|1|2|4)(?![\d.])",
    re.IGNORECASE,
)


def _retry_file_path(ade_sdd: Path) -> Path:
    return ade_sdd / _RETRY_FILE_NAME


def get_retry_count(ade_sdd: Path) -> int:
    """读取当前重试计数"""
    try:
        return int(_retry_file_path(ade_sdd).read_text(encoding="utf-8").strip())
    except (FileNotFoundError, ValueError):
        return 0


def increment_retry(ade_sdd: Path) -> int:
    """递增重试计数，返回新值"""
    count = get_retry_count(ade_sdd) + 1
    _retry_file_path(ade_sdd).write_text(str(count), encoding="utf-8")
    return count


def reset_retry(ade_sdd: Path) -> None:
    """重置重试计数（由 UserPromptSubmit hook 在对话开始时调用）"""
    try:
        _retry_file_path(ade_sdd).unlink(missing_ok=True)
    except OSError:
        pass


def extract_last_assistant_text(transcript_content: str) -> str:
    """
    从 transcript 中提取最后一段 AI（assistant）响应文本。

    支持三种格式，按优先级尝试：
      1. JSONL：每行一个 JSON 对象，{"role":"assistant","content":"..."}
      2. 纯文本带角色标记：[ASSISTANT] / Assistant: / <assistant>
      3. 回退：返回整个 transcript 末尾 2000 字符（兜底）

    只检查最后一段，避免历史状态头干扰。
    """
    if not transcript_content.strip():
        return ""

    # ── 格式 1：JSONL ─────────────────────────────────────────────────────────
    lines = transcript_content.strip().splitlines()
    jsonl_turns = []
    for line in lines:
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
            if isinstance(obj, dict):
                jsonl_turns.append(obj)
        except json.JSONDecodeError:
            break  # 不是 JSONL，停止尝试

    if jsonl_turns:
        # 找最后一条 role=assistant 的记录
        # 兼容两种格式：
        #   格式A（标准 Anthropic API）：{"role": "assistant", "content": [...]}
        #   格式B（Claude Code JSONL）：{"type": "assistant", "message": {"role": "assistant", "content": [...]}}
        for turn in reversed(jsonl_turns):
            # 格式B：Claude Code JSONL — 顶层 type="assistant"，content 嵌套在 message 内
            if turn.get("type") in ("assistant", "ai"):
                inner = turn.get("message", {})
                content = inner.get("content", "")
            else:
                # 格式A：顶层直接有 role
                role = turn.get("role", "")
                if role not in ("assistant", "ai"):
                    continue
                content = turn.get("content", "")

            if isinstance(content, list):
                # Anthropic API 格式：content 是 block 列表
                texts = [
                    block.get("text", "")
                    for block in content
                    if isinstance(block, dict) and block.get("type") == "text"
                ]
                result = "\n".join(texts)
                if result:
                    return result
            elif content:
                return str(content)
        return ""

    # ── 格式 2：纯文本角色标记 ────────────────────────────────────────────────
    # 支持：[ASSISTANT]、[Assistant]、Assistant:、<assistant>
    _ASSISTANT_SPLIT_RE = re.compile(
        r"\[ASSISTANT\]|\[Assistant\]|^Assistant:\s*|<assistant>",
        re.MULTILINE | re.IGNORECASE,
    )
    parts = _ASSISTANT_SPLIT_RE.split(transcript_content)
    if len(parts) > 1:
        last_part = parts[-1]
        # 截断到下一个角色标记之前
        _ANY_ROLE_RE = re.compile(
            r"\[HUMAN\]|\[Human\]|^Human:\s*|<human>|"
            r"\[USER\]|\[User\]|^User:\s*|<user>",
            re.MULTILINE | re.IGNORECASE,
        )
        cut = _ANY_ROLE_RE.search(last_part)
        if cut:
            last_part = last_part[: cut.start()]
        return last_part.strip()

    # ── 格式 3：回退 — 取末尾 2000 字符 ──────────────────────────────────────
    return transcript_content[-2000:]


def _detect_review_point_context(st: dict, response_text: str) -> Optional[str]:
    """Return review point id when state phase and response marker both match."""
    phase = str(st.get("phase") or "")
    if not phase:
        return None

    m = _REVIEW_POINT_MARKER_RE.search(response_text)
    if not m:
        return None

    point = m.group(1)
    if phase in _REVIEW_POINT_PHASES.get(point, set()):
        return point
    return None


def _has_any(text: str, patterns: list[str]) -> bool:
    return any(re.search(p, text, flags=re.IGNORECASE | re.MULTILINE) for p in patterns)


def _check_review_point_format(response_text: str, review_point: str) -> tuple[bool, str]:
    """Coarse structural checks for manual review point presentation."""
    checks: dict[str, list[tuple[str, list[str]]]] = {
        "1": [
            ("AC验收标准列表", [r"\bAC[-\s]?\d+\b", r"验收标准"]),
            ("核心接口一览", [r"核心接口", r"\binterface\b", r"\bAPI\b", r"接口"]),
            ("关键设计决策", [r"关键设计决策", r"设计决策", r"决策"]),
            ("已识别风险点", [r"风险点", r"风险"]),
            ("测试用例数量", [r"测试用例数量", r"\d+\s*个测试用例", r"测试用例\s*[:：]\s*\d+"]),
        ],
        "1.5": [
            ("核心业务理解", [r"核心业务理解", r"业务理解"]),
            ("接口/依赖", [r"接口", r"依赖", r"\bAPI\b"]),
            ("分层实现思路", [r"分层", r"实现思路"]),
            ("并发/事务策略", [r"并发", r"事务"]),
            ("异常处理思路", [r"异常处理", r"异常"]),
        ],
        "2": [
            ("逐文件核对标记", [r"逐文件", r"字典序", r"文件名"]),
            ("用户逐文件反馈符号", [r"[✅⚠️⏸️]"]),
            ("文件清单", [r"\.md\b", r"Task", r"文件"]),
        ],
        "2.5": [
            ("14条门禁通过状态", [r"14\s*条门禁", r"门禁.*通过"]),
            ("CodingModel 11维决策", [r"CodingModel", r"11\s*维"]),
            ("风险Task", [r"风险\s*Task", r"风险"]),
            ("关键类骨架", [r"关键类骨架", r"类骨架", r"已读源码"]),
        ],
        "4": [
            ("问题清单表", [r"问题清单", r"\bIssue\b", r"缺陷"]),
            ("AC覆盖对账表", [r"AC.*(对账|覆盖)", r"(对账|覆盖).*AC"]),
        ],
    }

    missing = [
        label
        for label, patterns in checks.get(review_point, [])
        if not _has_any(response_text, patterns)
    ]
    if missing:
        return False, "、".join(missing)

    if review_point == "2":
        marker_count = len(re.findall(r"[✅⚠️⏸️]", response_text))
        if marker_count < 1:
            return False, "用户逐文件反馈符号"

    return True, ""


def _is_automation_enabled(ade_sdd: Path) -> bool:
    """Read automation switch; default to manual mode when unavailable."""
    try:
        from lib import config as config_mod
        return config_mod.is_automation_enabled(ade_sdd)
    except Exception:
        return False


def _check_manual_review_point_format(ade_sdd: Path, response_text: str) -> str:
    """Return an inject message when a manual review point response is underspecified."""
    if _is_automation_enabled(ade_sdd):
        return ""

    try:
        from lib import work_item_context
        st = work_item_context.resolve_default_state(ade_sdd).data
    except Exception:
        return ""

    review_point = _detect_review_point_context(st, response_text)
    if not review_point:
        return ""

    ok, missing = _check_review_point_format(response_text, review_point)
    if ok:
        return ""

    return (
        f"[ae-sdd harness] Stop hook：检测到审核点{review_point}响应缺少必要呈现内容"
        f"（缺失：{missing}）。\n"
        "请在对话内直接呈现完整内容，不要仅给出文件路径或让用户自行打开文档。"
    )


def check_output(
    transcript_content: str,
    ade_sdd: Optional[Path] = None,
) -> tuple[bool, str]:
    """
    检查 AI 响应是否存在结构性错误（v2.0 精简版）。

    🆕 v3.6（决策 1B）：废弃自报标记检测（◆ STATE: / ◆ GATE: / ◆ LOADED:）。
    流程合规性检测已全部转移到 UserPromptSubmit hook（flow_monitor + prompt_inject）。
    本函数只负责：
      1. 空响应 / 结构性截断检测 → 重试（防 AI 输出残缺）
      2. PRD compact 失败检测（HS-8，保留）→ 重试
      3. 人工审核点对话呈现格式粗筛 → 重试
      4. 其余情况放行（allow stop）

    Args:
        transcript_content: last_assistant_message 文本；旧版回退时为 transcript_path 完整对话
        ade_sdd:            .ae-sdd/ 路径。None = 非 ae-sdd 项目，直接放行

    Returns:
        (should_stop, inject_message)
        should_stop=True  → 允许停止，inject_message 为空
        should_stop=False → 阻止停止，inject_message 注入给 AI
    """
    # 非 ae-sdd 项目，不检查
    if ade_sdd is None:
        return True, ""

    # 防无限循环：先读计数，达上限直接放行
    count = get_retry_count(ade_sdd)
    if count >= MAX_RETRY:
        return True, ""

    # 检查 1：空响应 / 结构性截断
    last_response = extract_last_assistant_text(transcript_content)
    if not last_response.strip():
        increment_retry(ade_sdd)
        return False, (
            "[ae-sdd harness] Stop hook：检测到空响应，可能 AI 输出被截断。\n"
            "请重新生成完整响应。"
        )

    # 检查 2：PRD compact 失败（HS-8，保留）
    compact_issue = _check_compact_failure(ade_sdd)
    if compact_issue:
        increment_retry(ade_sdd)
        return False, compact_issue

    # 检查 3：人工审核点对话呈现格式（自动化模式下跳过）
    review_format_issue = _check_manual_review_point_format(ade_sdd, last_response)
    if review_format_issue:
        increment_retry(ade_sdd)
        return False, review_format_issue

    # 其余情况放行（流程合规性由 UserPromptSubmit hook + flow_monitor 负责）
    return True, ""


# ─── 🆕 v3.5.4 HS-8：compact 失败检测（Stop hook 保留）────────────────────────
# 本函数保留：PRD compact 卡在 awaiting_compact 属于"session 结束前必须处理"的场景，
# 与流程合规性无关，由 Stop hook 兜底仍有价值。
def _check_compact_failure(ade_sdd: Optional[Path]) -> str:
    """HS-8：检测 PRD 级 compact 卡在 awaiting_compact 中途态。

    compact 成功的完整痕迹：prdStatus=compacted + summary.md 存在 + compactHistory 有记录。
    compact 失败/中途异常：prdStatus=awaiting_compact 但无 summary.md（卡在中间态）。

    检测到卡住 → 返回报警文本（阻止 session 结束，强制人工收尾，旧 state.json 保留）。
    无异常 → 返回空串。
    """
    if ade_sdd is None:
        return ""

    try:
        from lib import paths
        project_dir = paths.project_root(ade_sdd)
        auto_eng = project_dir / ".auto-engineering"
        if not auto_eng.is_dir():
            return ""

        stuck: list[str] = []
        for prd_dir in auto_eng.iterdir():
            if not prd_dir.is_dir():
                continue
            state_file = prd_dir / "state.json"
            if not state_file.is_file():
                continue
            try:
                import json
                ps = json.loads(state_file.read_text(encoding="utf-8"))
            except Exception:
                continue
            if ps.get("prdStatus") == "awaiting_compact":
                summary_file = prd_dir / "summary.md"
                if not summary_file.is_file():
                    stuck.append(prd_dir.name)

        if not stuck:
            return ""

        stuck_list = ", ".join(stuck)
        return (
            "[ae-sdd harness] HS-8 告警：检测到 PRD compact 卡在 awaiting_compact 中途态，"
            f"未生成 summary.md（受影响 PRD: {stuck_list}）。\n"
            "compact 可能失败，旧 PRD state.json 已保留（未覆盖）。\n"
            "请人工核查 compact 失败原因，修复后重跑：\n"
            f"  ae-sdd runtime compact --runtime <runtime> --prd <PRD-ID>\n"
            "确认 compact 成功（prdStatus=compacted + summary.md 生成）后再结束 session。\n"
            "（HS-8 物理拦截，🆕 v3.5.4）"
        )
    except Exception:
        return ""  # 检测异常不阻断（兜底放行）

