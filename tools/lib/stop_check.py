"""
stop_check.py — Stop hook v1.3

v1.3 修正（2026-06-22）：
  - 修复历史状态头误判：只检查 transcript 最后一段 AI 响应，不全文搜索
  - 支持多种 transcript 格式（JSONL / 纯文本带角色标记 / 回退全文）

v1.2 修正（2026-06-22）：
  - stdin 读取 JSON，从 transcript_path 文件读对话记录
  - 输出 JSON 格式，exit 始终 0
  - 防无限循环：用 .ae-sdd/.stop_retry_count 文件持久化计数

v1.4 修正（2026-06-27）：
  - CLI 入口优先使用 Claude Code Stop hook 的 last_assistant_message 字段
  - last_assistant_message 不存在时，保留 transcript_path 解析作为旧版回退
"""
from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Optional

# 状态头必须包含的标记
_STATE_HEADER_RE = re.compile(r"◆\s*STATE\s*:", re.MULTILINE)

# 防无限循环：最大重试次数
MAX_RETRY = 2

# 重试计数文件名（放在 .ae-sdd/ 下）
_RETRY_FILE_NAME = ".stop_retry_count"


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


def check_output(
    transcript_content: str,
    ade_sdd: Optional[Path] = None,
) -> tuple[bool, str]:
    """
    检查 transcript 最后一段 AI 响应是否包含状态头。

    Args:
        transcript_content: last_assistant_message 文本；旧版回退时为 transcript_path 读取的完整对话记录
        ade_sdd:            .ae-sdd/ 路径。None = 非 ae-sdd 项目，直接放行

    Returns:
        (should_stop, inject_message)
        should_stop=True  → 允许停止，inject_message 为空
        should_stop=False → 阻止停止，inject_message 注入给 AI
    """
    # 非 ae-sdd 项目，不检查
    if ade_sdd is None:
        return True, ""

    # 只检查最后一段 AI 响应
    last_response = extract_last_assistant_text(transcript_content)
    if _STATE_HEADER_RE.search(last_response):
        # 🆕 v3.4.0 F-1 修复：状态头存在时，交叉验证 ◆ GATE: 声明与实际文档一致
        gate_issue = _verify_gate_claims(last_response, ade_sdd)
        if gate_issue:
            # GATE 声明与实际不符 → 阻止停止（防 AI 谎报 GATE 状态，建议书3 F-1）
            count = get_retry_count(ade_sdd)
            if count >= MAX_RETRY:
                return True, ""
            increment_retry(ade_sdd)
            return False, gate_issue
        # 🆕 v3.5.4 HS-8：状态头通过后，检测 PRD compact 失败（卡在 awaiting_compact 无 summary.md）
        compact_issue = _check_compact_failure(ade_sdd)
        if compact_issue:
            count = get_retry_count(ade_sdd)
            if count >= MAX_RETRY:
                return True, ""  # 达重试上限，放行避免无限循环
            increment_retry(ade_sdd)
            return False, compact_issue
        return True, ""

    # 防无限循环
    count = get_retry_count(ade_sdd)
    if count >= MAX_RETRY:
        return True, ""
    increment_retry(ade_sdd)

    msg = (
        "[ae-sdd harness] 响应缺少状态头，请在本次响应末尾补充：\n"
        "◆ STATE:  <phase>/<story>\n"
        "◆ GATE:   ✅ CLEAR | 🔴 BLOCKED(<gate-id>)\n"
        "◆ LAST:   <刚完成的操作>\n"
        "◆ NEXT:   <下一个必须做的操作>\n"
    )
    return False, msg


# ─── 🆕 v3.4.0 F-1 修复：GATE 声明交叉验证（建议书3 §5 F-1）─────────────────
_GATE_LINE_RE = re.compile(r"◆\s*GATE\s*:([^\n]*)", re.MULTILINE)
_G08_CLEAR_RE = re.compile(r"G[-_]?08[^✅❌]*✅|✅[^✅❌]*G[-_]?08", re.IGNORECASE)
# 🆕 v3.5.10 Gap-012：原 _G08_CLEAR_RE 只覆盖 G-08 一个 gate，其余 25 个 gate 的 ✅ 声明
# AI 可任意谎报不被发现。扩展为通用模式，命中任一 G-XX ✅ 声明都跑对应真实 check。
# 支持格式：G-08 ✅ / G08 ✅ / G-RA-3 ✅ / G-CODEPLAN-SRC ✅ / ✅ G-08 等
_GATE_CLEAR_RE = re.compile(
    r"(G[-_](?:00|0[1-9]|1[0-4]|RA[-_]?\d|RA[-_]?FLOW[-_]?VIOLATION|"
    r"CODEPLAN[-_]?SRC|DOC[-_]?STORAGE|DOC[-_]?CONSISTENCY|PATH|CODE[-_]?1))"
    r"[^✅❌]*✅|✅[^✅❌]*"
    r"(G[-_](?:00|0[1-9]|1[0-4]|RA[-_]?[1-5]|RA[-_]?FLOW[-_]?VIOLATION|"
    r"CODEPLAN[-_]?SRC|DOC[-_]?STORAGE|DOC[-_]?CONSISTENCY|PATH|CODE[-_]?1))",
    re.IGNORECASE,
)
# 🆕 v3.5.10 Gap-012：可谎报校验的 gate 白名单（这些 gate 有"产物存在/Review 通过"语义，
# AI 容易谎报；G-00/G-PATH 等无产物校验语义的不在此列，避免无意义校验）
_VERIFIABLE_GATE_IDS = {
    "G-01", "G-02", "G-03", "G-04", "G-05", "G-06", "G-07", "G-08",
    "G-09", "G-10", "G-11", "G-12", "G-13", "G-14",
    "G-RA-1", "G-RA-2", "G-RA-3", "G-RA-4", "G-RA-5",
}
# gate id → 规范化（去分隔符）映射，用于从 CLEAR_RE 命中串里反查 gate id
def _normalize_gate_id(raw: str) -> str:
    """G-08 / G08 / g-08 → G-08；G-RA-3 / GRA3 → G-RA-3。"""
    s = re.sub(r"[\s_]", "", raw).upper()
    s = s.replace("G-", "G").replace("G", "G-", 1) if s.startswith("G") and not s.startswith("G-") else s
    # 重新规范化：G00→G-00, G08→G-08, GRA1→G-RA-1, GCODEPLANSCR→G-CODEPLAN-SRC
    s2 = re.sub(r"[\s_-]", "", raw).upper()
    mapping = {
        "G00": "G-00", "G01": "G-01", "G02": "G-02", "G03": "G-03", "G04": "G-04",
        "G05": "G-05", "G06": "G-06", "G07": "G-07", "G08": "G-08", "G09": "G-09",
        "G10": "G-10", "G11": "G-11", "G12": "G-12", "G13": "G-13", "G14": "G-14",
        "GRA1": "G-RA-1", "GRA2": "G-RA-2", "GRA3": "G-RA-3", "GRA4": "G-RA-4", "GRA5": "G-RA-5",
    }
    return mapping.get(s2, raw)


def _verify_gate_claims(last_response: str, ade_sdd: Optional[Path]) -> str:
    """交叉验证 AI 自报的 ◆ GATE: 声明与实际文档/状态一致（防谎报，建议书3 F-1）。

    覆盖范围（🆕 v3.5.10 Gap-012 扩展）：
    - 原 v3.4.0：仅 G-08 ✅ CLEAR 声明 → 校验 CodingPlan 14 关键词
    - v3.5.10：扩展到全部 _VERIFIABLE_GATE_IDS（19 个 gate），任一 ✅ 声明都跑真实 check
      AI 谎报任一 gate 通过但实际未过 → 阻断

    返回非空字符串 = 阻断原因；空字符串 = 通过。
    """
    if ade_sdd is None:
        return ""

    gate_line_match = _GATE_LINE_RE.search(last_response)
    if gate_line_match is None:
        return ""  # 无 GATE 声明，不校验（向后兼容）

    gate_line = gate_line_match.group(1)

    # 🆕 v3.5.10 Gap-012：通用 gate ✅ 声明校验（覆盖 19 个可谎报 gate）
    # 先用通用正则找出所有命中的 gate，逐个跑真实 check
    claimed_gates: list[str] = []
    for m in _GATE_CLEAR_RE.finditer(gate_line):
        # 取两个分组中非空的那个
        raw = m.group(1) or m.group(2) or ""
        gid = _normalize_gate_id(raw)
        if gid in _VERIFIABLE_GATE_IDS and gid not in claimed_gates:
            claimed_gates.append(gid)

    if claimed_gates:
        try:
            from lib import paths, state as state_mod, gates as gates_mod
            st = state_mod.read_state(paths.state_path(ade_sdd))
            current_story = st.get("currentStory", "") or ""
            project_dir = paths.project_root(ade_sdd)
            master = paths.locate_master_source()

            # gate id → check 函数名映射（与 GATE_REGISTRY 对齐）
            check_fn_map = {
                "G-01": "check_g01", "G-02": "check_g02", "G-03": "check_g03",
                "G-04": "check_g04", "G-05": "check_g05", "G-06": "check_g06",
                "G-07": "check_g07", "G-08": "check_g08", "G-09": "check_g09",
                "G-10": "check_g10", "G-11": "check_g11", "G-12": "check_g12",
                "G-13": "check_g13", "G-14": "check_g14",
                "G-RA-1": "check_ra_required", "G-RA-2": "check_ra_dimensions",
                "G-RA-3": "check_ra_derivatives",
                "G-RA-4": "check_ra_authenticity", "G-RA-5": "check_ra_depth",
            }
            for gid in claimed_gates:
                fn_name = check_fn_map.get(gid)
                if not fn_name:
                    continue
                fn = getattr(gates_mod, fn_name, None)
                if fn is None:
                    continue
                try:
                    # 不同 check 函数签名不同，统一尝试 4 种调用形式
                    try:
                        r = fn(project_dir, st, current_story)
                    except TypeError:
                        try:
                            r = fn(project_dir, st, current_story, master)
                        except TypeError:
                            r = fn(project_dir, st, current_story, master, None)
                except Exception:
                    continue  # 校验异常不阻断（兜底放行）
                if not r.pass_:
                    return (
                        f"[ae-sdd harness] GATE 声明与实际不符：AI 声称 {gid} ✅ CLEAR，"
                        f"但实际 {gid} 未通过（{r.message}）（F-1 假门禁修复 v3.5.10）。\n"
                        "请勿谎报 GATE 状态；修复对应门禁后再声明 ✅。"
                    )
        except Exception:
            return ""  # 校验异常不阻断（兜底放行）

    return ""


# ─── 🆕 v3.5.4 HS-8：compact 失败检测（Stop hook 阻断 + 报警）──────────────────
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
