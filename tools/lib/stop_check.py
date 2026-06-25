"""
stop_check.py — Stop hook v1.3

v1.3 修正（2026-06-22）：
  - 修复历史状态头误判：只检查 transcript 最后一段 AI 响应，不全文搜索
  - 支持多种 transcript 格式（JSONL / 纯文本带角色标记 / 回退全文）

v1.2 修正（2026-06-22）：
  - stdin 读取 JSON，从 transcript_path 文件读对话记录
  - 输出 JSON 格式，exit 始终 0
  - 防无限循环：用 .ae-sdd/.stop_retry_count 文件持久化计数
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
        for turn in reversed(jsonl_turns):
            role = turn.get("role", "")
            if role in ("assistant", "ai"):
                content = turn.get("content", "")
                if isinstance(content, list):
                    # Anthropic API 格式：content 是 block 列表
                    texts = [
                        block.get("text", "")
                        for block in content
                        if isinstance(block, dict) and block.get("type") == "text"
                    ]
                    return "\n".join(texts)
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
        transcript_content: 完整对话记录文本（从 transcript_path 读取）
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


def _verify_gate_claims(last_response: str, ade_sdd: Optional[Path]) -> str:
    """交叉验证 AI 自报的 ◆ GATE: 声明与实际文档/状态一致（防谎报，建议书3 F-1）。

    当前覆盖：
    - G-08 ✅ CLEAR 声明 → 校验实际 {STORY}-CodingPlan.md 含 14 关键词 + 无 ❌ 标记
      （AI 谎报 G-08 通过但 CodingPlan 缺关键词/有 ❌ → 阻断）

    返回非空字符串 = 阻断原因；空字符串 = 通过。
    """
    if ade_sdd is None:
        return ""

    gate_line_match = _GATE_LINE_RE.search(last_response)
    if gate_line_match is None:
        return ""  # 无 GATE 声明，不校验（向后兼容）

    gate_line = gate_line_match.group(1)

    # G-08 ✅ CLEAR 声明校验
    if _G08_CLEAR_RE.search(gate_line):
        try:
            from lib import paths, state as state_mod, gates as gates_mod
            st = state_mod.read_state(paths.state_path(ade_sdd))
            current_story = st.get("currentStory", "") or ""
            if not current_story:
                return ""  # 无 Story，跳过
            project_dir = paths.project_root(ade_sdd)
            cp = paths.find_doc(project_dir, current_story, "-CodingPlan.md")
            if cp is None:
                return (
                    "[ae-sdd harness] GATE 声明与实际不符：AI 声称 G-08 ✅ CLEAR，"
                    f"但 {current_story}-CodingPlan.md 不存在（F-1 假门禁修复）。\n"
                    "请勿谎报 GATE 状态；先生成 CodingPlan 或修正 GATE 声明。"
                )
            # 跑真实 G-08 检查
            r = gates_mod.check_g08(project_dir, st, current_story)
            if not r.pass_:
                return (
                    f"[ae-sdd harness] GATE 声明与实际不符：AI 声称 G-08 ✅ CLEAR，"
                    f"但实际 G-08 未通过（{r.message}）（F-1 假门禁修复）。\n"
                    "请勿谎报 GATE 状态；修复 CodingPlan 14 门禁后再声明 ✅。"
                )
        except Exception:
            return ""  # 校验异常不阻断（兜底放行）

    return ""
