"""
gate_intercept.py — PreToolUse hook v1.4

v1.4 修正（2026-06-22）：
  - 修复 _ALWAYS_ALLOW_PATTERNS 中 .json/.yaml/.yml 过宽放行问题：
    src/main/resources/*.yaml 等资源文件在设计阶段应被拦截，
    移除 .json/.yaml/.yml 后缀的通用放行规则
  - 修复链式 Bash 命令绕过只读白名单：
    'git status && rm -rf design/' 前缀命中白名单但后半段有危险操作
    _is_readonly_bash() 增加链式分隔符检测（&&、||、;、|、换行），有分隔符一律不认为只读
  - 修复 code-reviewed 阶段缺少源码写保护：
    code-reviewed 加入 _DESIGN_PHASES，CR 阶段只能写报告文档，不能改源码
  - 修复 _extract_target_phase 过宽：注释/echo 命令中的 ae-sdd state write
    不再触发 gate check，命令头必须以 ae-sdd 或 python .../ae-sdd 开头
  - 扩充 BASH_READONLY_PREFIXES：新增 node/python3/git --version 等常用版本查询命令，
    以及 which 查询命令，避免在设计阶段误拦截只读环境查询

v1.3 变更（2026-06-22）：
  - MultiEdit 被加入 WRITE_TOOLS 和 PHASE_PERMIT
  - 快速通道从 .ae-sdd/.quick_channel 文件读取（跨 hook 共享状态）

v1.2 修正（2026-06-22）：
  - stdin 读取 JSON（{"hook_event_name":"PreToolUse","tool_name":"...","tool_input":{...}}）
  - file_path 从 tool_input.file_path 读取，不从环境变量
  - bash command 从 tool_input.command 读取，不从环境变量
  - 输出 JSON 拒绝格式：
      {"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"},
       "systemMessage":"..."}
  - exit 始终 0（Claude Code 通过 JSON permissionDecision 判断，不通过 exit code）

Claude Code PreToolUse hook 数据格式（来源：hookify 官方实现）：
  stdin:  {
            "hook_event_name": "PreToolUse",
            "tool_name": "Write",          # 或 "Edit" / "Bash" / ...
            "tool_input": {
              "file_path": "src/...",       # Write/Edit
              "content": "...",             # Write
              "new_string": "...",          # Edit
              "command": "mvn compile"      # Bash
            }
          }
  stdout: {} 允许
          {
            "hookSpecificOutput": {
              "hookEventName": "PreToolUse",
              "permissionDecision": "deny"
            },
            "systemMessage": "拒绝原因（显示给 AI）"
          } 拒绝
  exit:   始终 0
"""
from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Optional

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import paths, state as state_mod  # noqa: E402


# ─── Phase → 允许工具表 ─────────────────────────────────────────────────────
PHASE_PERMIT: dict[str, frozenset[str]] = {
    "initialized":     frozenset({"Write", "Edit", "MultiEdit"}),
    "dr-generated":    frozenset({"Write", "Edit", "MultiEdit"}),
    "story-generated": frozenset({"Write", "Edit", "MultiEdit"}),
    "story-reviewed":  frozenset({"Write", "Edit", "MultiEdit"}),
    "task-generated":  frozenset({"Write", "Edit", "MultiEdit"}),
    "task-reviewed":   frozenset({"Write", "Edit", "MultiEdit", "Bash"}),
    "coding":          frozenset({"Write", "Edit", "MultiEdit", "Bash"}),
    "test-running":    frozenset({"Write", "Edit", "MultiEdit", "Bash"}),
    "code-reviewed":   frozenset({"Write", "Edit", "MultiEdit"}),
    "completed":       frozenset(),
}

# 只读工具（任何 phase 都允许）
READONLY_TOOLS: frozenset[str] = frozenset({
    "Read", "Glob", "Grep", "TodoWrite", "TodoRead",
    "LS", "Cat", "AskUserQuestion",
})

# 写文件类工具（Write / Edit / MultiEdit 统一处理）
WRITE_TOOLS: frozenset[str] = frozenset({"Write", "Edit", "MultiEdit"})

BASH_READONLY_PREFIXES: tuple[str, ...] = (
    # 文件查看
    "cat ", "ls ", "find ", "grep ",
    "pwd",
    # Git 只读
    "git status", "git log", "git diff", "git --version",
    # 版本查询（任何 phase 允许）
    "mvn --version", "mvn -v",
    "java --version", "java -version",
    "python --version", "python3 --version",
    "node --version", "node -v",
    "npm --version", "npm -v",
    "gradle --version",
    "pip --version", "pip3 --version",
    # ae-sdd 只读子命令
    "ae-sdd version", "ae-sdd health",
    "ae-sdd state read", "ae-sdd state next-step",
    "ae-sdd gates check", "ae-sdd classify",
    "ae-sdd assets check",
    # 环境查询
    "which ",
    # 注意：echo 不加入白名单（echo 'hello' > file.txt 可写文件）
)

# ─── 路径感知 ─────────────────────────────────────────────────────────────────
# 源码写保护阶段：这些阶段的 Write/Edit/MultiEdit 禁止写入 src/ 源码目录
# - 设计阶段（initialized → task-generated）：只写设计文档
# - code-reviewed：只写 CR 报告，不应改源代码（改了就是绕过 CR）
_DESIGN_PHASES = frozenset({
    "initialized", "dr-generated", "story-generated",
    "story-reviewed", "task-generated",
    "code-reviewed",  # CR 阶段：只写报告，不改源码
})

_SOURCE_CODE_PATTERNS: tuple[re.Pattern, ...] = tuple(re.compile(p) for p in [
    r"src/main/java/",
    r"src/main/kotlin/",
    r"src/test/java/",
    r"src/test/kotlin/",
    r"src/main/resources/.*\.(xml|yaml|yml|properties)$",
])

_ALWAYS_ALLOW_PATTERNS: tuple[re.Pattern, ...] = tuple(re.compile(p) for p in [
    r"design/",
    r"ae-sdd-doc/",
    r"\.ae-sdd/",
    r"\.auto-engineering/",
    r"\.claude/",
    r"pom\.xml$",
    r"README",
    r"\.md$",
    # 注意：.json/.yaml/.yml 不在此列——
    # src/main/resources/*.yaml 等资源文件在设计阶段应被拦截
    # 改用 _check_path_permission 逻辑：先查 source code patterns，再查 always_allow
])


def _is_source_code_path(file_path: str) -> bool:
    normalized = file_path.replace("\\", "/")
    return any(p.search(normalized) for p in _SOURCE_CODE_PATTERNS)


def _is_always_allowed_path(file_path: str) -> bool:
    normalized = file_path.replace("\\", "/")
    return any(p.search(normalized) for p in _ALWAYS_ALLOW_PATTERNS)


def _check_path_permission(
    tool_name: str,
    file_path: Optional[str],
    phase: str,
) -> tuple[bool, str]:
    if tool_name not in WRITE_TOOLS:
        return True, ""
    if not file_path:
        return True, ""
    if _is_always_allowed_path(file_path):
        return True, ""
    if phase in _DESIGN_PHASES and _is_source_code_path(file_path):
        return False, (
            f"设计阶段（phase={phase}）禁止写入源码目录。\n"
            f"目标文件: {file_path}\n"
            f"请先完成设计文档，通过 Task Review 后切换到 task-reviewed phase。\n"
            f"切换命令: ae-sdd state write --phase task-reviewed\n"
        )
    return True, ""


# ─── ae-sdd state write 保护 ─────────────────────────────────────────────────
# 只匹配以 ae-sdd 或 python .../ae-sdd 开头的命令，
# 排除 echo/注释等非执行形式（防止 '# ae-sdd state write' 等误触发 gate check）
_STATE_WRITE_CMD_RE = re.compile(r"^python\s+\S*ae-sdd\b", re.IGNORECASE)


def _extract_target_phase(bash_command: str) -> Optional[str]:
    """
    从 Bash 命令中提取目标 phase（仅针对真实执行的 ae-sdd state write 命令）。

    安全策略：命令必须以 ae-sdd 或 python .../ae-sdd 开头，
    排除注释（# ae-sdd ...）和 echo（echo ae-sdd ...）等非执行形式。
    """
    stripped = bash_command.strip()
    # 命令头检查：以 ae-sdd 开头，或以 python .../ae-sdd 开头
    if not (stripped.startswith("ae-sdd") or _STATE_WRITE_CMD_RE.match(stripped)):
        return None
    m = re.search(r"--phase\s+(\S+)", stripped, re.IGNORECASE)
    return m.group(1) if m else None


def _check_state_write(
    bash_command: str,
    current_phase: str,
    ade_sdd: Optional[Path],
    project_key: str,
) -> tuple[bool, str]:
    target_phase = _extract_target_phase(bash_command)
    if not target_phase:
        return True, ""

    from lib.state import PHASE_FLOW
    try:
        current_idx = PHASE_FLOW.index(current_phase)
        target_idx = PHASE_FLOW.index(target_phase)
    except ValueError:
        return True, ""

    if target_idx <= current_idx:
        return True, ""

    if target_idx > current_idx + 1:
        steps = target_idx - current_idx
        return False, (
            f"禁止跨步跳跃（当前 {current_phase} → 目标 {target_phase}，跳了 {steps} 步）。\n"
            f"必须按顺序切换: {' → '.join(PHASE_FLOW[current_idx:target_idx + 1])}\n"
        )

    # 向前跳 1 步：验证进入条件
    PHASE_ENTRY_GATES: dict[str, list[str]] = {
        "dr-generated":    ["G-00", "G-01"],
        "story-generated": ["G-00", "G-01", "G-02"],
        "story-reviewed":  ["G-00", "G-02", "G-03"],
        "task-generated":  ["G-00", "G-03", "G-04"],
        "task-reviewed":   ["G-00", "G-05", "G-06", "G-07", "G-08"],
        "coding":          ["G-00", "G-07", "G-08"],
        "test-running":    ["G-00"],
        "code-reviewed":   ["G-00", "G-09", "G-10", "G-11"],
        "completed":       ["G-00", "G-12", "G-13"],
    }
    required_gates = PHASE_ENTRY_GATES.get(target_phase, [])
    if not required_gates:
        return True, ""

    from lib import gates as gates_mod
    master = paths.locate_master_source()
    failed = []
    for gate_id in required_gates:
        results = gates_mod.check_all(master, ade_sdd, project_key, only=gate_id)
        if results and not results[0].pass_:
            failed.append((gate_id, results[0].message))

    if not failed:
        return True, ""

    gate_lines = "\n".join(f"  ❌ {gid}: {msg}" for gid, msg in failed)
    return False, (
        f"切换到 {target_phase} 的进入条件未通过：\n"
        f"{gate_lines}\n"
        f"修复后重试: ae-sdd state write --phase {target_phase}\n"
    )


# ─── 只读 Bash 检测 ───────────────────────────────────────────────────────────

# 链式命令分隔符正则（&&、||、;、|、换行）
_CHAIN_RE = re.compile(r"&&|\|\||\s*;\s*|\s*\|\s*|\n")


def _is_readonly_bash(command: Optional[str]) -> bool:
    """
    判断 Bash 命令是否为只读操作。

    安全策略：
    - 只有命令是「单条」且前缀命中白名单时才认为只读
    - 含 &&、||、;、| 的链式命令一律不认为只读
      （防止 'git status && rm -rf design/' 绕过白名单）
    - 例外：ae-sdd gates check --json 等 ae-sdd 内置命令允许带 --json 参数
    """
    if not command:
        return False
    cmd = command.strip()
    # 链式命令：含分隔符 → 拒绝快速放行，交由 phase permit 决定
    if _CHAIN_RE.search(cmd):
        return False
    return any(cmd.startswith(p) for p in BASH_READONLY_PREFIXES)


# ─── 拒绝响应构造 ─────────────────────────────────────────────────────────────
def _deny_response(tool_name: str, reason: str) -> dict:
    """构造 Claude Code PreToolUse 拒绝响应（JSON 格式）"""
    full_reason = (
        f"[ae-sdd gate-intercept] {tool_name} 被拒绝\n\n"
        f"{reason}\n"
        f"如需紧急绕过，请说：/ae-sdd-quick 或 '走快速通道'（仍需落档）"
    )
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
        },
        "systemMessage": full_reason,
    }


# ─── 主拦截逻辑 ───────────────────────────────────────────────────────────────
def check_intercept(
    tool_name: str,
    *,
    bash_command: Optional[str] = None,
    file_path: Optional[str] = None,
    allow_readonly: bool = True,
    project_dir: Optional[Path] = None,
    forced_phase: Optional[str] = None,
) -> tuple[bool, str]:
    """
    核心拦截判断。

    Returns:
        (allowed, deny_reason)
        allowed=True  → 允许，reason 为空
        allowed=False → 拒绝，reason 为纯文本（_deny_response 会包装成 JSON）
    """
    # 1. 只读工具永远放行
    if allow_readonly and tool_name in READONLY_TOOLS:
        return True, ""

    # 2. 读取状态
    phase = forced_phase
    ade_sdd: Optional[Path] = None
    project_key = "unknown"

    if phase is None:
        ade_sdd = paths.locate_project_ae_sdd(project_dir)
        if ade_sdd is None:
            return True, ""  # 非 ae-sdd 项目，不拦截
        st = state_mod.read_state(paths.state_path(ade_sdd))
        phase = st.get("phase", "initialized")
        cfg = paths.read_config(ade_sdd)
        project_key = cfg.get("projectKey", "unknown")

    # 3. ae-sdd state write 保护
    if tool_name == "Bash" and bash_command:
        if "ae-sdd" in bash_command and "state" in bash_command and "write" in bash_command:
            allowed, reason = _check_state_write(bash_command, phase, ade_sdd, project_key)
            if not allowed:
                return False, reason
            return True, ""

    # 4. 只读 Bash 放行
    if tool_name == "Bash" and _is_readonly_bash(bash_command):
        return True, ""

    # 5. 路径感知
    if file_path:
        allowed, reason = _check_path_permission(tool_name, file_path, phase)
        if not allowed:
            return False, reason

    # 6. Phase 工具权限
    permitted = PHASE_PERMIT.get(phase, frozenset())
    if tool_name in permitted:
        return True, ""

    # 7. 拒绝
    from lib.state import next_step_suggestion
    suggestion = next_step_suggestion({"phase": phase})
    reason = (
        f"当前 phase={phase} 不允许 {tool_name} 操作。\n"
        f"下一步: {suggestion['action']}\n"
        f"SKILL:  {suggestion['skill']}\n"
        f"切换:   ae-sdd state write --phase {suggestion['next']}\n"
    )
    return False, reason


# ─── 快速通道检测 ────────────────────────────────────────────────────────────
QUICK_CHANNEL_MARKERS: tuple[str, ...] = (
    "ae-sdd-quick",
    "走快速通道",
    "quick channel",
    "quick mode",
)


def is_quick_channel_active(context_text: Optional[str]) -> bool:
    import os
    if os.environ.get("AE_SDD_QUICK", "").lower() in ("1", "true", "yes"):
        return True
    if not context_text:
        return False
    return any(m in context_text for m in QUICK_CHANNEL_MARKERS)
