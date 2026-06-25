"""
prompt_inject.py — UserPromptSubmit hook v1.4

v1.4 修正（2026-06-22）：
  - 修复 completed phase 注入消息中的无效命令：
    'ae-sdd state write --phase （已结束）' → '（项目已完成，无需切换阶段）'

v1.3 变更（2026-06-22）：
  - 检测用户消息是否含快速通道标记，写入 .ae-sdd/.quick_channel 文件
    让 PreToolUse hook 能跨 hook 读到快速通道状态
  - 快速通道文件有效期：单次对话（下次 UserPromptSubmit 时清除非快速通道消息）

v1.2 修正（2026-06-22）：
  - stdin 读取 JSON，stdout 输出 JSON {"systemMessage": "..."}
  - 每次对话开始时重置 Stop hook 的重试计数
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# 快速通道标记文件名
_QUICK_CHANNEL_FILE = ".quick_channel"

QUICK_CHANNEL_MARKERS: tuple[str, ...] = (
    "ae-sdd-quick",
    "走快速通道",
    "quick channel",
    "quick mode",
)

# 🆕 v3.4.0 关卡1：/ae-sdd 触发词检测（建议书4）
AE_SDD_TRIGGER_MARKERS: tuple[str, ...] = (
    "/ae-sdd",
    "[$ae-sdd]",
    "启动自动化工程",
    "从 DR 开始",
    "端到端实现",
)


def _update_quick_channel(ade_sdd: Path, user_prompt: str) -> None:
    """
    根据用户消息更新快速通道状态文件。
    含快速通道标记 → 写入文件（PreToolUse hook 读取）
    不含标记 → 清除文件（快速通道仅单次对话有效）
    """
    qc_file = ade_sdd / _QUICK_CHANNEL_FILE
    if any(m in user_prompt for m in QUICK_CHANNEL_MARKERS):
        try:
            qc_file.write_text(user_prompt[:200], encoding="utf-8")
        except OSError:
            pass  # 写入失败（如目录只读）：降级，快速通道文件未创建
    else:
        try:
            qc_file.unlink(missing_ok=True)
        except OSError:
            pass


def inject(
    project_dir: Optional[Path] = None,
    user_prompt: str = "",
) -> dict:
    """
    生成注入到 AI context 的 JSON payload。

    Returns:
        dict → {"systemMessage": "..."} 或 {}
    """
    from lib import gates as gates_mod, paths, state as state_mod
    from lib.stop_check import reset_retry

    ade_sdd = paths.locate_project_ae_sdd(project_dir)
    if ade_sdd is None:
        return {}

    # 每次对话开始：重置 Stop hook 重试计数
    reset_retry(ade_sdd)

    # 更新快速通道状态（让 PreToolUse hook 能读到）
    _update_quick_channel(ade_sdd, user_prompt)

    # 读状态 + 配置（entry token 提醒需要 projectKey）
    st = state_mod.read_state(paths.state_path(ade_sdd))
    phase = st.get("phase", "initialized")
    current_story = st.get("currentStory") or "（未设定）"
    cfg = paths.read_config(ade_sdd)
    project_key = cfg.get("projectKey", "unknown")

    # 🆕 v3.4.0 关卡1：/ae-sdd 触发词检测 + entry token 提醒（建议书4）
    entry_reminder: list[str] = []
    if any(m in user_prompt for m in AE_SDD_TRIGGER_MARKERS):
        try:
            from lib import session as session_mod
            cur_story = st.get("currentStory", "") or ""
            if not session_mod.has_valid_entry_token(ade_sdd, cur_story):
                enter_cmd = (f"ae-sdd enter {project_key} --story {cur_story}"
                             if cur_story else f"ae-sdd enter {project_key}")
                entry_reminder.append(
                    f"⚠️ 检测到 ae-sdd 触发，必须先领取流程凭证（关卡1入口关卡）：\n"
                    f"   {enter_cmd}\n"
                    f"   未领凭证的流程产物落地/代码改动将被关卡2/3 物理拦截。\n"
                    f"   （建议书4 入口关卡方案）"
                )
        except Exception:
            pass  # session 模块异常不阻断注入

    # G-00 资产检查
    master = paths.locate_master_source()
    g00 = gates_mod.check_g00(master, ade_sdd, project_key)
    g00_status = "✅ CLEAR" if g00.pass_ else f"🔴 BLOCKED: {g00.message}"

    # 下一步建议
    suggestion = state_mod.next_step_suggestion(st)

    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    # completed phase 没有下一步命令，不显示 state write 指引
    next_phase = suggestion['next']
    if next_phase and next_phase not in ("（已结束）", ""):
        next_line = f"  next:     {suggestion['action']}  →  ae-sdd state write --phase {next_phase}"
    else:
        next_line = f"  next:     {suggestion['action']}  （项目已完成，无需切换阶段）"

    lines = [
        f"<!-- ae-sdd harness 自动注入 @ {now} -->",
        f"◆ HARNESS STATE",
        f"  project:  {project_key}",
        f"  phase:    {phase}",
        f"  story:    {current_story}",
        f"  G-00:     {g00_status}",
        next_line,
        f"  skill:    {suggestion['skill']}",
        f"<!-- /ae-sdd harness -->",
    ]

    if not g00.pass_:
        lines.insert(1,
            f"⛔ G-00 未通过，禁止继续任何业务操作。"
            f"修复: ae-sdd assets generate --project {project_key}"
        )

    # 🆕 v3.4.0 关卡1：entry token 提醒前置（最高优先级，在 G-00 之前）
    if entry_reminder:
        lines.insert(1, "\n".join(entry_reminder))

    return {"systemMessage": "\n".join(lines)}

