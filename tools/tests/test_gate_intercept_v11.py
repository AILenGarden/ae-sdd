"""
test_gate_intercept_v11.py — gate_intercept v1.1 新功能测试

覆盖：
  1. 状态机自保护（ae-sdd state write gate 验证）
  2. 路径感知（设计阶段禁止写源码）
  3. 快速通道不豁免 G-00（行为验证）
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib.gate_intercept import (
    BASH_READONLY_PREFIXES,
    _check_path_permission,
    _check_state_write,
    _extract_target_phase,
    _is_readonly_bash,
    _is_source_code_path,
    check_intercept,
)


def _make_work_item_state(tmp_path: Path, work_item: str, story: str,
                          phase: str) -> Path:
    """建 task-scoped work-item state（.auto-engineering/<id>/state.json）。

    v3.9.13 起 state 源从项目级 .ae-sdd/state.json 改为
    .auto-engineering/<work-item>/state.json。check_intercept / prompt_inject
    均经 resolve_default_state() 扫描 .auto-engineering/*/state.json，恰好 1 个
    未 completed 的 work-item 即命中。本 helper 写 nested state（stateModel=nested），
    兼容 get_active_phase/get_active_story。
    """
    wi_dir = tmp_path / ".auto-engineering" / work_item
    wi_dir.mkdir(parents=True, exist_ok=True)
    sp = wi_dir / "state.json"
    sp.write_text(json.dumps({
        "stateModel": "nested",
        "activeStory": story,
        "storyStates": {story: {"phase": phase}},
    }, ensure_ascii=False, indent=2), encoding="utf-8")
    return sp


# ─── 状态机自保护 ─────────────────────────────────────────────────────────────

class TestStateMachineProtection:
    """ae-sdd state write 不再走只读白名单，必须通过 gate 验证"""

    def test_state_write_not_in_readonly_prefixes(self):
        """ae-sdd state write 不应命中只读白名单"""
        cmd = "ae-sdd state write --phase coding"
        assert not _is_readonly_bash(cmd), (
            "ae-sdd state write 不应被视为只读命令，否则 AI 可以随意跳 phase"
        )

    def test_readonly_ae_sdd_commands_still_pass(self):
        """只读 ae-sdd 命令仍然放行"""
        for cmd in [
            "ae-sdd state read",
            "ae-sdd state next-step",
            "ae-sdd gates check",
            "ae-sdd classify --text hello",
            "ae-sdd version",
            "ae-sdd health",
        ]:
            assert _is_readonly_bash(cmd), f"只读命令应放行: {cmd}"

    def test_extract_target_phase_normal(self):
        assert _extract_target_phase("ae-sdd state write --phase coding") == "coding"
        assert _extract_target_phase("ae-sdd state write --phase task-reviewed --story STORY-001") == "task-reviewed"

    def test_extract_target_phase_none_for_read(self):
        assert _extract_target_phase("ae-sdd state read") is None
        assert _extract_target_phase("ae-sdd gates check") is None

    def test_check_state_write_skip_backward(self):
        """回退不拦截"""
        allowed, _ = _check_state_write(
            "ae-sdd state write --phase initialized",
            current_phase="coding",
            ade_sdd=None,
            project_key="test",
        )
        assert allowed, "回退到早期 phase 不应被拦截（允许人工修正）"

    def test_check_state_write_skip_same(self):
        """原地不拦截"""
        allowed, _ = _check_state_write(
            "ae-sdd state write --phase initialized",
            current_phase="initialized",
            ade_sdd=None,
            project_key="test",
        )
        assert allowed

    def test_check_state_write_block_skip_multiple_steps(self):
        """跨步跳跃始终拦截（不管 gate）"""
        allowed, reason = _check_state_write(
            "ae-sdd state write --phase coding",   # 从 initialized 跳 5 步
            current_phase="initialized",
            ade_sdd=None,
            project_key="test",
        )
        assert not allowed
        assert "跨步跳跃" in reason or "跳了" in reason

    def test_check_state_write_next_step_no_ae_sdd(self, tmp_path):
        """
        向前跳 1 步 + 无 .ae-sdd →
        gate 跑不完整（G-01 会失败因为没有 design/ 目录），应被拦截。
        这是有意为之：没有 .ae-sdd 就无法跑 gate，切 phase 应被阻止。
        v3.14：initialized → route-selected 是单步（G-00 失败因无 .ae-sdd）。
        """
        allowed, reason = _check_state_write(
            "ae-sdd state write --phase route-selected",
            current_phase="initialized",
            ade_sdd=None,
            project_key="test",
        )
        # route-selected 入口门禁 = [G-00]；无 .ae-sdd → G-00 失败 → 拒绝
        assert not allowed
        assert "gate" in reason.lower() or "G-00" in reason or "资产" in reason or "init" in reason

    def test_state_write_intercepted_by_check_intercept(self, tmp_path):
        """
        check_intercept 集成测试：
        有 .ae-sdd（initialized）+ state write 跨步跳跃 → 被拦截
        """
        # 创建最小 .ae-sdd 结构
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # state 源为 task-scoped work-item state（v3.9.13），phase=initialized
        _make_work_item_state(tmp_path, "Story-001", "STORY-001", "initialized")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state write --phase coding",  # 跨 5 步
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed
        assert "跨步" in reason or "跳了" in reason


# ─── 路径感知 ─────────────────────────────────────────────────────────────────

class TestPathAwareness:
    """设计阶段禁止写入 src/ 源码目录"""

    @pytest.mark.parametrize("path", [
        "src/main/java/com/example/UserService.java",
        "src/main/kotlin/com/example/Service.kt",
        "src/test/java/com/example/UserServiceTest.java",
        "src/test/kotlin/ServiceTest.kt",
    ])
    def test_source_code_path_detected(self, path):
        assert _is_source_code_path(path), f"应识别为源码路径: {path}"

    @pytest.mark.parametrize("path", [
        "design/STORY-001.md",
        "ae-sdd-doc/iterations/2026-06-22/Story/STORY-001.md",
        ".ae-sdd/state.json",
        "README.md",
        "pom.xml",
    ])
    def test_doc_path_not_source(self, path):
        assert not _is_source_code_path(path), f"不应识别为源码路径: {path}"

    @pytest.mark.parametrize("phase", [
        "initialized", "ra-generated", "dr-generated", "story-generated",
        "story-reviewed", "task-generated",
    ])
    def test_write_java_blocked_in_design_phases(self, phase):
        allowed, reason = _check_path_permission(
            "Write",
            "src/main/java/com/example/Service.java",
            phase,
        )
        assert not allowed, f"{phase} 应禁止写 Java 源码"
        assert "设计阶段" in reason or "src/" in reason

    @pytest.mark.parametrize("phase", [
        "task-reviewed", "coding", "test-running",
    ])
    def test_write_java_allowed_in_coding_phases(self, phase):
        allowed, _ = _check_path_permission(
            "Write",
            "src/main/java/com/example/Service.java",
            phase,
        )
        assert allowed, f"{phase} 应允许写 Java 源码"

    def test_write_doc_always_allowed(self):
        """文档路径任何 phase 都允许"""
        for phase in ["initialized", "coding", "completed"]:
            allowed, _ = _check_path_permission(
                "Write",
                "design/STORY-001.md",
                phase,
            )
            assert allowed, f"{phase} 应允许写文档"

    def test_completed_still_blocks_source(self):
        """completed phase Write 在 PHASE_PERMIT 里是空集，整体被 phase 检查拦截"""
        allowed, _ = check_intercept(
            "Write",
            file_path="src/main/java/Service.java",
            forced_phase="completed",
        )
        assert not allowed

    def test_path_check_passes_for_none_path(self):
        """file_path=None 时不做路径检查"""
        allowed, _ = _check_path_permission("Write", None, "initialized")
        assert allowed

    def test_path_check_passes_for_non_write(self):
        """非 Write/Edit 工具不做路径检查"""
        allowed, _ = _check_path_permission("Read", "src/main/java/X.java", "initialized")
        assert allowed

    def test_check_intercept_with_file_path(self, tmp_path):
        """集成：有 .ae-sdd（initialized）+ 写 Java 文件 → 被拦截"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # state 源为 task-scoped work-item state（v3.9.13），phase=initialized
        _make_work_item_state(tmp_path, "Story-001", "STORY-001", "initialized")
        allowed, reason = check_intercept(
            "Write",
            file_path="src/main/java/Service.java",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed
        assert "设计阶段" in reason


# ─── prompt_inject ────────────────────────────────────────────────────────────


def _additional_context(result: dict) -> str:
    """从 prompt_inject 返回结构中提取 additionalContext（Harness 新格式）。

    prompt_inject 自 v3.5.8 ra-review-loop-unification 起输出
    ``{"hookSpecificOutput": {"hookEventName": "UserPromptSubmit",
                              "additionalContext": "..."}}``，
    旧 ``systemMessage`` key 已废弃。统一用本 helper 提取，避免断言再次漂移
    （与 tools/tests/test_prompt_inject_plugin.py:_additional_context 同构）。
    """
    return result.get("hookSpecificOutput", {}).get("additionalContext", "")


class TestPromptInject:
    """prompt_inject v1.2：返回 dict，无 as_json 参数"""

    def test_no_ae_sdd_returns_empty_dict(self, tmp_path):
        from lib.prompt_inject import inject
        result = inject(project_dir=tmp_path, user_prompt="普通消息", session_key="prompt-state")
        assert result == {}

    def test_with_ae_sdd_injects_state(self, tmp_path):
        from lib.prompt_inject import inject
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # state 源为 task-scoped work-item state（v3.9.13），phase=coding, STORY-001
        _make_work_item_state(tmp_path, "Story-001", "STORY-001", "coding")
        (ae_sdd / "assets").mkdir()

        result = inject(project_dir=tmp_path, user_prompt="/ae-sdd 继续", session_key="prompt-g00")
        msg = _additional_context(result)
        assert msg, "inject 应返回非空 additionalContext"
        assert "◆ HARNESS STATE" in msg
        assert "coding" in msg
        assert "STORY-001" in msg

    def test_g00_fail_adds_warning(self, tmp_path):
        from lib.prompt_inject import inject
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # state 源为 task-scoped work-item state（v3.9.13），phase=initialized
        # 无 assets/ 目录 → G-00 失败 → 注入 ⛔ 警告
        _make_work_item_state(tmp_path, "Story-001", "STORY-001", "initialized")

        result = inject(project_dir=tmp_path, user_prompt="/ae-sdd 继续", session_key="prompt-g00")
        msg = _additional_context(result)
        assert "G-00" in msg or "⛔" in msg

    def test_resets_stop_retry_count(self, tmp_path):
        """inject 应重置 Stop hook 重试计数"""
        from lib.prompt_inject import inject
        from lib.stop_check import get_retry_count, increment_retry

        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))

        # 先把重试计数设为 2
        increment_retry(ae_sdd)
        increment_retry(ae_sdd)
        assert get_retry_count(ae_sdd) == 2

        # inject 后计数应归零
        inject(project_dir=tmp_path, user_prompt="/ae-sdd 继续", session_key="prompt-retry")
        assert get_retry_count(ae_sdd) == 0


# ─── stop_check ───────────────────────────────────────────────────────────────

class TestStopCheck:
    """stop_check v1.2：检查 transcript 内容，持久化重试计数"""

    def test_no_ae_sdd_always_allows(self, tmp_path):
        from lib.stop_check import check_output
        should_stop, _ = check_output("任意输出内容", ade_sdd=None)
        assert should_stop

    def test_with_state_header_allows_stop(self, tmp_path):
        from lib.stop_check import check_output
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()

        transcript = "完成了任务\n◆ STATE: coding/STORY-001\n◆ GATE: ✅ CLEAR"
        should_stop, _ = check_output(transcript, ade_sdd=ae_sdd)
        assert should_stop

    def test_without_state_header_allows_stop(self, tmp_path):
        """v3.6（决策 1B）：废弃 ◆ STATE 自报标记检测——最新响应无状态头 → 放行。

        流程合规性已由 UserPromptSubmit hook（flow_monitor 产物核查）接管，
        Stop hook 不再校验自报标记。
        """
        from lib.stop_check import check_output
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()

        transcript = "我已经帮你完成了任务，代码写好了。"
        should_stop, msg = check_output(transcript, ade_sdd=ae_sdd)
        assert should_stop, "v3.6 废弃自报标记：无状态头应放行"
        assert msg == ""

    def test_retry_count_increments_on_truncation_block(self, tmp_path):
        """v3.6：重试计数仅在结构性阻断（空响应/截断/HS-8 compact 失败）时递增。

        废弃自报标记后，普通无状态头响应不再阻断、不再递增计数；
        只有真正被截断的空响应才阻断并递增。
        """
        from lib.stop_check import check_output, get_retry_count
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()

        # 空响应（结构性截断）→ 阻断 + 递增计数
        check_output("", ade_sdd=ae_sdd)
        assert get_retry_count(ae_sdd) == 1
        check_output("   \n  ", ade_sdd=ae_sdd)
        assert get_retry_count(ae_sdd) == 2

    def test_max_retry_prevents_infinite_loop(self, tmp_path):
        from lib.stop_check import check_output, MAX_RETRY, increment_retry
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()

        # 先把计数打到上限
        for _ in range(MAX_RETRY):
            increment_retry(ae_sdd)

        transcript = "没有状态头的输出"
        should_stop, _ = check_output(transcript, ade_sdd=ae_sdd)
        assert should_stop, f"达到最大重试次数 {MAX_RETRY} 后应放行"

    def test_no_ae_sdd_allows_without_header(self):
        from lib.stop_check import check_output
        should_stop, _ = check_output("没有状态头", ade_sdd=None)
        assert should_stop, "无 .ae-sdd 时应放行（非 ae-sdd 项目）"
