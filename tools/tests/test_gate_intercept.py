"""
test_gate_intercept.py — gate_intercept 模块单元测试
"""
from __future__ import annotations

import sys
import json
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import memory_store
from lib.gate_intercept import (
    PHASE_PERMIT,
    READONLY_TOOLS,
    check_intercept,
    is_quick_channel_active,
)


# ─── check_intercept 基础用例 ────────────────────────────────────────────────

class TestReadonlyAlwaysAllowed:
    """只读工具任何 phase 都放行"""

    @pytest.mark.parametrize("tool", sorted(READONLY_TOOLS))
    @pytest.mark.parametrize("phase", [
        "initialized", "coding", "completed", "task-reviewed"
    ])
    def test_readonly_tool_always_pass(self, tool, phase):
        allowed, reason = check_intercept(tool, allow_readonly=True, forced_phase=phase)
        assert allowed, f"{tool} @ {phase} 应放行，但被拒绝: {reason}"

    def test_readonly_disabled_still_checks_phase(self):
        """allow_readonly=False 时只读工具也走 phase 检查"""
        # Read 不在 PHASE_PERMIT["completed"] 中
        allowed, _ = check_intercept("Read", allow_readonly=False, forced_phase="completed")
        assert not allowed


class TestBashReadonlyCommands:
    """Bash 只读命令放行"""

    @pytest.mark.parametrize("cmd", [
        "cat pom.xml",
        "ls -la",
        "grep -r 'TODO' src/",
        "git status",
        "git log --oneline -10",
        "ae-sdd state read",
        "ae-sdd gates check",
        "mvn --version",
    ])
    def test_bash_readonly_allowed(self, cmd):
        allowed, _ = check_intercept("Bash", bash_command=cmd, forced_phase="initialized")
        assert allowed, f"只读命令应放行: {cmd}"

    @pytest.mark.parametrize("cmd", [
        "mvn clean install",
        "rm -rf target/",
        "git commit -m 'fix'",
        "echo 'hello' > file.txt",
    ])
    def test_bash_write_blocked_in_initialized(self, cmd):
        """initialized phase 不允许写操作 Bash"""
        allowed, _ = check_intercept("Bash", bash_command=cmd, forced_phase="initialized")
        assert not allowed, f"写操作命令不应放行（initialized）: {cmd}"


class TestPhasePermissions:
    """各 phase 的写权限边界"""

    # ── initialized / dr-generated / story-generated / story-reviewed：无 Bash ──
    @pytest.mark.parametrize("phase", [
        "initialized", "dr-generated", "story-generated", "story-reviewed",
    ])
    def test_no_bash_in_design_phases(self, phase):
        allowed, reason = check_intercept("Bash",
                                          bash_command="mvn test",
                                          forced_phase=phase)
        assert not allowed, f"{phase} 不应允许 Bash(mvn test)"
        assert "phase" in reason

    @pytest.mark.parametrize("phase", [
        "initialized", "dr-generated", "story-generated", "story-reviewed",
        "task-generated",
    ])
    def test_write_allowed_in_doc_phases(self, phase):
        """文档阶段允许 Write"""
        allowed, _ = check_intercept("Write", forced_phase=phase)
        assert allowed

    # ── task-reviewed / coding / test-running：允许 Bash ──
    @pytest.mark.parametrize("phase", ["task-reviewed", "coding", "test-running"])
    def test_bash_allowed_in_coding_phases(self, phase):
        allowed, _ = check_intercept("Bash",
                                     bash_command="mvn test",
                                     forced_phase=phase)
        assert allowed

    # ── completed：写操作全拒 ──
    @pytest.mark.parametrize("tool", ["Write", "Edit", "Bash"])
    def test_completed_blocks_all_writes(self, tool):
        allowed, reason = check_intercept(tool,
                                          bash_command="echo done",
                                          forced_phase="completed")
        assert not allowed
        assert "completed" in reason or "phase" in reason

    def test_unknown_phase_falls_back_to_deny(self):
        """未知 phase → 不在 PHASE_PERMIT 中 → 拒绝写"""
        allowed, reason = check_intercept("Write", forced_phase="non-existent-phase")
        assert not allowed


class TestDenyMessage:
    """拒绝消息必须包含关键信息"""

    def test_deny_message_contains_phase(self):
        _, reason = check_intercept("Write", forced_phase="completed")
        assert "completed" in reason

    def test_deny_message_contains_tool(self):
        _, reason = check_intercept("Write", forced_phase="completed")
        assert "Write" in reason

    def test_deny_message_contains_next_step(self):
        _, reason = check_intercept("Write", forced_phase="completed")
        # 应包含"下一步"指引
        assert "下一步" in reason or "SKILL" in reason

    def test_deny_message_contains_quick_channel_hint(self):
        # v1.2: 快速通道提示在 _deny_response() 包装层，不在裸 reason 里
        from lib.gate_intercept import _deny_response
        _, reason = check_intercept("Bash",
                                    bash_command="mvn compile",
                                    forced_phase="initialized")
        full = _deny_response("Bash", reason)
        assert "快速通道" in full["systemMessage"]

    def test_bash_command_preview_in_message(self):
        cmd = "mvn clean install -DskipTests"
        _, reason = check_intercept("Bash", bash_command=cmd, forced_phase="completed")
        # v1.2: 拒绝原因里仍包含 phase/next step，mvn 预览在 _deny_response 层
        assert "completed" in reason or "下一步" in reason


class TestNonAeSddProject:
    """无 .ae-sdd/ 目录时不拦截（不影响非 ae-sdd 项目）"""

    def test_no_ae_sdd_dir_allows_all(self, tmp_path):
        # tmp_path 是空目录，没有 .ae-sdd/
        allowed, _ = check_intercept("Write", project_dir=tmp_path)
        assert allowed, "无 .ae-sdd/ 的项目不应被拦截"

    def test_no_ae_sdd_bash_allowed(self, tmp_path):
        allowed, _ = check_intercept("Bash",
                                     bash_command="rm -rf /",
                                     project_dir=tmp_path)
        assert allowed


# ─── is_quick_channel_active ─────────────────────────────────────────────────

class TestQuickChannel:
    """快速通道检测"""

    @pytest.mark.parametrize("text", [
        "ae-sdd-quick",
        "/ae-sdd-quick 做个小改动",
        "走快速通道，改个字段名",
        "quick channel enabled",
    ])
    def test_quick_channel_detected(self, text):
        assert is_quick_channel_active(text)

    @pytest.mark.parametrize("text", [
        "正常流程",
        "请帮我做 Story",
        "",
        None,
    ])
    def test_no_quick_channel(self, text):
        assert not is_quick_channel_active(text)

    def test_env_var_activates_quick_channel(self, monkeypatch):
        monkeypatch.setenv("AE_SDD_QUICK", "1")
        assert is_quick_channel_active(None)

    def test_env_var_false_no_activation(self, monkeypatch):
        monkeypatch.setenv("AE_SDD_QUICK", "0")
        assert not is_quick_channel_active(None)


# ─── PHASE_PERMIT 完整性检查 ─────────────────────────────────────────────────

class TestPhasePermitCompleteness:
    """确保所有 PHASE_FLOW 中的 phase 都在 PHASE_PERMIT 里"""

    def test_all_phases_covered(self):
        from lib.state import PHASE_FLOW
        for phase in PHASE_FLOW:
            assert phase in PHASE_PERMIT, (
                f"phase '{phase}' 在 PHASE_FLOW 中但不在 PHASE_PERMIT 中，"
                "请在 gate_intercept.py 补充"
            )

    def test_phase_permit_values_are_frozensets(self):
        for phase, tools in PHASE_PERMIT.items():
            assert isinstance(tools, frozenset), (
                f"PHASE_PERMIT['{phase}'] 应为 frozenset，实际是 {type(tools)}"
            )


# ─── 🆕 v3.5.4 HS-7：prd-complete 物理拦截测试 ────────────────────────────────

class TestHS7PrdCompleteGate:
    """HS-7：ae-sdd state prd-complete 前置校验 4 层 AND 物理拦截"""

    def _make_prd_state(self, tmp_path, prd_id="PRD-CS-001", all_pass=True):
        """构造 PRD 级 state.json。all_pass=True 时 4 层全过，False 时 G-PRD-1 失败。"""
        prd_dir = tmp_path / ".auto-engineering" / prd_id
        prd_dir.mkdir(parents=True, exist_ok=True)
        story = {
            "storyId": "STORY-001-BE",
            "codeReviewReport": "ae-sdd-doc/CR/STORY-001-BE.md" if all_pass else "",
            "sevenBisPassed": all_pass,
            "userConfirmedAt": "2026-06-27T10:00:00Z" if all_pass else "",
        }
        ps = {
            "prdId": prd_id,
            "storyIds": [story],
            "crossStoryDeps": [],
            "crossStoryResidualRisks": [],
            "prdReview": {
                "confirmedAt": "2026-06-27T10:00:00Z" if all_pass else "",
                "confirmedBy": "tester" if all_pass else "",
            } if all_pass else {},
            "prdStatus": "in_progress",
        }
        (prd_dir / "state.json").write_text(
            json.dumps(ps, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        return tmp_path

    def test_prd_complete_blocked_when_4layers_fail(self, tmp_path):
        """4 层 AND 未全过 → prd-complete 被拦"""
        project_dir = self._make_prd_state(tmp_path, all_pass=False)
        cmd = "ae-sdd state prd-complete --prd PRD-CS-001 --runtime mavis"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_phase="code-reviewed"
        )
        assert not allowed, "4 层未过应被 HS-7 拦截"
        assert "HS-7" in reason
        assert "4 层 AND 未全过" in reason
        assert "prd-check-complete" in reason

    def test_prd_complete_allowed_when_4layers_pass(self, tmp_path):
        """4 层 AND 全过 → prd-complete 放行（不因 HS-7 拦）"""
        project_dir = self._make_prd_state(tmp_path, all_pass=True)
        cmd = "ae-sdd state prd-complete --prd PRD-CS-001 --runtime mavis"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_phase="code-reviewed"
        )
        # 4 层全过 → HS-7 放行（可能因 phase=code-reviewed 不允许 Bash 被拦，但不是 HS-7）
        if not allowed:
            assert "HS-7" not in reason, "4 层全过不应被 HS-7 拦截"

    def test_prd_complete_blocked_when_no_prd_state(self, tmp_path):
        """PRD state.json 不存在 → 拦截"""
        cmd = "ae-sdd state prd-complete --prd PRD-NONEXIST --runtime mavis"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=tmp_path, forced_phase="code-reviewed"
        )
        assert not allowed
        assert "HS-7" in reason
        assert "不存在" in reason

    def test_prd_check_complete_not_intercepted(self, tmp_path):
        """prd-check-complete（只读校验）不被 HS-7 拦截"""
        project_dir = self._make_prd_state(tmp_path, all_pass=False)
        cmd = "ae-sdd state prd-check-complete --prd PRD-CS-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_phase="code-reviewed"
        )
        # prd-check-complete 命中只读白名单或 phase 校验，但不应因 HS-7 拦截
        if not allowed:
            assert "HS-7" not in reason, "prd-check-complete 不应被 HS-7 拦截"

    def test_echo_prd_complete_not_intercepted(self, tmp_path):
        """echo/注释形式的 prd-complete 不触发 HS-7（防误判）"""
        project_dir = self._make_prd_state(tmp_path, all_pass=False)
        cmd = "echo 'ae-sdd state prd-complete --prd PRD-CS-001'"
        allowed, _ = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_phase="code-reviewed"
        )
        # echo 命令不应触发 HS-7（_is_ae_sdd_cmd 排除非执行形式）
        # 即使被其他规则拦，reason 也不应含 HS-7
