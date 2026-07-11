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
from lib import memory_gate, paths, state as state_mod, work_item_context  # noqa: E402


# ─── Phase → 允许工具表 ─────────────────────────────────────────────────────
PHASE_PERMIT: dict[str, frozenset[str]] = {
    "initialized":     frozenset({"Write", "Edit", "MultiEdit"}),
    "ra-generated":    frozenset({"Write", "Edit", "MultiEdit"}),  # 🆕 v3.4.0 RA 阶段
    "dr-generated":    frozenset({"Write", "Edit", "MultiEdit"}),
    "story-generated": frozenset({"Write", "Edit", "MultiEdit"}),
    "story-reviewed":  frozenset({"Write", "Edit", "MultiEdit"}),
    "testcase-generated": frozenset({"Write", "Edit", "MultiEdit"}),  # 🆕 v3.7.0 TestCase 独立系列
    "testcase-reviewed":  frozenset({"Write", "Edit", "MultiEdit"}),  # 🆕 v3.7.0
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
    # echo 由 _is_readonly_bash 特判：仅无重定向的单条 echo 放行。
)

# ─── 路径感知 ─────────────────────────────────────────────────────────────────
# 源码写保护阶段：这些阶段的 Write/Edit/MultiEdit 禁止写入 src/ 源码目录
# - 设计阶段（initialized → task-generated）：只写设计文档
# - code-reviewed：只写 CR 报告，不应改源代码（改了就是绕过 CR）
_DESIGN_PHASES = frozenset({
    "initialized", "ra-generated",  # 🆕 v3.4.0 RA 阶段也禁写 src/
    "dr-generated", "story-generated",
    "story-reviewed",
    "testcase-generated", "testcase-reviewed",  # 🆕 v3.7.0 TestCase 独立系列，仍在设计阶段
    "task-generated",
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
    "TestCase": frozenset({"testcase-generated", "testcase-reviewed"}),  # 🆕 v3.7.0 独立系列
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


def _is_relative_to_path(path: Path, base: Path) -> bool:
    """Path.is_relative_to 的兼容封装，统一先 resolve(strict=False)。"""
    try:
        path.resolve(strict=False).relative_to(base.resolve(strict=False))
        return True
    except ValueError:
        return False


def _check_product_storage_path(
    file_path: str,
    product_type: str,
    ade_sdd: Optional[Path],
) -> tuple[bool, str]:
    """HS-10：流程产物必须落在 document-storage 推导的文档工作区内。

    这是关卡2的物理路径归属校验。它不尝试从文件名反推完整 intent 参数，
    但会强制产物位于 `{docWorkspace}/ae-sdd-doc/` 下，防止写到 `d:\\tmp\\`
    这类游离位置。具体文件名/版本号由 `ae-sdd doc save` / G-DOC-STORAGE 继续校验。
    """
    if ade_sdd is None:
        return True, ""

    cfg = paths.read_config(ade_sdd)
    project_key = cfg.get("projectKey")
    if not project_key:
        return False, (
            f"HS-10 阻断：无法读取 projectKey，不能验证 {product_type} 产物落地路径。\n"
            f"目标文件: {file_path}\n"
            f"请先修复 .ae-sdd/config.yaml，或用 `ae-sdd doc save` 通过 document_storage.resolve_path 落地。"
        )

    doc_workspace = paths.resolve_doc_workspace(ade_sdd, project_key)
    if doc_workspace is None:
        return False, (
            f"HS-10 阻断：无法从 assets 解析 docWorkspacePath/gitPath，不能验证 {product_type} 产物落地路径。\n"
            f"目标文件: {file_path}\n"
            f"请先补齐项目资产，或用 `ae-sdd doc save` 通过 document_storage.resolve_path 落地。"
        )

    target = Path(file_path)
    if not target.is_absolute():
        target = paths.project_root(ade_sdd) / target
    target = target.resolve(strict=False)
    expected_root = (doc_workspace / "ae-sdd-doc").resolve(strict=False)

    if _is_relative_to_path(target, expected_root):
        return True, ""

    return False, (
        f"HS-10 阻断：{product_type} 流程产物未落在 document-storage 推导的文档工作区内。\n"
        f"目标文件: {target}\n"
        f"允许根目录: {expected_root}\n"
        f"请改用 `ae-sdd doc save` 或 `ae-sdd doc resolve`，由 document_storage.resolve_path 推导路径后再落地。\n"
        f"（HS-10 物理拦截 + G-DOC-STORAGE 兜底）"
    )


def _check_product_landing(
    file_path: str, phase: str, ade_sdd: Optional[Path], state_data: Optional[dict] = None
) -> tuple[bool, str]:
    """关卡2：ae-sdd 流程产物落地校验（🆕 v3.4.0，建议书4）。

    命中产物模式时：
    1. 校验当前 session 有有效 entry token（关卡1领凭证）
    2. 校验当前 phase 允许写该类产物（产物-Phase 映射）
    3. 🆕 v3.7.2：被拦截时在修复提示里追加 `ae-sdd doc save` 命令建议
    不合规 → 物理拦截。
    返回 (allowed, deny_reason)。
    """
    product_type = _match_product_type(file_path)
    if product_type is None:
        return True, ""

    # HS-10：先校验路径归属，避免 d:\tmp\ 等游离路径被 entry-token/phase 检查遮住。
    allowed, reason = _check_product_storage_path(file_path, product_type, ade_sdd)
    if not allowed:
        return False, reason

    # 🆕 v3.7.2 统一修复建议：被拦截时引导用 ae-sdd doc save（避免手拼路径）
    doc_save_hint = (
        f"\n💡 建议：用 `ae-sdd doc save` 命令落地流程产物，代码自动处理路径/版本/ChangeLog/索引/"
        f"gitignore，无需手拼路径（document-storage-skill §4.0 CLI 入口）。"
    )

    # 从 state 读 currentStory（用于定位 session.json）
    # 🆕 v3.9.0 嵌套 state 兼容：用统一接口读 activeStory
    st = state_data if state_data is not None else (
        work_item_context.resolve_default_state(ade_sdd).data if ade_sdd else {}
    )
    current_story = state_mod.get_active_story(st) or "" if ade_sdd else ""

    # 🆕 v3.9.0 R5：产物 STORY-ID 归属校验——若产物含 STORY-ID，须在当前 state 的 storyStates 内
    if ade_sdd and product_type in ("Story", "Story Supplement", "TestCase", "Task", "CodingPlan",
                                     "Coding Report", "Test Report", "CR Report"):
        story_id_in_path = _extract_story_id_from_path(file_path)
        if story_id_in_path and state_mod.is_nested_state(st):
            story_states = set(state_mod.list_story_ids_in_state(st))
            if story_id_in_path not in story_states:
                return False, (
                    f"产物 {product_type} 的 STORY-ID={story_id_in_path} 未登记到当前 state 的 storyStates。\n"
                    f"当前 state 仅含: {sorted(story_states)}\n"
                    f"目标文件: {file_path}\n"
                    f"请先跑 /ae-sdd 路由，或 `ae-sdd state relocate --story {story_id_in_path}` 重定位，"
                    f"或 `ae-sdd state write --add-story {story_id_in_path}` 归入当前 state。\n"
                    f"（v3.9.0 R5 产物-STORY 归属门禁）"
                    f"{doc_save_hint}"
                )

    # 关卡1：entry token 校验
    try:
        from lib import session as session_mod
        if ade_sdd is not None and not session_mod.has_valid_entry_token(ade_sdd, current_story):
            return False, (
                f"ae-sdd 流程产物（{product_type}）落地前须先领取 entry token。\n"
                f"目标文件: {file_path}\n"
                f"请先运行: ae-sdd enter <projectKey> --story {current_story or '<STORY-ID>'}\n"
                f"（关卡1 入口凭证缺失，建议书4）"
                f"{doc_save_hint}"
            )
    except Exception as e:
        return False, (
            f"ae-sdd 流程产物（{product_type}）落地门禁自检异常，禁止放行。\n"
            f"目标文件: {file_path}\n"
            f"异常: {type(e).__name__}: {e}\n"
            f"请先修复 session/entry token 检查，再重试。"
            f"{doc_save_hint}"
        )

    # 关卡2：产物-Phase 映射校验
    allowed_phases = _PRODUCT_PHASE_MAP.get(product_type)
    if allowed_phases and phase not in allowed_phases:
        return False, (
            f"当前 phase={phase} 不允许写入 {product_type} 类产物（允许 phase: {sorted(allowed_phases)}）。\n"
            f"目标文件: {file_path}\n"
            f"请先切换到允许的 phase：ae-sdd state write --phase {sorted(allowed_phases)[0]}\n"
            f"（关卡2 产物-Phase 映射，建议书4）"
            f"{doc_save_hint}"
        )

    # 🆕 v3.8.1 S-3：文件意图锁冲突检测（多 sub-agent 并发写防护）
    # PreToolUse hook 无法可靠识别"当前是哪个 agent 在写"，故本层只做冲突告警不硬阻断。
    # 锁的获取/释放由 agent 显式调 `ae-sdd state lock/unlock` CLI（与 register_agent 同模式）。
    # 仅当存在 ≥2 个活跃 agent 且目标文件已被锁时告警（单 agent 场景不触发，防误伤）。
    if ade_sdd and st:
        try:
            active_agents = st.get("activeAgents") or []
            if len(active_agents) >= 2:
                rel_path = _relative_product_path(file_path, ade_sdd)
                if rel_path:
                    lock_info = state_mod.check_file_lock(st, rel_path)
                    if lock_info:
                        holder = lock_info.get("agentId", "unknown")
                        # warn 不阻断（首版策略，与 B-3 remediation 同思路）
                        import sys
                        sys.stderr.write(
                            f"[ae-sdd harness] ⚠️ S-3 文件锁告警：{product_type} 产物 {rel_path} "
                            f"已被 agent {holder} 持锁，当前有 {len(active_agents)} 个活跃 agent 并发。"
                            f"并发写同一文件会丢更新，建议协调或用 `ae-sdd state lock --path {rel_path}` 重新登记。\n"
                        )
        except Exception:
            pass  # 锁检查异常不阻断（兜底放行）

    return True, ""


def _relative_product_path(file_path: str, ade_sdd: Optional[Path]) -> str:
    """把绝对/相对文件路径归一为 state.fileLocks 的 key（相对 project_dir，正斜杠）。

    project_dir 推导自 ade_sdd（.ae-sdd/ 的父目录）。无法推导时返回空串。
    """
    if not ade_sdd:
        return ""
    try:
        project_dir = ade_sdd.parent
        p = Path(file_path)
        try:
            rel = p.resolve().relative_to(project_dir.resolve())
        except ValueError:
            # file_path 可能已是相对路径
            rel = Path(file_path)
        return str(rel).replace("\\", "/")
    except Exception:
        return ""


def _extract_story_id_from_path(file_path: str) -> Optional[str]:
    """🆕 v3.9.0 R5：从产物文件路径提取 STORY-ID。

    扫描路径各段 + 文件名，匹配 STORY-\\d+ 模式。
    如 "ae-sdd-doc/Story/STORY-003-BE/STORY-003-BE-CodingPlan.md" → "STORY-003-BE"

    Returns:
        STORY-ID 字符串（大写）或 None
    """
    import re
    if not file_path:
        return None
    # 匹配完整 STORY-ID（含后缀如 -BE）
    m = re.search(r"STORY[-_]?\d+(?:[-_]?[A-Za-z]+)?", file_path, re.IGNORECASE)
    return m.group(0).upper() if m else None


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
    state_data: Optional[dict] = None,
) -> tuple[bool, str]:
    if tool_name not in WRITE_TOOLS:
        return True, ""
    if not file_path:
        return True, ""
    if _is_always_allowed_path(file_path):
        # 🆕 v3.4.0 关卡2：即使 always-allow，若是 ae-sdd 流程产物仍校验 entry token + 产物-Phase
        allowed, reason = _check_product_landing(file_path, phase, ade_sdd, state_data=state_data)
        if not allowed:
            return False, reason
        return True, ""
    # 🆕 v3.4.0 关卡2：非 always-allow 路径若命中产物模式也校验（如 d:\tmp\*-CodingPlan.md）
    allowed, reason = _check_product_landing(file_path, phase, ade_sdd, state_data=state_data)
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
                st = state_data if state_data is not None else work_item_context.resolve_default_state(ade_sdd).data
                # 🆕 v3.9.1 嵌套 state 兼容：用统一接口读 activeStory（nested → activeStory）
                current_story = state_mod.get_active_story(st) or ""
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
        except Exception as e:
            return False, (
                f"代码改动准入门禁自检异常，禁止放行。\n"
                f"目标文件: {file_path}\n"
                f"异常: {type(e).__name__}: {e}\n"
                f"请先修复 session 审核点检查，再重试。"
            )
    return True, ""


# ─── 🆕 v3.8.2 存端兜底：关联 phase 写操作前检查 memory enter ──────────────────
def _check_memory_entered(
    phase: str,
    ade_sdd: Optional[Path],
    state_data: Optional[dict],
) -> tuple[bool, str]:
    """关联 phase 写操作前检查 memory 是否已 enter。

    堵住 Agent 不调 state write 直接写源码绕过 memory gate 的漏洞：
    PHASE_PERMIT 放行 Write/Edit 前，若当前 phase 属于 5 个关联节点
    (ra/design/coding-plan/coding/review)，强制要求该 scope 已 memory enter。

    🆕 v3.9.7 fix-life-deadlock：函数入口惰性 mkdir memory_root，避免
    全新 .ae-sdd 项目的 design/coding-plan 等关联阶段在第一次写操作前
    因 .ae-sdd/memory/ 不存在导致后续 locate_scope/is_scope_active 链路
    隐式失败。原行为：缺目录时 _read_json 静默返回 {}，is_scope_active
    永远返回 False，门槛看似"未 enter"——但实际是"目录都没建"。
    本修改 best-effort 静默 mkdir，与 ae-sdd init 的"项目首次创建即
    创建 memory 子目录"对齐；不改 token 判定语义（仍以 is_scope_active
    为准），仅消除"目录缺失 = 永假"的隐性死路。
    项目侧仍需 `ae-sdd memory enter` 才能拿到 stage 活跃态。

    Returns:
        (allowed, deny_reason)
        allowed=True  → 已 enter 或非关联 phase，放行
        allowed=False → 未 enter，返回 deny reason 含修复命令
    """
    if ade_sdd is None:
        return True, ""  # 非 ae-sdd 项目，不拦截
    try:
        # 🆕 v3.9.7 fix-life-deadlock：惰性创建 memory 根目录（best-effort）
        try:
            memory_root = ade_sdd / "memory"
            if not memory_root.exists():
                memory_root.mkdir(parents=True, exist_ok=True)
        except Exception:
            # 任何 OSError（如 EROFS、PermissionError）都不应阻断门禁逻辑
            # ——让下游 locate_scope/is_scope_active 自己处理，保留原失败模式
            pass

        from lib import memory_gate, memory_store
        memory_phase = memory_gate.memory_phase_for_state_phase(phase)
        if not memory_phase:
            return True, ""  # 非关联 phase（如 initialized/completed），跳过
        # 🆕 v3.9.1 嵌套 state 兼容：用统一接口读 activeStory（nested → activeStory）
        story = state_mod.get_active_story(state_data or {}) or None
        scope = memory_store.locate_scope(
            project=str(ade_sdd.parent), phase=memory_phase, story=story,
        )
        if memory_store.is_scope_active(scope):
            return True, ""  # 已 enter 未 exit，放行
        story_arg = f" --story {story}" if story else ""
        return False, (
            f"存端记忆门禁：当前 phase={phase}（memory phase={memory_phase}）"
            f"尚未执行 memory enter，禁止写操作。\n"
            f"memory 是关联节点的强制工具集，不写记忆不得推进工作。\n"
            f"请先执行:\n"
            f"  ae-sdd memory enter --phase {memory_phase}{story_arg}\n"
            f"  ae-sdd memory write --phase {memory_phase}{story_arg}"
            f" --summary \"...\" --evidence <file:line>\n"
            f"  ae-sdd memory exit --phase {memory_phase}{story_arg}\n"
            f"（🆕 v3.8.2 存端兜底，堵 PHASE_PERMIT 放行绕过 memory gate 漏洞）"
        )
    except Exception as e:
        return False, (
            f"存端记忆门禁自检异常，禁止放行。\n"
            f"当前 phase={phase}\n"
            f"异常: {type(e).__name__}: {e}\n"
            f"请先修复 memory gate 检查，再重试。"
        )


# ─── ae-sdd state write 保护 ─────────────────────────────────────────────────
# 只匹配以 ae-sdd 或 python .../ae-sdd 开头的命令，
# 排除 echo/注释等非执行形式（防止 '# ae-sdd state write' 等误触发 gate check）
_STATE_WRITE_CMD_RE = re.compile(r"^python\s+\S*ae-sdd\b", re.IGNORECASE)


def _is_ae_sdd_cmd(bash_command: str) -> bool:
    """命令是否以 ae-sdd 或 python .../ae-sdd 开头（真实执行形式，排除注释/echo）。"""
    stripped = (bash_command or "").strip()
    return stripped.startswith("ae-sdd") or bool(_STATE_WRITE_CMD_RE.match(stripped))


# ─── 🆕 v3.9.2 修复：ae-sdd memory 命令放行（设计阶段死锁 bug）──────────────────
# v3.8.2 引入 memory gate 后，设计阶段 6 个 phase（ra/design/coding-plan/review 域）
# 推进前必须完成 ae-sdd memory enter/write/exit，但这三个是 Bash 命令，
# 而这些 phase 的 PHASE_PERMIT 不含 Bash → AI 跑 memory 被自己设的门禁拦死。
# 与 step 3 给 ae-sdd state write 开特殊通道同理，memory 命令只动
# .ae-sdd/memory/ 目录，属流程自管理命令族，独立于 PHASE_PERMIT 放行。
# 第二个 token 必须是 memory，覆盖全部 8 个子命令
# （enter/write/exit/read/search/promote/summarize）。
_MEMORY_CMD_RE = re.compile(r"^(?:ae-sdd|python\s+\S*ae-sdd)\s+memory\b", re.IGNORECASE)
_ASSETS_GENERATE_CMD_RE = re.compile(
    r"^(?:ae-sdd|python\S*\s+\S*ae-sdd)\s+assets\s+generate\b",
    re.IGNORECASE,
)


def _is_ae_sdd_memory_cmd(bash_command: str) -> bool:
    """命令是否为 `ae-sdd memory <subcmd>` 真实执行形式（排除注释/echo）。

    Returns:
        True 当且仅当命令头匹配 ae-sdd memory / python .../ae-sdd memory。
        不检查子命令是否合法——CLI 自身会校验。
    """
    stripped = (bash_command or "").strip()
    return bool(_MEMORY_CMD_RE.match(stripped))


def _is_ae_sdd_assets_generate_cmd(bash_command: str) -> bool:
    """命令是否为单条 `ae-sdd assets generate` 维护命令。"""
    stripped = (bash_command or "").strip()
    return bool(_ASSETS_GENERATE_CMD_RE.match(stripped))


# ─── 🆕 v3.9.17 修复：ae-sdd state new / enter 逃生放行 ────────────────────────
# resolve_default_state() 会把 completed 状态排除出隐式候选池（避免已完成任务
# 假性占位歧义/被误当默认态），但这意味着「唯一候选已 completed」或「全部候选
# 已 completed」时会退化成 NoWorkItemStateError——而该异常建议的自救命令正是
# `ae-sdd state new` / `ae-sdd enter`。若这两个命令本身还要走 resolve_default_state
# 才能放行，就会先于自己创建的新状态被同一个异常拦死，构成死锁。
# state new 只在 .auto-engineering/ 下新建一个工作项目录，enter 只写入其
# session.json 凭证，二者都不读写任何既有 work-item 的 state.json，因此可以
# 独立于 work-item 解析结果放行（同 memory / assets generate 命令族）。
_STATE_NEW_OR_ENTER_CMD_RE = re.compile(
    r"^(?:ae-sdd|python\s+\S*ae-sdd)\s+(?:state\s+new|enter)\b",
    re.IGNORECASE,
)


def _is_ae_sdd_state_new_or_enter_cmd(bash_command: str) -> bool:
    """命令是否为单条 `ae-sdd state new` / `ae-sdd enter` 自救命令。"""
    stripped = (bash_command or "").strip()
    return bool(_STATE_NEW_OR_ENTER_CMD_RE.match(stripped))


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
    # 🆕 v3.9.1 嵌套 state 兼容：判空用统一接口（nested 读 activeStory，flat 读 currentStory）
    if not state_mod.get_active_story(effective_state):
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
    # 🆕 v3.10.0：砍 Task phase + Route 下移重分级。
    # 🆕 v3.10.1：子系列合并--story-generated/testcase-generated 各自 = generate+review loop 完成。
    #   大=DR 入口（原 PRD 层弃用，RA 为前置不在链内）；中=Story 入口；
    #   小=CodingPlan 入口（需 Story+TestCase 存在）；微=无文档直出 CodingPlan。
    #   所有链的 coding 入口统一 G-00+G-07+G-08（Plan-first 不豁免）。
    #   旧 state 无 scale -> _resolve_scale 已在上方跨步判定时回写，此处读同一 scale。
    scale_for_gates = scale_state.get("scale") or _resolve_scale(scale_state)
    PHASE_ENTRY_GATES: dict[str, dict[str, list[str]]] = {
        "大": {  # RA 入口：4 loop（RA-DR-Story-TestCase）+ Coding/Testing
            "ra-generated":    ["G-00", "G-RA-1", "G-RA-2", "G-RA-3", "G-RA-4", "G-RA-5", "G-RA-6", "G-RA-FLOW-VIOLATION"],
            "dr-generated":    ["G-00", "G-01", "G-DR-CTX"],
            "story-generated": ["G-00", "G-02", "G-03", "G-STORY-CTX", "G-REVIEW-DEPTH"],
            "testcase-generated": ["G-00", "G-02", "G-03", "G-04", "G-TESTCASE-CTX"],
            "coding-process":  ["G-00", "G-02", "G-03", "G-04", "G-STORY-CTX", "G-TESTCASE-CTX"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11", "G-REVIEW-DEPTH"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
        "中": {  # DR 入口：3 loop（DR-Story-TestCase）+ Coding/Testing，跳 RA
            "dr-generated":    ["G-00", "G-01", "G-RA-1", "G-RA-2", "G-RA-3", "G-RA-4", "G-RA-5", "G-RA-6", "G-RA-FLOW-VIOLATION", "G-DR-CTX"],
            "story-generated": ["G-00", "G-02", "G-03", "G-STORY-CTX", "G-REVIEW-DEPTH"],
            "testcase-generated": ["G-00", "G-02", "G-03", "G-04", "G-TESTCASE-CTX"],
            "coding-process":  ["G-00", "G-02", "G-03", "G-04", "G-STORY-CTX", "G-TESTCASE-CTX"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11", "G-REVIEW-DEPTH"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
        "小": {  # CodingPlan 入口：已有 Story+TestCase，直出 coding-process
            "coding-process":  ["G-00", "G-02", "G-03", "G-04", "G-STORY-CTX", "G-TESTCASE-CTX"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11", "G-REVIEW-DEPTH"],
            "completed":       ["G-00", "G-12", "G-13"],
        },
        "微": {  # 无文档：initialized->coding-process，coding 入口 G-00+G-07+G-08（Plan-first 不豁免）
            "coding-process":  ["G-00"],
            "coding":          ["G-00", "G-07", "G-08"],
            "test-running":    ["G-00"],
            "code-reviewed":   ["G-00", "G-09", "G-CODE-1", "G-10", "G-11", "G-REVIEW-DEPTH"],
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

# 链式命令分隔符正则（&&、||、;、|、换行、单个 &、$() 命令替换、反引号）
# 单个 & 需与 && 区分：用前后不为 & 的环视避免和 && 分支重复匹配同一位置。
_CHAIN_RE = re.compile(r"&&|\|\||\s*;\s*|\s*\|\s*|\n|(?<!&)&(?!&)|\$\(|`")
_REDIRECT_RE = re.compile(r">|<")


def _is_readonly_bash(command: Optional[str]) -> bool:
    """
    判断 Bash 命令是否为只读操作。

    安全策略：
    - 只有命令是「单条」且前缀命中白名单时才认为只读
    - 含 &&、||、;、| 的链式命令一律不认为只读
      （防止 'git status && rm -rf design/' 绕过白名单）
    - 含 > / < 重定向一律不认为只读
    - 例外：ae-sdd gates check --json 等 ae-sdd 内置命令允许带 --json 参数
    """
    if not command:
        return False
    cmd = command.strip()
    # 链式命令：含分隔符 → 拒绝快速放行，交由 phase permit 决定
    if _CHAIN_RE.search(cmd):
        return False
    if _REDIRECT_RE.search(cmd):
        return False
    if re.match(r"^(echo|printf)\b", cmd):
        return True
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


# ─── 🆕 v3.9.3：待初始化项目拦截 ────────────────────────────────────────────────

def _check_pending_init_intercept(
    tool_name: str,
    bash_command: Optional[str],
    file_path: Optional[str],
    allow_readonly: bool,
) -> tuple[bool, str]:
    """ae-sdd 待初始化项目的拦截逻辑。

    只放行：只读工具、只读 Bash、ae-sdd init 命令。
    拦截：Write/Edit/MultiEdit 和任意非只读 Bash。
    """
    # 只读工具放行
    if allow_readonly and tool_name in READONLY_TOOLS:
        return True, ""

    # 只读 Bash 放行
    if tool_name == "Bash" and _is_readonly_bash(bash_command):
        return True, ""

    # ae-sdd init 命令放行（含 python -m ae-sdd / 绝对路径等变体）
    if tool_name == "Bash" and bash_command:
        stripped = bash_command.strip()
        if re.match(r'^(python\S*\s+)?\S*ae-sdd\s+init\b', stripped) and not _CHAIN_RE.search(stripped):
            return True, ""

    # 其余全部拦截
    return False, (
        f"ae-sdd 项目尚未初始化（未找到 .ae-sdd/config.yaml）。\n"
        f"当前 {tool_name} 操作已被拦截。\n\n"
        f"请先运行 ae-sdd init 初始化项目：\n"
        f"  ae-sdd init . <projectKey>\n"
        f"  # init 会自动生成 baseline 项目资产\n"
        f"然后重新触发 /ae-sdd。\n\n"
        f"如需紧急绕过，请说：/ae-sdd-quick 或 '走快速通道'"
    )


# ─── 主拦截逻辑 ───────────────────────────────────────────────────────────────
def check_intercept(
    tool_name: str,
    *,
    bash_command: Optional[str] = None,
    file_path: Optional[str] = None,
    allow_readonly: bool = True,
    project_dir: Optional[Path] = None,
    forced_phase: Optional[str] = None,
    session_key: str = "",
    forced_engaged: Optional[bool] = None,
) -> tuple[bool, str]:
    """
    核心拦截判断。

    Args:
        forced_engaged: 测试用。None=按真实 engage 标记判断（生产默认）；
            True/False=强制跳过 engage 检查（仅测试应传，生产代码勿用）。

    Returns:
        (allowed, deny_reason)
        allowed=True  → 允许，reason 为空
        allowed=False → 拒绝，reason 为纯文本（_deny_response 会包装成 JSON）
    """
    # 1. 只读工具永远放行
    if allow_readonly and tool_name in READONLY_TOOLS:
        return True, ""

    # 1b. 只读 Bash 不应被 work-item 歧义锁死。
    if allow_readonly and tool_name == "Bash" and _is_readonly_bash(bash_command):
        return True, ""

    # 1c. 🆕 v3.9.17 ae-sdd state new / enter 逃生放行，先于 work-item 解析。
    # 必须在 resolve_default_state() 之前放行，否则「唯一/全部候选已 completed」
    # 退化出的 NoWorkItemStateError 会先拦住这两个本该用来脱困的自救命令。
    # 该放行独立于 phase 之外（不经过 PHASE_PERMIT），一旦被链式/替换/重定向
    # 语法夹带绕过就会越过所有后续检查，因此除 _CHAIN_RE 外再叠加 _REDIRECT_RE。
    if (
        tool_name == "Bash"
        and bash_command
        and _is_ae_sdd_state_new_or_enter_cmd(bash_command)
        and not _CHAIN_RE.search(bash_command.strip())
        and not _REDIRECT_RE.search(bash_command.strip())
    ):
        return True, ""

    # 2. 读取状态
    phase = forced_phase
    ade_sdd: Optional[Path] = None
    project_key = "unknown"

    if phase is None:
        ade_sdd = paths.locate_project_ae_sdd(project_dir)
        if ade_sdd is None:
            # 🆕 v3.9.3：用户触发 /ae-sdd 但项目未 init → 检查待初始化标记
            pending = paths.pending_init_marker(project_dir)
            if pending.exists():
                return _check_pending_init_intercept(tool_name, bash_command, file_path, allow_readonly)
            return True, ""  # 非 ae-sdd 项目，不拦截
        # 🆕 engage 判定：未 engage 的会话不做门禁校验。
        # 用户调 /ae-sdd 后 prompt_inject 会写本会话的 engage 标记；
        # 没调过的会话/子 Agent 直接放行，实现"按需启用 hook"语义。
        # forced_engaged 非 None 时（测试用）跳过标记检查。
        if forced_engaged is None and not work_item_context.is_session_engaged(ade_sdd, session_key):
            return True, ""
        try:
            resolved_state = work_item_context.resolve_default_state(
                ade_sdd,
                session_key=session_key,
            )
        except work_item_context.AmbiguousWorkItemError as e:
            return False, str(e)
        except work_item_context.NoWorkItemStateError as e:
            return False, str(e)
        st = resolved_state.data
        # 🆕 v3.9.1 嵌套 state 兼容：用统一接口读 active phase（nested → storyStates[activeStory].phase）
        phase = state_mod.get_active_phase(st)
        cfg = paths.read_config(ade_sdd)
        project_key = cfg.get("projectKey", "unknown")
    else:
        st = {"phase": phase}

    # 3a. ae-sdd assets generate 维护命令放行。
    # G-00 失败时给出的修复动作必须可执行；仅允许单条命令，链式 Bash 继续拦截。
    if tool_name == "Bash" and bash_command:
        stripped = bash_command.strip()
        if _is_ae_sdd_assets_generate_cmd(stripped) and not _CHAIN_RE.search(stripped):
            return True, ""

    # 3. ae-sdd state write 保护
    if tool_name == "Bash" and bash_command:
        if "ae-sdd" in bash_command and "state" in bash_command and "write" in bash_command:
            allowed, reason = _check_state_write(bash_command, phase, ade_sdd, project_key, state_data=st)
            if not allowed:
                return False, reason
            return True, ""

    # 3c. 🆕 v3.9.2 ae-sdd memory 命令放行（修复设计阶段死锁）
    # memory enter/write/exit 只动 .ae-sdd/memory/，属流程自管理命令族，
    # 独立于 PHASE_PERMIT 放行（同 step 3 对 state write 的处理）。
    # 链式命令（&&/||/;/|/换行）不快速放行，交回 step 4-6 正常检查，
    # 防止 'ae-sdd memory enter && rm -rf .ae-sdd/' 被误放。
    if tool_name == "Bash" and bash_command:
        stripped = bash_command.strip()
        if _is_ae_sdd_memory_cmd(stripped) and not _CHAIN_RE.search(stripped):
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
        allowed, reason = _check_path_permission(tool_name, file_path, phase, ade_sdd, state_data=st)
        if not allowed:
            return False, reason

    # 5.5 🆕 v3.8.2 存端兜底：关联 phase 写操作前检查 memory enter。
    # 堵住 Agent 不调 state write 直接写源码绕过 memory gate 的漏洞。
    # 仅对写工具（Write/Edit/MultiEdit）触发，只读工具与 Bash 已在第1/4步放行。
    if tool_name in WRITE_TOOLS and ade_sdd is not None:
        allowed, reason = _check_memory_entered(phase, ade_sdd, st)
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
