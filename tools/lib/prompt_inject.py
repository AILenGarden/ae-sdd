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

    # 读状态
    st = state_mod.read_state(paths.state_path(ade_sdd))
    phase = st.get("phase", "initialized")
    current_story = st.get("currentStory") or "（未设定）"
    cfg = paths.read_config(ade_sdd)
    project_key = cfg.get("projectKey", "unknown")

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

    return {"systemMessage": "\n".join(lines)}

