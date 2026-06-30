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
from lib import memory_gate, paths, state as state_mod  # noqa: E402


# ─── Phase → 允许工具表 ─────────────────────────────────────────────────────
PHASE_PERMIT: dict[str, frozenset[str]] = {
    "initialized":     frozenset({"Write", "Edit", "MultiEdit"}),
    "ra-generated":    frozenset({"Write", "Edit", "MultiEdit"}),  # 🆕 v3.4.0 RA 阶段
    "dr-generated":    frozenset({"Write", "Edit", "MultiEdit"}),
    "story-generated": frozenset({"Write", "Edit", "MultiEdit"}),
    "story-reviewed":  frozenset({"Write", "Edit", "MultiEdit"}),
    "task-generated":  frozenset({"Write", "Edit", "MultiEdit"}),
    "task-reviewed":   frozenset({"Write", "Edit", "MultiEdit", "Bash"}),
    "coding-process":  frozenset({"Write", "Edit", "MultiEdit", "Bash"}),  # 🆕 v3.5.16 CodingProcess：写 CodePlan + 跑门禁 CLI
    "coding":          frozenset({"Write", "Edit", "MultiEdit", "Bash"}),
    "test-running":    frozenset({"Write", "Edit", "MultiEdit", "Bash"}),
    "code-reviewed":   frozenset({"Write", "Edit", "MultiEdit"}),
    "completed":       frozenset(),
    "paused":          frozenset(),  # 🆕 v3.6 paused 阶段禁止所有写操作，仅允许只读工具
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
    "initialized", "ra-generated",  # 🆕 v3.4.0 RA 阶段也禁写 src/
    "dr-generated", "story-generated",
    "story-reviewed", "task-generated",
    "coding-process",  # 🆕 v3.5.16 CodingProcess 只产 CodePlan，禁写生产代码（task-reviewed 仍允许写 src/，保持原行为）
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

# ─── 🆕 v3.4.0 关卡2：ae-sdd 流程产物路径模式 + 产物-Phase 映射（建议书4）─────────
# 命中这些模式的流程产物，落地前校验 entry token + 当前 phase 允许写该类产物。
_PRODUCT_PATTERNS: tuple[tuple[re.Pattern, str], ...] = tuple(
    (re.compile(p), label) for p, label in [
        (r".*-CodingPlan\.md$", "CodingPlan"),
        (r".*-CodingReport-.*\.md$", "CodingReport"),
        (r".*-CodeReview.*\.md$", "CodeReview"),
        (r".*-Story\.md$", "Story"),
        (r".*-TestCase\.md$", "TestCase"),
        (r".*-task-.*\.md$", "Task"),
        (r".*-DR\.md$", "DR"),
        (r".*-RA\.md$", "RA"),
        (r".*-业务逻辑汇总\.md$", "业务逻辑汇总"),
    ]
)

# 产物类型 → 允许写入的 phase（关卡2校验依据）
_PRODUCT_PHASE_MAP: dict[str, frozenset[str]] = {
    "DR": frozenset({"dr-generated"}),
    "RA": frozenset({"ra-generated", "dr-generated"}),
    "Story": frozenset({"story-generated", "story-reviewed"}),
    "TestCase": frozenset({"story-reviewed", "task-generated"}),
    "Task": frozenset({"task-generated", "task-reviewed"}),
    "CodingPlan": frozenset({"task-reviewed", "coding-process", "coding"}),  # 🆕 v3.5.16 coding-process 产出 CodePlan
    "CodingReport": frozenset({"coding", "test-running", "code-reviewed"}),
    "CodeReview": frozenset({"code-reviewed"}),
    "业务逻辑汇总": frozenset({"story-reviewed", "task-generated"}),
}


def _match_product_type(file_path: str) -> Optional[str]:
    """若路径匹配某 ae-sdd 流程产物模式，返回产物类型；否则 None。"""
    normalized = file_path.replace("\\", "/")
    for pat, label in _PRODUCT_PATTERNS:
        if pat.search(normalized):
            return label
    return None


def _check_product_landing(
    file_path: str, phase: str, ade_sdd: Optional[Path]
) -> tuple[bool, str]:
    """关卡2：ae-sdd 流程产物落地校验（🆕 v3.4.0，建议书4）。

    命中产物模式时：
    1. 校验当前 session 有有效 entry token（关卡1领凭证）
    2. 校验当前 phase 允许写该类产物（产物-Phase 映射）
    不合规 → 物理拦截。
    返回 (allowed, deny_reason)。
    """
    product_type = _match_product_type(file_path)
    if product_type is None:
        return True, ""

    # 从 state 读 currentStory（用于定位 session.json）
    st = state_mod.read_state(paths.state_path(ade_sdd)) if ade_sdd else {}
    current_story = st.get("currentStory", "") if ade_sdd else ""

    # 关卡1：entry token 校验
    try:
        from lib import session as session_mod
        if ade_sdd is not None and not session_mod.has_valid_entry_token(ade_sdd, current_story):
            return False, (
                f"ae-sdd 流程产物（{product_type}）落地前须先领取 entry token。\n"
                f"目标文件: {file_path}\n"
                f"请先运行: ae-sdd enter <projectKey> --story {current_story or '<STORY-ID>'}\n"
                f"（关卡1 入口凭证缺失，建议书4）"
            )
    except Exception:
        # session 模块异常不阻断（兜底放行，避免误伤）
        pass

    # 关卡2：产物-Phase 映射校验
    allowed_phases = _PRODUCT_PHASE_MAP.get(product_type)
    if allowed_phases and phase not in allowed_phases:
        return False, (
            f"当前 phase={phase} 不允许写入 {product_type} 类产物（允许 phase: {sorted(allowed_phases)}）。\n"
            f"目标文件: {file_path}\n"
            f"请先切换到允许的 phase：ae-sdd state write --phase {sorted(allowed_phases)[0]}\n"
            f"（关卡2 产物-Phase 映射，建议书4）"
        )

    return True, ""


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
    ade_sdd: Optional[Path] = None,
) -> tuple[bool, str]:
    if tool_name not in WRITE_TOOLS:
        return True, ""
    if not file_path:
        return True, ""
    if _is_always_allowed_path(file_path):
        # 🆕 v3.4.0 关卡2：即使 always-allow，若是 ae-sdd 流程产物仍校验 entry token + 产物-Phase
        allowed, reason = _check_product_landing(file_path, phase, ade_sdd)
        if not allowed:
            return False, reason
        return True, ""
    # 🆕 v3.4.0 关卡2：非 always-allow 路径若命中产物模式也校验（如 d:\tmp\*-CodingPlan.md）
    allowed, reason = _check_product_landing(file_path, phase, ade_sdd)
    if not allowed:
        return False, reason
    if phase in _DESIGN_PHASES and _is_source_code_path(file_path):
        return False, (
            f"设计阶段（phase={phase}）禁止写入源码目录。\n"
            f"目标文件: {file_path}\n"
            f"请先完成设计文档，通过 Task Review 后切换到 task-reviewed phase。\n"
            f"切换命令: ae-sdd state write --phase task-reviewed\n"
        )
    # 🆕 v3.4.0 关卡3：代码改动准入（建议书4）— coding/test-running phase 写 src/ 须有审核点确认 token
    # 🆕 v3.5.15：按 scale 路由——微链（微任务/BUG/配置类）免 task-reviewed 确认（微任务无 Task/无审核点 2.5），
    #   改为要求 coding phase 自身确认（审核点 2.5 微任务版 = coding 确认）。
    if phase in ("coding", "test-running") and _is_source_code_path(file_path):
        try:
            from lib import session as session_mod
            if ade_sdd is not None:
                st = state_mod.read_state(paths.state_path(ade_sdd))
                current_story = st.get("currentStory", "")
                scale = st.get("scale") or state_mod._resolve_scale(st)
                # 微链：免 task-reviewed，改要求 coding phase 确认
                if scale == "微":
                    required_confirm = "coding"
                    gate_label = "微任务 coding 准入"
                else:
                    required_confirm = "task-reviewed"
                    gate_label = "审核点 2.5（CodingPlan 评审）"
                if not session_mod.is_phase_confirmed(ade_sdd, required_confirm, current_story):
                    return False, (
                        f"代码改动准入未通过：{scale}任务 coding phase 写源码须先完成 {gate_label}。\n"
                        f"目标文件: {file_path}\n"
                        f"用户确认后运行: ae-sdd state confirm --phase {required_confirm} --story {current_story or '<STORY-ID>'}\n"
                        f"（关卡3 代码改动准入，建议书4 / 🆕 v3.5.15 scale 路由）"
                    )
                # 🆕 v3.5.16 硬层产物校验：coding phase 写 src/ 须先走过 CodingProcess
                # （证明 CodePlan 经 CodingProcess 加载5上下文+调CodingSkill产出，而非凭记忆写代码）
                if not session_mod.is_phase_confirmed(ade_sdd, "coding-process", current_story):
                    return False, (
                        f"代码改动准入未通过：coding phase 写源码须先完成 CodingProcess（加载5上下文+CodeAnalysis+产CodePlan）。\n"
                        f"目标文件: {file_path}\n"
                        f"先执行 CodingProcess 产出 CodePlan，用户确认后运行: "
                        f"ae-sdd state confirm --phase coding-process --story {current_story or '<STORY-ID>'}\n"
                        f"（🆕 v3.5.16 Task→Coding 解耦，硬层产物校验，防 AI 凭记忆绕过 CodingProcess）"
                    )
        except Exception:
            pass  # session 模块异常不阻断（兜底放行）
    return True, ""


# ─── ae-sdd state write 保护 ─────────────────────────────────────────────────
# 只匹配以 ae-sdd 或 python .../ae-sdd 开头的命令，
# 排除 echo/注释等非执行形式（防止 '# ae-sdd state write' 等误触发 gate check）
_STATE_WRITE_CMD_RE = re.compile(r"^python\s+\S*ae-sdd\b", re.IGNORECASE)


def _is_ae_sdd_cmd(bash_command: str) -> bool:
    """命令是否以 ae-sdd 或 python .../ae-sdd 开头（真实执行形式，排除注释/echo）。"""
    stripped = (bash_command or "").strip()
    return stripped.startswith("ae-sdd") or bool(_STATE_WRITE_CMD_RE.match(stripped))


def _extract_option_value(bash_command: str, option: str) -> Optional[str]:
    stripped = bash_command.strip()
    if not (stripped.startswith("ae-sdd") or _STATE_WRITE_CMD_RE.match(stripped)):
        return None
    m = re.search(rf"{re.escape(option)}\s+(\S+)", stripped, re.IGNORECASE)
    return m.group(1) if m else None


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
    return _extract_option_value(bash_command, "--phase")


# ─── 🆕 v3.5.4 HS-7 物理拦截：ae-sdd state prd-complete 前置校验 4 层 AND ──────
def _check_prd_complete_gate(
    bash_command: str, project_dir: Optional[Path]
) -> tuple[bool, str]:
    """HS-7：拦截 ae-sdd state prd-complete，实时校验 4 层 AND。

    堵住 cmd_state_prd_complete（ae-sdd:268-271）跳过校验直接写 awaiting_compact 的漏洞。
    拦截时实时跑 state.check_prd_4_layers（不依赖"上次证据"，防过时）。

    Returns:
        (allowed, deny_reason)
    """
    if not _is_ae_sdd_cmd(bash_command):
        return True, ""
    # 仅匹配 state prd-complete（非 prd-check-complete / prd-archive）
    if not re.search(r"\bstate\s+prd-complete\b", bash_command):
        return True, ""

    prd_id = _extract_option_value(bash_command, "--prd")
    if not prd_id:
        return True, ""  # 缺 --prd 参数，交由 CLI 自身报错

    if project_dir is None:
        return True, ""  # 无法定位项目根，兜底放行

    prd_state_path = project_dir / ".auto-engineering" / prd_id / "state.json"
    if not prd_state_path.exists():
        return False, (
            f"HS-7 阻断：PRD 级 state.json 不存在（{prd_state_path}）。\n"
            f"无法校验 4 层 AND，禁止执行 prd-complete。\n"
            f"请先跑: ae-sdd state prd-check-complete --prd {prd_id}\n"
            f"（HS-7 物理拦截，🆕 v3.5.4）"
        )

    try:
        import json
        ps = json.loads(prd_state_path.read_text(encoding="utf-8"))
    except Exception as e:
        return False, (
            f"HS-7 阻断：PRD state.json 解析失败（{e}）。\n"
            f"禁止执行 prd-complete。\n"
            f"（HS-7 物理拦截，🆕 v3.5.4）"
        )

    result = state_mod.check_prd_4_layers(ps)
    if result["all_pass"]:
        return True, ""  # 4 层全过，放行

    # 未全过：列 missing 项阻断
    missing_lines = []
    for k in ("G-PRD-1", "G-PRD-2", "G-PRD-3", "G-PRD-4"):
        r = result[k]
        if not r["pass"]:
            missing_lines.append(f"  ❌ {k} {r['label']}: {', '.join(r['missing'])}")
    missing_text = "\n".join(missing_lines) if missing_lines else "  (无明细)"
    return False, (
        f"HS-7 阻断：PRD {prd_id} 4 层 AND 未全过，禁止执行 prd-complete。\n"
        f"{missing_text}\n"
        f"请先跑: ae-sdd state prd-check-complete --prd {prd_id}\n"
        f"修复未达成项后重试。\n"
        f"（HS-7 物理拦截，🆕 v3.5.4 — 堵 cmd_state_prd_complete 跳校验漏洞）"
    )


def _check_state_write(
    bash_command: str,
    current_phase: str,
    ade_sdd: Optional[Path],
    project_key: str,
    state_data: Optional[dict] = None,
) -> tuple[bool, str]:
    target_phase = _extract_target_phase(bash_command)
    if not target_phase:
        return True, ""

    from lib.state import PHASE_FLOWS, _resolve_scale
    # 🆕 v3.5.15：按 state.scale 选子链判定跨步跳跃。
    #   微链 initialized→coding 是合法单步（idx 0→1），不再被「跨步跳跃」误拦。
    #   旧 state 无 scale → _resolve_scale 反推（默认大链，最保守）。
    scale_state = dict(state_data or {"phase": current_phase})
    scale = _resolve_scale(scale_state)
    chain = PHASE_FLOWS[scale]
    try:
        current_idx = chain.index(current_phase)
        target_idx = chain.index(target_phase)
    except ValueError:
        return True, ""

    if target_idx <= current_idx:
        return True, ""

    if target_idx > current_idx + 1:
        steps = target_idx - current_idx
        return False, (
            f"禁止跨步跳跃（scale={scale}，当前 {current_phase} → 目标 {target_phase}，跳了 {steps} 步）。\n"
            f"必须按顺序切换: {' → '.join(chain[current_idx:target_idx + 1])}\n"
        )

    # 向前跳 1 步：验证进入条件
    effective_state = dict(state_data or {"phase": current_phase})
    if not effective_state.get("currentStory"):
        effective_state["currentStory"] = _extract_option_value(bash_command, "--story")
    if not effective_state.get("currentTask"):
        effective_state["currentTask"] = _extract_option_value(bash_command, "--task")

    memory_result = memory_gate.check_state_transition(
        ade_sdd=ade_sdd,
        state_data=effective_state,
        target_phase=target_phase,
    )
    if memory_result.get("blocked"):
        return False, memory_gate.format_transition_block(memory_result)

    # 🆕 v3.5.15：PHASE_ENTRY_GATES 按 scale 路由。
    #   微链 coding 入口免 G-07/08（微任务无 Task，无 task-reviewed 前置）；
    #   小链 task-generated 入口含 G-03/G-04（小任务有 Task）；
    #   中链 story-generated 入口含 G-02（中任务有 Story）。
    #   旧 state 无 scale → _resolve_scale 已在上方跨步判定时回写，此处读同一 scale。
    scale_for_gates = scale_state.get("scale") or _resolve_scale(scale_state)
    PHASE_ENTRY_GATES: dict[str, dict[str, list[str]]] = {
        "大": {
            "ra-generated":    ["G-00"],
            "dr-generated":    ["G-00", "G-01"],
            "story-generated": ["G-00", "G-01", "G-02"],
            "story-reviewed":  ["G-00", "G-02", "G-03"],
            "task-generated":  ["G-00", "G-03", "G-04"],
            "task-reviewed":   ["G-00", "G-05", "G-06", "G-07", "G-08"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
        "中": {  # 跳过 DR：ra→story，无 dr-generated
            "ra-generated":    ["G-00"],
            "story-generated": ["G-00", "G-01", "G-02"],
            "story-reviewed":  ["G-00", "G-02", "G-03"],
            "task-generated":  ["G-00", "G-03", "G-04"],
            "task-reviewed":   ["G-00", "G-05", "G-06", "G-07", "G-08"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
        "小": {  # 跳过 DR/Story：ra→task，无 dr/story
            "ra-generated":    ["G-00"],
            "task-generated":  ["G-00", "G-03", "G-04"],
            "task-reviewed":   ["G-00", "G-05", "G-06", "G-07", "G-08"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
        "微": {  # 跳过 RA/DR/Story/Task：initialized→coding，coding 入口仅 G-00
            "coding":          ["G-00"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
    }
    scale_gates = PHASE_ENTRY_GATES.get(scale_for_gates, {})
    required_gates = scale_gates.get(target_phase, [])
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
    else:
        st = {"phase": phase}

    # 3. ae-sdd state write 保护
    if tool_name == "Bash" and bash_command:
        if "ae-sdd" in bash_command and "state" in bash_command and "write" in bash_command:
            allowed, reason = _check_state_write(bash_command, phase, ade_sdd, project_key, state_data=st)
            if not allowed:
                return False, reason
            return True, ""

    # 3b. 🆕 v3.5.4 HS-7：ae-sdd state prd-complete 前置校验 4 层 AND
    if tool_name == "Bash" and bash_command and "prd-complete" in bash_command:
        # project_dir 优先用入参，否则从 ade_sdd（.ae-sdd/）反推项目根
        hs7_project_dir = project_dir if project_dir is not None else (
            ade_sdd.parent if ade_sdd is not None else None
        )
        allowed, reason = _check_prd_complete_gate(bash_command, hs7_project_dir)
        if not allowed:
            return False, reason
        # prd-complete 通过 4 层 AND 后，仍走后续路径/phase 校验，不提前 return

    # 4. 只读 Bash 放行
    if tool_name == "Bash" and _is_readonly_bash(bash_command):
        return True, ""

    # 5. 路径感知（🆕 v3.4.0 关卡2 产物落地校验内嵌）
    if file_path:
        allowed, reason = _check_path_permission(tool_name, file_path, phase, ade_sdd)
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
