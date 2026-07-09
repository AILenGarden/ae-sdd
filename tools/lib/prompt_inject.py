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
  - stdin 读取 JSON，stdout 输出 JSON {"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": "..."}}
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

# 🆕 B1 修复：SKILL 裸文件名 → 内置 target 路径映射表
# 依据：state.next_step_suggestion() 返回的 7 个 skill 文件名 + 它们在 source/skills/ 的真实路径。
# registry.yaml 的 replaces 字段用的就是这个内置路径（见 plugin-registry-spec.md）。
# 只覆盖 next_step_suggestion 会返回的文件名，不扩范围（KISS）。
_SKILL_FILE_TO_BUILTIN_TARGET: dict[str, str] = {
    "requirement-analysis-skill.md": "source/skills/phase1-design/requirement-analysis-skill.md",
    "dr-generate-skill.md":          "source/skills/phase1-design/dr-generate-skill.md",
    "story-generate-skill.md":       "source/skills/phase1-design/story-generate-skill.md",
    "story-review-skill.md":         "source/skills/phase1-design/story-review-skill.md",
    "testcase-generate-skill.md":    "source/skills/phase1-design/testcase-generate-skill.md",
    "testcase-review-skill.md":      "source/skills/phase1-design/testcase-review-skill.md",
    "task-generate-skill.md":        "source/skills/phase2-task/task-generate-skill.md",
    "coding-process-skill.md":       "source/skills/phase2-coding/coding-process-skill.md",
    "coding-skill.md":               "source/skills/phase2-coding/coding-skill.md",
    "coding-report-skill.md":        "source/skills/phase2-coding/coding-report-skill.md",
    "test-generate-skill.md":        "source/skills/phase3-review/test-generate-skill.md",
    "test-review-skill.md":          "source/skills/phase3-review/test-review-skill.md",
    "code-review-skill.md":          "source/skills/phase3-review/code-review-skill.md",
}


def _resolve_skill_path(skill_file: str, ade_sdd, master) -> Optional[str]:
    """🆕 B1 修复：把 next_step_suggestion 返回的 SKILL 裸文件名过一遍 plugin_loader。

    - 命中外挂 → 返回 "外挂名 @ 命中层 → 外挂绝对路径"
    - fallback 内置 / 映射表未覆盖 / 任何异常 → 返回 None（保持原行为）

    设计：与 entry-token / drift 探测同模式（try/except 降级，绝不阻断主流程）。
    """
    if not skill_file or skill_file in ("?", "—"):
        return None
    target = _SKILL_FILE_TO_BUILTIN_TARGET.get(skill_file)
    if not target:
        return None
    try:
        from lib import plugin_loader
        result = plugin_loader.resolve_skill(target, ade_sdd, master)
        if result.plugin and result.resolved_path:
            return f"{result.plugin.name} @ {result.layer_label} → {result.resolved_path}"
    except Exception:
        pass  # plugin_loader 异常不阻断注入，降级为原 skill 裸文件名
    return None

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


# 🆕 v3.8.2：取端注入——memory enter 后的活跃 scope 下注入历史记忆。
# 解决 memory 只写不读的黑洞问题：LLM 每次对话能看到当前 phase+story 的历史决策。
# 注入 task/project compact memory，task 优先、project 只补剩余预算；scratch/事件流不注入。
_MEMORY_INJECT_LIMIT = 8


def _inject_memory_block(ade_sdd: Path, phase: str, current_story: str) -> Optional[str]:
    """构建 ◆ MEMORY 注入块。活跃 scope 返回文本，否则返回 None。

    仅在 memory enter 后未 exit 的活跃 scope 下注入，避免未 enter 时噪声。
    容错：任何异常静默降级返回 None（与 plugin_loader/drift 探测同模式）。
    """
    try:
        from lib import memory_gate, memory_store
        memory_phase = memory_gate.memory_phase_for_state_phase(phase)
        if not memory_phase:
            return None
        story = current_story if current_story and current_story != "（未设定）" else None
        scope = memory_store.locate_scope(
            project=str(ade_sdd.parent), phase=memory_phase, story=story,
        )
        if not memory_store.is_scope_active(scope):
            return None
        task_entries = [
            e for e in memory_store.read(scope, memory_scope="task", limit=0)
            if e.get("type") == "memory"
        ]
        project_entries = [
            e for e in memory_store.read(scope, memory_scope="project", limit=0)
            if e.get("type") == "memory"
        ]
        task_selected = task_entries[-_MEMORY_INJECT_LIMIT:]
        project_slots = max(_MEMORY_INJECT_LIMIT - len(task_selected), 0)
        memory_entries = task_selected + (project_entries[-project_slots:] if project_slots else [])
        if not memory_entries:
            return None
        lines = [f"MEMORY compact task-first phase={memory_phase} story={story or '<project>'}"]
        for e in memory_entries:
            layer = e.get("layer", "L1")
            scope_name = e.get("memoryScope") or memory_store.memory_scope_for_layer(layer)
            kind = e.get("kind", "observation")
            summary = e.get("summary", "")
            evidence = e.get("evidence") or []
            ev_str = ", ".join(evidence) if evidence else "-"
            lines.append(f"- [{scope_name} {kind}] {summary} ev: {ev_str}")
        return "\n".join(lines)
    except Exception:
        return None


def inject(
    project_dir: Optional[Path] = None,
    user_prompt: str = "",
    session_key: str = "",
) -> dict:
    """
    生成注入到 AI context 的 JSON payload。

    Returns:
        dict → {"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": "..."}} 或 {}
    """
    from lib import gates as gates_mod, paths, state as state_mod, work_item_context
    from lib.stop_check import reset_retry

    # 🆕 v3.9.3：触发检测提前到 ade_sdd 判空前，确保未初始化项目也能收到提示
    _is_ae_sdd_triggered = any(m in user_prompt for m in AE_SDD_TRIGGER_MARKERS)

    ade_sdd = paths.locate_project_ae_sdd(project_dir)
    if ade_sdd is None:
        if _is_ae_sdd_triggered:
            return _inject_uninitialized_block(project_dir)
        return {}

    # 清除待初始化标记（已找到 .ae-sdd/，说明项目已初始化）

    # 每次对话开始：重置 Stop hook 重试计数
    reset_retry(ade_sdd)

    # 更新快速通道状态（让 PreToolUse hook 能读到）
    _update_quick_channel(ade_sdd, user_prompt)

    # 🆕 v3.9.3：清理待初始化标记（与 quick_channel 同理，非触发消息时清除）
    if not _is_ae_sdd_triggered:
        _clear_pending_init(project_dir)

    cfg = paths.read_config(ade_sdd)
    project_key = cfg.get("projectKey", "unknown")

    # Read the work-item state, not the project-global mirror, unless this is a
    # legacy project with no isolated work-item states.
    try:
        resolved_state = work_item_context.resolve_default_state(
            ade_sdd,
            session_key=session_key,
            prompt_text=user_prompt,
            bind_session=True,
        )
    except work_item_context.AmbiguousWorkItemError as e:
        return _inject_work_item_ambiguity(project_key, e)
    except work_item_context.NoWorkItemStateError as e:
        return _inject_work_item_required(project_key, e)

    st = resolved_state.data
    resolved_state_path = resolved_state.path
    # 🆕 v3.9.0 嵌套 state 兼容：用统一接口读 active phase/story
    # - nested state: 取 activeStory 子状态的 phase
    # - flat state: 取顶层 phase / currentStory（v1 行为不变）
    phase = state_mod.get_active_phase(st)
    current_story = state_mod.get_active_story(st) or "（未设定）"

    # 🆕 v3.9.0 R5：STORY-ID 一致性检测——用户引用的 Story 与 activeStory 不一致时提醒
    story_mismatch_msg: Optional[str] = None
    # 🆕 v3.9.2 P0-2：新 Story 未被管理时自动创建 work item 并 activate
    _auto_activate_story: Optional[str] = None
    try:
        from lib.classify import extract_requirement_features, match_state
        features = extract_requirement_features(user_prompt, ade_sdd.parent)
        if features.story_ids and features.modifies_story:
            active = current_story if current_story != "（未设定）" else None
            mentioned = features.story_ids[0]
            if active and mentioned != active:
                # 查该 mentioned Story 是否已被某 state 管理
                hit = paths.find_nested_state_by_story_id(ade_sdd, mentioned)
                if hit:
                    target_path, _ = hit
                    story_mismatch_msg = (
                        f"⚠️ R5 重定位提醒：用户引用 {mentioned}，当前 activeStory={active}。"
                        f"该 Story 已被 {target_path.parent.name} 管理，"
                        f"建议跑 `ae-sdd state relocate --story {mentioned}` 重定位。"
                    )
                else:
                    # 🆕 v3.9.2 P0-2：标记待自动 activate（ae-sdd 触发时执行）
                    _auto_activate_story = mentioned
                    story_mismatch_msg = (
                        f"⚠️ Story 切换：当前 activeStory={active}，用户引用 {mentioned}。"
                        f"{mentioned} 未被任何 state 管理，将在 ae-sdd 触发时自动创建新 work item。"
                    )
    except Exception:
        pass  # 一致性检测失败不阻断注入

    # 🆕 v3.4.0 关卡1：/ae-sdd 触发词检测 + entry token 提醒（建议书4）
    entry_reminder: list[str] = []
    if _is_ae_sdd_triggered:
        # 🆕 v3.9.3 P-R2：ae-sdd 触发 + 新 Story 未被管理 → 先查 find_nested_state_by_story_id
        # 找到 → 切到该 state（用现成的顶层名）
        # 找不到 → 提示用户跑 state new，不再自动用 (id=id) 怪异名建 state
        if _auto_activate_story:
            try:
                from lib import paths as paths_mod2
                work_item_id = _auto_activate_story
                # R2 查归属 state
                hit = paths_mod2.find_nested_state_by_story_id(ade_sdd, work_item_id)
                if hit:
                    sp_new, st_new = hit
                    work_item_key = sp_new.parent.name
                    story_mismatch_msg = (
                        f"✅ Story {work_item_id} 已被 state {work_item_key} 管理（R2 命中），"
                        f"active 已切换。"
                    )
                else:
                    # 找不到归属 state → 不再自动建 (id=id) 怪异名 state
                    # 改为提示用户跑 state new（带 R2 预检）
                    work_item_key = ""
                    sp_new = None
                    st_new = None
                    story_mismatch_msg = (
                        f"⚠️ Story {work_item_id} 未被任何嵌套 state 管理。\n"
                        f"   🆕 v3.9.3 起不再自动创建怪异名 state（防 STORY-X--STORY-X 蔓延）。\n"
                        f"   请先跑：\n"
                        f"     ae-sdd state new --id {work_item_id} --entry-node STORY"
                        f" [--story-ids {work_item_id}] [--nested]\n"
                        f"   工具会扫描 design/ 找父级 DR/PRD 文档并验证关联性。"
                    )
                if sp_new and st_new is not None:
                    # 更新 mirror（active 切到该 work item）
                    mirror = dict(st_new)
                    work_item_context.bind_session_state(
                        ade_sdd,
                        session_key,
                        sp_new,
                        work_item_key,
                        work_item_id,
                    )
                    # 刷新后续使用的 current_story / st
                    st = st_new
                    resolved_state_path = sp_new
                    current_story = work_item_id
            except Exception:
                pass  # 自动激活失败不阻断注入

        try:
            from lib import session as session_mod
            # 🆕 v3.9.3 优先用 (top_node, features) 校验，否则用项目级
            cur_story = state_mod.get_active_story(st) or ""
            has_token = False
            if cur_story:
                # 反查 active state 拿顶层名
                hit = None
                if cur_story.upper().startswith("STORY-"):
                    hit = paths.find_nested_state_by_story_id(ade_sdd, cur_story)
                if hit:
                    sp_t, _ = hit
                    top_node = sp_t.parent.name
                    # 简化：用顶层目录名作为 features key
                    if top_node.startswith("PRD-"):
                        has_token = session_mod.has_valid_entry_token(ade_sdd, top_node="PRD", features={"prd_feature": top_node[4:]})
                    elif top_node.startswith("DR-"):
                        has_token = session_mod.has_valid_entry_token(ade_sdd, top_node="DR", features={"dr_feature": top_node[3:]})
                    elif top_node.startswith("Story-"):
                        has_token = session_mod.has_valid_entry_token(ade_sdd, top_node="STORY", features={"story_ids": [cur_story]})
                    else:
                        has_token = session_mod.has_valid_entry_token(ade_sdd, cur_story)
                else:
                    # fallback：按当前 Story 查 token，底层兼容 R6 与 legacy raw Story 目录
                    has_token = session_mod.has_valid_entry_token(ade_sdd, cur_story)
            else:
                has_token = session_mod.has_valid_entry_token(ade_sdd)
            if not has_token:
                enter_cmd = (f"ae-sdd enter {project_key} --work-item <R6 顶层名>"
                             if cur_story else f"ae-sdd enter {project_key}")
                entry_reminder.append(
                    f"⚠️ 检测到 ae-sdd 触发，必须先领取流程凭证（关卡1入口关卡）：\n"
                    f"   {enter_cmd}\n"
                    f"   未领凭证的流程产物落地/代码改动将被关卡2/3 物理拦截。\n"
                    f"   （v3.9.3 起 work-item 必须传 R6 顶层名，如 Story-003 / DR-CS）"
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
        # 🆕 v3.9.2 P0-2：state write 建议带上 --story，确保路由到正确的 work-item state
        story_flag = f" --story {current_story}" if current_story and current_story != "（未设定）" else ""
        next_line = f"  next:     {suggestion['action']}  →  ae-sdd state write{story_flag} --phase {next_phase}"
    else:
        next_line = f"  next:     {suggestion['action']}  （项目已完成，无需切换阶段）"

    # 🆕 B1 修复：把 next-step skill 文件名过 plugin_loader，命中外挂则注入实际路径
    plugin_line = _resolve_skill_path(suggestion['skill'], ade_sdd, master)

    lines = [
        f"<!-- ae-sdd harness 自动注入 @ {now} -->",
        f"◆ HARNESS STATE",
        f"  project:  {project_key}",
        f"  phase:    {phase}",
        f"  story:    {current_story}",
        f"  G-00:     {g00_status}",
        next_line,
        f"  skill:    {suggestion['skill']}",
    ]
    if plugin_line:
        lines.append(f"  plugin:   {plugin_line}  ⚠️ 本次必须加载此 外挂路径，禁用内置")
    lines.append(f"<!-- /ae-sdd harness -->")

    if not g00.pass_:
        lines.insert(1,
            f"⛔ G-00 未通过，禁止继续任何业务操作。"
            f"修复: ae-sdd assets generate --project {project_key}"
        )

    # 🆕 v3.4.0 关卡1：entry token 提醒前置（最高优先级，在 G-00 之前）
    if entry_reminder:
        lines.insert(1, "\n".join(entry_reminder))

    # 🆕 v3.9.0 R5：Story 一致性提醒（在 entry token 之后、G-00 之前）
    if story_mismatch_msg:
        lines.insert(1, story_mismatch_msg)

    # 🆕 v3.4.0：母版版本漂移探测（仅文本提醒，不阻断）
    # 检测已安装的 ae-sdd SKILL 是否落后于母版，若落后则在注入块末尾追加提醒
    try:
        from lib.paths import compare_versions, MASTER_VERSION  # type: ignore
        installed_v = MASTER_VERSION
        master_v = _read_project_master_version(ade_sdd)
        drift = compare_versions(installed_v, master_v or MASTER_VERSION)
        if drift and master_v and master_v != installed_v:
            lines.append(f"⚠️  master-freshness: {drift}")
            lines.append("   建议: bash scripts/dev-sync.sh  或  ae-sdd install --target-path ~/.zcode/skills/ae-sdd")
    except Exception:
        pass  # 探测失败不影响主流程

    # 🆕 v3.6 主流程监管器：偏移检测 + 矫正注入（决策 1B/2B）
    flow_msg = _run_flow_monitor(ade_sdd, st, resolved_state_path)
    if flow_msg:
        lines.append(flow_msg)

    # 🆕 v3.9.11：反模式检测 — 防止 user_prompt 中出现非标 phase 名
    # 根因（v3.9.10 life 实测）：用户/AI 在 prompt 里写 `step-4-test-generate`、
    #   `step-3-doc-format-adjustment` 等 step-X- 自由命名，state.json 的 currentStep 字段
    #   也存这种值，但 ae-sdd PHASE_FLOWS 不含，导致 hook 校验走不通（G-00 fail、gate 拒绝写）。
    # 修复：UserPromptSubmit 注入时正则扫 user_prompt，若含 step-X- 模式或非 PHASE_FLOWS phase 名，
    #   追加反模式警告块，列出 PHASE_FLOWS[scale] 合法值，引导 LLM 用标准 phase 名。
    if _is_ae_sdd_triggered:
        _antipattern_msg = _detect_phase_naming_antipattern(user_prompt, phase)
        if _antipattern_msg:
            lines.append(_antipattern_msg)

    # 🆕 v3.8.2：取端注入——memory enter 后的活跃 scope 下注入历史记忆。
    # 解决 memory 只写不读黑洞：LLM 进节点时能看到该 story 此前 phase 的决策证据。
    memory_block = _inject_memory_block(ade_sdd, phase, current_story)
    if memory_block:
        lines.append(memory_block)

    return {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": "\n".join(lines),
        }
    }


def _inject_work_item_ambiguity(project_key: str, exc) -> dict:
    from datetime import datetime as _dt, timezone as _tz
    now = _dt.now(_tz.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines = [
        f"<!-- ae-sdd harness automatic injection @ {now} -->",
        "WORK-ITEM AMBIGUITY",
        str(exc),
        "Do not infer phase/story from .ae-sdd/state.json in a multi-work-item project.",
        f"project: {project_key}",
        "<!-- /ae-sdd harness -->",
    ]
    return {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": "\n".join(lines),
        }
    }


def _inject_work_item_required(project_key: str, exc) -> dict:
    from datetime import datetime as _dt, timezone as _tz
    now = _dt.now(_tz.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines = [
        f"<!-- ae-sdd harness automatic injection @ {now} -->",
        "WORK-ITEM STATE REQUIRED",
        str(exc),
        "Project-level .ae-sdd/state.json is not a valid active state source.",
        f"project: {project_key}",
        "<!-- /ae-sdd harness -->",
    ]
    return {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": "\n".join(lines),
        }
    }


def _read_project_master_version(ade_sdd: Optional[Path]) -> Optional[str]:
    """🆕 v3.4.0：从业务仓 .ae-sdd/config.yaml 读 master.version。"""
    if ade_sdd is None:
        return None
    cfg = ade_sdd / "config.yaml"
    if not cfg.is_file():
        return None
    try:
        import re
        text = cfg.read_text(encoding="utf-8")
        m = re.search(r"master:\s*\n(?:\s+\S+:\s*\S+\s*\n)*\s+version:\s*\"?([\d.]+)\"?", text)
        if m:
            return m.group(1)
    except OSError:
        pass
    return None


def _run_flow_monitor(ade_sdd: Path, state: dict, state_path: Optional[Path] = None) -> Optional[str]:
    """🆕 v3.6 主流程监管器：每轮 UserPromptSubmit 时执行偏移检测 + 矫正注入。

    决策 1B：废弃 ◆ STATE: 自报标记，完全依赖产物核查（gates check）
    决策 2B：监管器实体 = 本 hook Python 逻辑

    流程：
      1. 接收调用方传入的 state dict（已由 inject() 读取）
      2. 调 flow_monitor.detect_drift() 运行产物核查
      3. severity == 0 → 无偏移，返回 None
      4. severity >= 1 → 递增 correctionCounts[phase] + 写 state.json
      5. severity == 3 → 同时写 state.phase=paused（Level 3 暂停）
      6. 生成矫正注入文本并返回（追加到 additionalContext）

    全流程 try/except：任何异常降级返回 None，不阻断主流程。

    Args:
        ade_sdd: .ae-sdd/ 目录路径
        state:   inject() 已读取的 state dict（只读参考，不直接修改）

    Returns:
        矫正注入文本（str）或 None（无偏移/异常降级）
    """
    try:
        from lib import flow_monitor as fm, state as state_mod, paths as paths_mod

        # 偏移检测（Layer 1 产物核查 + Layer 3 矫正次数）
        drift = fm.detect_drift(state, ade_sdd)

        if drift.severity == 0:
            return None  # 无偏移

        # 递增矫正计数并持久化
        if state_path is None:
            return None
        fresh_state = state_mod.read_state(state_path)  # 重新读取确保原子性
        state_mod.increment_correction(fresh_state, drift.phase)

        # severity == 3：同时写 state.phase=paused（Level 3 暂停）
        if drift.severity >= 3:
            state_mod.pause_state(
                fresh_state,
                pause_reason="level3-escalation",
                by="flow-monitor",
            )

        state_mod.write_state(state_path, fresh_state)

        # 更新 drift 中的矫正计数（写入后的真实值）
        drift.correction_count = state_mod.get_correction_count(fresh_state, drift.phase)

        # 生成矫正文本
        msg = fm.build_correction_message(drift)
        return msg if msg else None

    except Exception:
        return None  # 任何异常降级放行，不阻断主流程


# ─── 🆕 v3.9.3：未初始化项目提示 ───────────────────────────────────────────────

def _inject_uninitialized_block(project_dir: Optional[Path]) -> dict:
    """用户触发 /ae-sdd 但项目未初始化 → 注入"请先 init"状态块 + 写标记文件。

    标记文件供 gate_intercept 跨 hook 读取，用于拦截未初始化项目的写操作。
    """
    from lib import paths as _paths
    marker = _paths.pending_init_marker(project_dir)
    try:
        marker.write_text("ae-sdd pending init", encoding="utf-8")
    except OSError:
        pass  # 标记文件写入失败不阻断注入

    from datetime import datetime as _dt, timezone as _tz
    now = _dt.now(_tz.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    cwd = (project_dir or Path.cwd()).resolve()

    lines = [
        f"<!-- ae-sdd harness 自动注入 @ {now} -->",
        f"⛔ ae-sdd 尚未在此项目初始化（未找到 {cwd}/.ae-sdd/config.yaml）",
        f"",
        f"◆ HARNESS STATE",
        f"  project:  （未初始化）",
        f"  phase:    （未初始化）",
        f"  story:    （未设定）",
        f"  G-00:     🔴 BLOCKED（项目资产缺失 — 尚未 ae-sdd init）",
        f"  next:     初始化项目  →  ae-sdd init <项目目录> <projectKey>",
        f"  skill:    —",
        f"",
        f"⚡ 修复步骤：",
        f"  1. 确认 projectKey（如 ae-sdd、life、order 等）",
        f"  2. 运行: ae-sdd init . <projectKey>  # 自动生成 baseline 项目资产",
        f"  3. 重新触发: /ae-sdd",
        f"",
        f"⛔ G-00 未通过，禁止继续任何业务操作。",
        f"<!-- /ae-sdd harness -->",
    ]

    return {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": "\n".join(lines),
        }
    }


def _clear_pending_init(project_dir: Optional[Path]) -> None:
    """非触发消息时清除待初始化标记（与 quick_channel 清理同理）。"""
    from lib import paths as _paths
    marker = _paths.pending_init_marker(project_dir)
    try:
        marker.unlink(missing_ok=True)
    except OSError:
        pass


# ─── 🆕 v3.9.11：phase 命名反模式检测 ────────────────────────────────────────
import re as _re  # 模块级 import 避免每次调用重导

# step-X- 自由命名模式（如 step-4-test-generate / step-3-doc-format-adjustment）
_STEP_PATTERN = _re.compile(r"\bstep-\d+-[a-z][a-z0-9-]*", _re.IGNORECASE)


def _detect_phase_naming_antipattern(user_prompt: str, current_phase: str) -> Optional[str]:
    """检测 user_prompt 中是否出现非标 phase 命名（step-X- 自由命名）。

    根因（v3.9.10 life 实测）：
      用户/AI 在 prompt 里写 `step-4-test-generate`、`step-3-doc-format-adjustment`
      等 step-X- 自由命名。state.json 的 currentStep 字段也存这种值，但 ae-sdd
      PHASE_FLOWS 不含此类名，导致：
        - hook 校验走不通（G-00 fail、gate 拒绝写）
        - state.json phase 字段缺失时 hook 读到 None 误判
        - 复盘难度高（命名不统一，无法机械 grep）

    修复策略：检测到 step-X- 模式时，返回警告块文本，由 inject() 追加到注入块末尾。
    警告块列出当前 scale 的 PHASE_FLOWS 合法值，引导 LLM 用标准 phase 名。

    Args:
        user_prompt: 用户原始消息文本
        current_phase: 当前 state 的 phase（用于推断 scale 和列出合法 phase）

    Returns:
        警告块文本（多行字符串）或 None（未检测到反模式）
    """
    if not user_prompt:
        return None
    matches = _STEP_PATTERN.findall(user_prompt)
    if not matches:
        return None

    # 收集 PHASE_FLOWS 合法值（按 scale 列出，便于 LLM 选择）
    try:
        from lib.state import PHASE_FLOWS
        valid_phases: list[str] = []
        for scale, chain in PHASE_FLOWS.items():
            valid_phases.append(f"  [{scale}] {', '.join(chain)}")
        valid_text = "\n".join(valid_phases)
    except Exception:
        valid_text = "  (PHASE_FLOWS 读取失败，参考 ae-sdd 文档)"

    unique_matches = list(dict.fromkeys(matches))  # 去重保序
    return (
        f"⛔ PHASE 命名反模式检测：user_prompt 含非标命名 {unique_matches}。\n"
        f"   ae-sdd PHASE_FLOWS 不识别 step-X-* 命名（仅 state.json.currentStep 字段可存自由文本）。\n"
        f"   state write / gate / hook 校验全部基于标准 phase 名，混用会导致：\n"
        f"     - G-00 项目资产门禁阻断（state 缺 phase 字段）\n"
        f"     - PreToolUse hook 拒绝写操作（phase 不在 PHASE_PERMIT 表）\n"
        f"     - _PRODUCT_PHASE_MAP 产物-Phase 映射失配（TestCase 仅 testcase-generated/testcase-reviewed 允许写）\n"
        f"   合法 phase 名（按 scale）：\n{valid_text}\n"
        f"   若需切换 phase：ae-sdd state write --phase <标准 phase 名> --story <STORY-ID>\n"
        f"   （🆕 v3.9.11 反模式防护，life 项目事故复盘）"
    )
