"""
test_gate_intercept.py — gate_intercept 模块单元测试
"""
from __future__ import annotations

import sys
import json
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import memory_store, work_item_context
from lib.gate_intercept import (
    PHASE_PERMIT,
    READONLY_TOOLS,
    check_intercept,
    is_quick_channel_active,
)


def _write_work_item_state(project_dir: Path, data: dict, key: str = "Story-001") -> Path:
    state_path = project_dir / ".auto-engineering" / key / "state.json"
    state_path.parent.mkdir(parents=True, exist_ok=True)
    payload = dict(data)
    payload.setdefault("workItemKey", key)
    payload.setdefault("stateMachineId", key)
    if payload.get("stateModel") == "nested":
        payload.setdefault("entryNode", "STORY")
    else:
        payload.setdefault("currentWorkItem", key)
    state_path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    return state_path


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
        "echo test",
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
        "printf hello > file.txt",
    ])
    def test_bash_write_blocked_in_initialized(self, cmd):
        """initialized phase 不允许写操作 Bash"""
        allowed, _ = check_intercept("Bash", bash_command=cmd, forced_phase="initialized")
        assert not allowed, f"写操作命令不应放行（initialized）: {cmd}"


class TestPhasePermissions:
    """各 phase 的写权限边界"""

    # ── initialized / dr-generated / story-generated / story-reviewed / testcase-*：无 Bash ──
    @pytest.mark.parametrize("phase", [
        "initialized", "dr-generated", "story-generated", "story-reviewed",
        "testcase-generated", "testcase-reviewed",  # 🆕 v3.7.0
    ])
    def test_no_bash_in_design_phases(self, phase):
        allowed, reason = check_intercept("Bash",
                                          bash_command="mvn test",
                                          forced_phase=phase)
        assert not allowed, f"{phase} 不应允许 Bash(mvn test)"
        assert "phase" in reason

    @pytest.mark.parametrize("phase", [
        "initialized", "dr-generated", "story-generated", "story-reviewed",
        "testcase-generated", "testcase-reviewed",  # 🆕 v3.7.0
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
    @pytest.mark.parametrize("tool", ["Write", "Edit"])
    def test_completed_blocks_all_writes(self, tool):
        allowed, reason = check_intercept(tool, forced_phase="completed")
        assert not allowed
        assert "completed" in reason or "phase" in reason

    def test_completed_blocks_non_readonly_bash(self):
        allowed, reason = check_intercept("Bash",
                                          bash_command="mvn test",
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
        allowed, _ = check_intercept("Write", project_dir=tmp_path, forced_engaged=True)
        assert allowed, "无 .ae-sdd/ 的项目不应被拦截"

    def test_no_ae_sdd_bash_allowed(self, tmp_path):
        allowed, _ = check_intercept("Bash",
                                     bash_command="rm -rf /",
                                     project_dir=tmp_path, forced_engaged=True)
        assert allowed


class TestParallelWorkItemHookIsolation:
    """PreToolUse hook must fail closed when multiple work-item states are possible."""

    def _write_work_item(self, tmp_path: Path, name: str, story_id: str, phase: str) -> Path:
        state_path = tmp_path / ".auto-engineering" / name / "state.json"
        state_path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "version": "2",
            "projectKey": "test",
            "stateModel": "nested",
            "entryNode": "STORY",
            "stateMachineId": name,
            "workItemKey": name,
            "currentWorkItem": name,
            "scale": "小",
            "activeStory": story_id,
            "storyStates": {
                story_id: {
                    "phase": phase,
                    "completedSteps": [],
                    "codingRound": 0,
                    "lastUpdated": "2026-07-09T00:00:00Z",
                    "resetHistory": [],
                }
            },
            "history": [],
        }
        state_path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
        return state_path

    def test_write_is_blocked_by_work_item_ambiguity_not_stale_mirror(self, tmp_path):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        sp_a = self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")
        mirror = json.loads(sp_a.read_text(encoding="utf-8"))
        mirror["activeWorkItem"] = "Story-004"
        mirror["activeStatePath"] = str(sp_a)
        (ade_sdd / "state.json").write_text(json.dumps(mirror, ensure_ascii=False), encoding="utf-8")

        target = tmp_path / "src" / "main" / "java" / "Foo.java"
        allowed, reason = check_intercept("Write", file_path=str(target), project_dir=tmp_path, forced_engaged=True)

        assert not allowed
        assert "--work-item" in reason
        assert "Story-004" in reason
        assert "Story-005" in reason

    def test_readonly_bash_allowed_despite_work_item_ambiguity(self, tmp_path):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")

        allowed, reason = check_intercept("Bash", bash_command="echo test", project_dir=tmp_path, forced_engaged=True)

        assert allowed, reason

    def test_explicit_work_item_state_write_resolves_before_ambiguity(self, tmp_path):
        """显式 --work-item 应先定位目标，再执行 state write 校验。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state write --work-item Story-005 --phase coding",
            project_dir=tmp_path,
            forced_engaged=True,
        )

        assert allowed, reason

    def test_quoted_python_path_explicit_work_item_resolves_before_ambiguity(self, tmp_path):
        """带引号脚本路径也必须提取 --work-item，不能退回隐式歧义。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")

        allowed, reason = check_intercept(
            "Bash",
            bash_command=(
                'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd" state write '
                '--work-item Story-005 --phase coding'
            ),
            project_dir=tmp_path,
            forced_engaged=True,
        )

        assert allowed, reason

    def test_explicit_work_item_state_write_binds_session(self, tmp_path, monkeypatch):
        """显式目标在有 session key 时应成为后续 Write/Edit 的会话绑定。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state write --work-item Story-005 --phase coding",
            project_dir=tmp_path,
            forced_engaged=True,
            session_key="session-explicit-005",
        )

        assert allowed, reason
        binding = work_item_context.read_session_binding(ade_sdd, "session-explicit-005")
        assert binding is not None
        assert binding.key == "Story-005"

        monkeypatch.setattr(
            "lib.gate_intercept._check_memory_entered",
            lambda phase, ade_sdd, state_data: (True, ""),
        )
        followup_allowed, followup_reason = check_intercept(
            "Write",
            file_path=str(tmp_path / "README.md"),
            project_dir=tmp_path,
            forced_engaged=True,
            session_key="session-explicit-005",
        )
        assert followup_allowed, followup_reason

    def test_explicit_missing_work_item_is_actionable(self, tmp_path):
        """显式目标不存在时应报告目标，而不是回退到隐式歧义。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state write --work-item Story-999 --phase coding",
            project_dir=tmp_path,
            forced_engaged=True,
        )

        assert not allowed
        assert "Story-999" in reason
        assert "not found" in reason.lower()

    def test_explicit_completed_work_item_preserves_state_write_policy(self, tmp_path):
        """显式 completed 目标应沿用幂等 state write 策略，而不是隐式过滤。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "coding")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state write --work-item Story-005 --phase completed",
            project_dir=tmp_path,
            forced_engaged=True,
        )

        assert allowed, reason

    def test_redirected_echo_still_blocked_by_work_item_ambiguity(self, tmp_path):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "coding")

        allowed, reason = check_intercept(
            "Bash", bash_command="echo test > out.txt", project_dir=tmp_path, forced_engaged=True
        )

        assert not allowed
        assert "--work-item" in reason

    def test_write_uses_session_bound_work_item_without_global_mirror(self, tmp_path):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "story-generated")
        sp_b = self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "initialized")
        work_item_context.bind_session_state(
            ade_sdd, "session-005", sp_b, "Story-005", "STORY-005-BE"
        )

        allowed, reason = check_intercept(
            "Write",
            file_path=str(tmp_path / "notes.txt"),
            project_dir=tmp_path, forced_engaged=True,
            session_key="session-005",
        )

        assert allowed, reason

    def test_completed_session_binding_denies_by_phase_not_ambiguity(self, tmp_path):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-004", "STORY-004-BE", "task-reviewed")
        sp_b = self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")
        work_item_context.bind_session_state(
            ade_sdd, "session-005", sp_b, "Story-005", "STORY-005-BE"
        )

        allowed, reason = check_intercept(
            "Write",
            file_path=str(tmp_path / "notes.txt"),
            project_dir=tmp_path, forced_engaged=True,
            session_key="session-005",
        )

        assert not allowed
        assert "phase=completed" in reason
        assert "Multiple ae-sdd work-item states" not in reason

    def _write_multi_story_work_item(
        self, tmp_path: Path, name: str, active_story_id: str, story_phases: dict
    ) -> Path:
        """写一个内含多个 Story 子状态的单个 work-item（activeStory 指向其中一个）。"""
        state_path = tmp_path / ".auto-engineering" / name / "state.json"
        state_path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "version": "2",
            "projectKey": "test",
            "stateModel": "nested",
            "entryNode": "STORY",
            "stateMachineId": name,
            "workItemKey": name,
            "currentWorkItem": name,
            "scale": "小",
            "activeStory": active_story_id,
            "storyStates": {
                story_id: {
                    "phase": phase,
                    "completedSteps": [],
                    "codingRound": 0,
                    "lastUpdated": "2026-07-09T00:00:00Z",
                    "resetHistory": [],
                }
                for story_id, phase in story_phases.items()
            },
            "history": [],
        }
        state_path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
        return state_path

    def test_active_story_pointing_at_completed_sibling_not_ignored(self, tmp_path):
        """activeStory 指向的 Story 已 completed，但同一 work-item 内还有别的 Story
        未完成时，隐式解析不应把整条 work-item 误判为"已完成"而从候选池中排除
        （v3.9.18 修复：修复前 get_active_phase() 只看 activeStory 指向的子状态，
        会把仍有未完成 Story 的 work-item 误判为整体已完结）。

        注意：这里只断言 resolve_default_state() 本身不再因误判把它排出候选池、
        不再退化成 NoWorkItemStateError；activeStory 指向的子状态本身是否允许
        Write 是另一层 phase 权限判断（取决于用户是否已切换焦点到未完成的
        Story），不是本用例要覆盖的范围。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_multi_story_work_item(
            tmp_path,
            "Story-004",
            active_story_id="STORY-004-A",
            story_phases={"STORY-004-A": "completed", "STORY-004-B": "coding"},
        )

        resolved = work_item_context.resolve_default_state(ade_sdd)

        assert resolved.source == "single-work-item"
        assert resolved.key == "Story-004"

    def test_only_completed_candidate_ignored_by_implicit_resolution(self, tmp_path):
        """唯一候选已 completed 时，隐式解析应视为"无活跃态"而非把它当默认态选中。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")

        allowed, reason = check_intercept(
            "Write", file_path=str(tmp_path / "notes.txt"), project_dir=tmp_path, forced_engaged=True
        )

        assert not allowed
        assert "No ae-sdd work-item state exists" in reason
        assert "phase=completed" not in reason

    def test_state_new_escapes_all_completed_deadlock(self, tmp_path):
        """唯一候选已 completed 时，NoWorkItemStateError 建议的自救命令本身必须可执行，
        否则会构成"建议的脱困命令自己也被拦"的死锁（v3.9.17 修复）。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state new --id STORY-006 --entry-node STORY",
            project_dir=tmp_path, forced_engaged=True,
        )

        assert allowed, reason

    def test_enter_escapes_all_completed_deadlock(self, tmp_path):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd enter test --story STORY-006",
            project_dir=tmp_path, forced_engaged=True,
        )

        assert allowed, reason

    def test_state_new_chained_command_not_escaped(self, tmp_path):
        """链式命令不得走 state new / enter 逃生通道，防止拼接危险后半段绕过。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")

        allowed, reason = check_intercept(
            "Bash",
            bash_command="ae-sdd state new --id STORY-006 --entry-node STORY && rm -rf .ae-sdd/",
            project_dir=tmp_path, forced_engaged=True,
        )

        assert not allowed

    @pytest.mark.parametrize(
        "smuggled_command",
        [
            "ae-sdd state new --id STORY-006 --entry-node STORY & echo pwned",
            "ae-sdd enter test --story STORY-006 & rm -rf .ae-sdd/",
            "ae-sdd state new --id $(touch pwned) --entry-node STORY",
            "ae-sdd state new --id `touch pwned` --entry-node STORY",
            "ae-sdd state new --id STORY-006 --entry-node STORY > /tmp/pwned",
        ],
    )
    def test_state_new_or_enter_smuggled_payload_not_escaped(self, tmp_path, smuggled_command):
        """单个 & / $() / 反引号 / 重定向夹带的命令不得走该逃生通道放行，
        否则会越过 completed/paused 等 phase 的全部权限限制（v3.9.18 修复）。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        self._write_work_item(tmp_path, "Story-005", "STORY-005-BE", "completed")

        allowed, reason = check_intercept(
            "Bash",
            bash_command=smuggled_command,
            project_dir=tmp_path, forced_engaged=True,
        )

        assert not allowed, f"smuggled payload should not escape: {smuggled_command!r}"


class TestQuotedAeSddCommandPrefixes:
    """合法 ae-sdd 命令前缀应兼容 Windows 引号，不放松 shell 控制符防护。"""

    @staticmethod
    def _make_project(tmp_path, phase="completed"):
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        _write_work_item_state(tmp_path, {
            "version": "1",
            "projectKey": "test",
            "phase": phase,
            "scale": "微",
            "entryNode": "BUG",
            "currentStory": "BUG-001",
            "history": [],
        }, key="Bug-BUG-001")
        return tmp_path

    @pytest.mark.parametrize(
        "prefix",
        [
            "ae-sdd",
            "python C:/Users/EDY/.claude/skills/ae-sdd/tools/bin/ae-sdd",
            'python "C:/Users/EDY/.claude/skills/ae-sdd/tools/bin/ae-sdd"',
            'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd"',
            (
                '"C:/Users/EDY/AppData/Local/Programs/Python/Python315/python.exe" '
                '"C:/Users/EDY/.claude/skills/ae-sdd/tools/bin/ae-sdd"'
            ),
        ],
    )
    def test_state_new_escape_accepts_supported_prefixes(self, tmp_path, prefix):
        project_dir = self._make_project(tmp_path)

        allowed, reason = check_intercept(
            "Bash",
            bash_command=f"{prefix} state new --id BUG-002 --entry-node BUG",
            project_dir=project_dir,
            forced_engaged=True,
        )

        assert allowed, reason

    def test_quoted_python_path_memory_command_uses_existing_fast_path(self, tmp_path):
        project_dir = self._make_project(tmp_path, phase="task-generated")

        allowed, reason = check_intercept(
            "Bash",
            bash_command=(
                'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd" memory enter '
                '--phase coding-plan --story BUG-001'
            ),
            project_dir=project_dir,
            forced_engaged=True,
        )

        assert allowed, reason

    def test_quoted_python_path_assets_generate_uses_existing_fast_path(self, tmp_path):
        project_dir = self._make_project(tmp_path, phase="initialized")

        allowed, reason = check_intercept(
            "Bash",
            bash_command=(
                'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd" assets generate '
                '--project test'
            ),
            project_dir=project_dir,
            forced_engaged=True,
        )

        assert allowed, reason

    @pytest.mark.parametrize("subcommand", ["gates check", "state read"])
    def test_quoted_python_path_readonly_commands_remain_readonly(self, tmp_path, subcommand):
        project_dir = self._make_project(tmp_path)

        allowed, reason = check_intercept(
            "Bash",
            bash_command=(
                'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd" '
                f"{subcommand} --work-item BUG-001"
            ),
            project_dir=project_dir,
            forced_engaged=True,
        )

        assert allowed, reason

    @pytest.mark.parametrize(
        "suffix",
        [
            "&& rm -rf .ae-sdd/",
            "; rm -rf .ae-sdd/",
            "$(touch pwned)",
            "`touch pwned`",
            "> out.txt",
        ],
    )
    def test_quoted_state_new_shell_controls_remain_blocked(self, tmp_path, suffix):
        project_dir = self._make_project(tmp_path)
        command = (
            'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd" state new '
            f"--id BUG-002 --entry-node BUG {suffix}"
        )

        allowed, _ = check_intercept(
            "Bash",
            bash_command=command,
            project_dir=project_dir,
            forced_engaged=True,
        )

        assert not allowed

    def test_malformed_quoted_prefix_fails_closed(self, tmp_path):
        project_dir = self._make_project(tmp_path)

        allowed, _ = check_intercept(
            "Bash",
            bash_command=(
                'python "C:/Program Files/ae-sdd/tools/bin/ae-sdd state new '
                '--id BUG-002 --entry-node BUG'
            ),
            project_dir=project_dir,
            forced_engaged=True,
        )

        assert not allowed


class TestHS10DocumentStoragePathGuard:
    """HS-10：流程产物必须落在 document-storage 推导的文档工作区内。"""

    def _make_project(self, tmp_path, phase="story-generated"):
        ae_sdd = tmp_path / ".ae-sdd"
        assets = ae_sdd / "assets"
        assets.mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        _write_work_item_state(tmp_path, {
            "version": "1",
            "projectKey": "test",
            "phase": phase,
            "scale": "大",
            "currentStory": "STORY-001",
            "currentTask": None,
            "history": [],
        })
        (assets / "test.assets.md").write_text(
            f"| gitPath | `{tmp_path}` |\n| docWorkspacePath | `{tmp_path}` |\n",
            encoding="utf-8",
        )
        return tmp_path

    def test_product_doc_outside_doc_workspace_blocked(self, tmp_path):
        project_dir = self._make_project(tmp_path, phase="story-generated")
        detached = tmp_path.parent / "detached" / "STORY-001-Story.md"
        allowed, reason = check_intercept(
            "Write",
            file_path=str(detached),
            project_dir=project_dir, forced_engaged=True,
        )
        assert not allowed
        assert "HS-10" in reason
        assert "document_storage.resolve_path" in reason


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
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True, forced_phase="code-reviewed"
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
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True, forced_phase="code-reviewed"
        )
        # 4 层全过 → HS-7 放行（可能因 phase=code-reviewed 不允许 Bash 被拦，但不是 HS-7）
        if not allowed:
            assert "HS-7" not in reason, "4 层全过不应被 HS-7 拦截"

    def test_prd_complete_blocked_when_no_prd_state(self, tmp_path):
        """PRD state.json 不存在 → 拦截"""
        cmd = "ae-sdd state prd-complete --prd PRD-NONEXIST --runtime mavis"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=tmp_path, forced_engaged=True, forced_phase="code-reviewed"
        )
        assert not allowed
        assert "HS-7" in reason
        assert "不存在" in reason

    def test_prd_check_complete_not_intercepted(self, tmp_path):
        """prd-check-complete（只读校验）不被 HS-7 拦截"""
        project_dir = self._make_prd_state(tmp_path, all_pass=False)
        cmd = "ae-sdd state prd-check-complete --prd PRD-CS-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True, forced_phase="code-reviewed"
        )
        # prd-check-complete 命中只读白名单或 phase 校验，但不应因 HS-7 拦截
        if not allowed:
            assert "HS-7" not in reason, "prd-check-complete 不应被 HS-7 拦截"

    def test_echo_prd_complete_not_intercepted(self, tmp_path):
        """echo/注释形式的 prd-complete 不触发 HS-7（防误判）"""
        project_dir = self._make_prd_state(tmp_path, all_pass=False)
        cmd = "echo 'ae-sdd state prd-complete --prd PRD-CS-001'"
        allowed, _ = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True, forced_phase="code-reviewed"
        )
        # echo 命令不应触发 HS-7（_is_ae_sdd_cmd 排除非执行形式）
        # 即使被其他规则拦，reason 也不应含 HS-7


# ─── 🆕 v3.5.15 多入口状态机：scale 路由跨步跳跃测试 ─────────────────────────

class TestScaleRoutedStateWrite:
    """🆕 v3.5.15：state write 跨步跳跃按 scale 子链判定。

    微链 initialized→coding 是合法单步（不再被误拦）；
    大/中/小链 initialized→coding 仍被拦（跨步跳跃）。
    """

    def _make_state(self, tmp_path, scale, phase="initialized"):
        """构造带 scale 的 .ae-sdd/state.json + config.yaml。"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        _write_work_item_state(tmp_path, {
            "version": "1", "projectKey": "test",
            "phase": phase, "scale": scale,
            "currentStory": "STORY-001", "currentTask": None,
            "history": [],
        })
        return tmp_path

    def _bypass_gates_and_memory(self, monkeypatch):
        """聚焦跨步跳跃逻辑：mock 掉 G-00 资产检查 + memory gate，避免夹具噪声。"""
        # memory gate 放行
        monkeypatch.setattr(
            "lib.memory_gate.check_state_transition",
            lambda **kw: {"pass": True, "blocked": False, "skipped": True, "reason": "mocked"}
        )
        # G-00 等 PHASE_ENTRY_GATES 放行（返回空结果列表 = 无失败）
        monkeypatch.setattr("lib.gates.check_all", lambda *a, **kw: [])

    def test_micro_initialized_to_task_generated_allowed(self, tmp_path, monkeypatch):
        """微链 initialized→task-generated 合法单步，不拦（Task系列入口）"""
        self._bypass_gates_and_memory(monkeypatch)
        project_dir = self._make_state(tmp_path, scale="微", phase="initialized")
        cmd = "ae-sdd state write --phase task-generated --story STORY-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert allowed, f"微链 initialized→task-generated 应放行，但被拒: {reason}"

    def test_micro_initialized_to_coding_blocked_v3516(self, tmp_path, monkeypatch):
        """🆕 v3.5.16 微链 initialized→coding 现在跨步（中间有 coding-process），应拦"""
        self._bypass_gates_and_memory(monkeypatch)
        project_dir = self._make_state(tmp_path, scale="微", phase="initialized")
        cmd = "ae-sdd state write --phase coding --story STORY-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert not allowed, "微链 initialized→coding 应被拦（v3.5.16 中间有 coding-process）"
        assert "跨步跳跃" in reason

    def test_large_initialized_to_coding_blocked(self, tmp_path, monkeypatch):
        """大链 initialized→coding 跨步跳跃（跳了 7 步），应拦"""
        self._bypass_gates_and_memory(monkeypatch)
        project_dir = self._make_state(tmp_path, scale="大", phase="initialized")
        cmd = "ae-sdd state write --phase coding --story STORY-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert not allowed, "大链 initialized→coding 应被拦（跨步跳跃）"
        assert "跨步跳跃" in reason
        assert "大" in reason  # 错误信息含 scale

    def test_small_initialized_to_coding_blocked(self, tmp_path, monkeypatch):
        """小链 initialized→coding 跨步跳跃（跳了 4 步），应拦"""
        self._bypass_gates_and_memory(monkeypatch)
        project_dir = self._make_state(tmp_path, scale="小", phase="initialized")
        cmd = "ae-sdd state write --phase coding --story STORY-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert not allowed, "小链 initialized→coding 应被拦（跨步跳跃）"
        assert "跨步跳跃" in reason

    def test_small_testcase_reviewed_to_task_allowed(self, tmp_path, monkeypatch):
        """小链 testcase-reviewed→task-generated 合法单步（🆕 v3.7.0 修正：小链无 ra-generated 节点，
        旧断言 ra-generated→task-generated 属于过期用例，改测实际存在的合法单步）"""
        self._bypass_gates_and_memory(monkeypatch)
        project_dir = self._make_state(tmp_path, scale="小", phase="testcase-reviewed")
        cmd = "ae-sdd state write --phase task-generated --story STORY-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert allowed, f"小链 testcase-reviewed→task 应放行，但被拒: {reason}"

    def test_story_reviewed_to_testcase_generated_allowed(self, tmp_path, monkeypatch):
        """🆕 v3.7.0 大/中/小链 story-reviewed→testcase-generated 合法单步（TestCase 独立系列入口）"""
        self._bypass_gates_and_memory(monkeypatch)
        for scale in ("大", "中", "小"):
            project_dir = self._make_state(tmp_path, scale=scale, phase="story-reviewed")
            cmd = "ae-sdd state write --phase testcase-generated --story STORY-001"
            allowed, reason = check_intercept(
                "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
            )
            assert allowed, f"scale={scale} story-reviewed→testcase-generated 应放行，但被拒: {reason}"

    def test_story_generated_to_coding_process_blocked(self, tmp_path, monkeypatch):
        """v3.10.0 大/中链 story-generated→coding-process 应被拦（跳过 TestCase）。"""
        self._bypass_gates_and_memory(monkeypatch)
        for scale in ("大", "中"):
            project_dir = self._make_state(tmp_path, scale=scale, phase="story-generated")
            cmd = "ae-sdd state write --phase coding-process --story STORY-001"
            allowed, reason = check_intercept(
                "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
            )
            assert not allowed, f"scale={scale} story-generated→coding-process 应被拦（跳过 TestCase）"
            assert "跨步跳跃" in reason

    def test_micro_initialized_to_dr_not_blocked_by_jump(self, tmp_path, monkeypatch):
        """微链不含 dr-generated，initialized→dr：跨步判定放行（dr 不在微链，index 抛 ValueError→return True）。
        set_phase 层会拦 phase 不在子链，但 gate_intercept 的跨步判定不拦。"""
        self._bypass_gates_and_memory(monkeypatch)
        project_dir = self._make_state(tmp_path, scale="微", phase="initialized")
        cmd = "ae-sdd state write --phase dr-generated --story STORY-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert allowed, f"微链 initialized→dr 由 set_phase 拦，gate_intercept 跨步判定不应拦: {reason}"


class TestCodingProcessHardGuard:
    """🆕 v3.5.16 C1 硬层产物校验：coding phase 写 src/ 须先走过 CodingProcess。

    防止 AI 凭记忆写代码绕过 CodingProcess。校验 coding-process phase 已 confirm。
    """

    def _make_state_with_session(self, tmp_path, phase, confirmed_phases=None):
        """构造 state.json + session.json（含 userConfirmedPhases）。"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        _write_work_item_state(tmp_path, {
            "version": "1", "projectKey": "test",
            "phase": phase, "scale": "大",
            "currentStory": "STORY-001", "currentTask": None,
            "history": [],
        })
        # session.json（关卡3 confirm 校验依赖）
        auto_eng = tmp_path / ".auto-engineering" / "STORY-001"
        auto_eng.mkdir(parents=True, exist_ok=True)
        (auto_eng / "session.json").write_text(json.dumps({
            "storyId": "STORY-001",
            "userConfirmedPhases": confirmed_phases or [],
        }, ensure_ascii=False), encoding="utf-8")
        return tmp_path

    def test_coding_src_blocked_without_coding_process_confirm(self, tmp_path):
        """🆕 v3.5.16 coding phase 写 src/ 但缺 coding-process confirm → 拦截"""
        # task-reviewed 已 confirm，但 coding-process 未 confirm
        project_dir = self._make_state_with_session(
            tmp_path, phase="coding",
            confirmed_phases=[{"phase": "task-reviewed"}]  # 缺 coding-process
        )
        allowed, reason = check_intercept(
            "Write",
            file_path=str(tmp_path / "src/main/java/Foo.java"),
            project_dir=project_dir, forced_engaged=True,
        )
        assert not allowed, "缺 coding-process confirm 时应拦截写 src/"
        assert "CodingProcess" in reason, f"拦截原因应提及 CodingProcess: {reason}"

    def test_coding_src_allowed_with_coding_process_confirm(self, tmp_path):
        """🆕 v3.5.16 coding phase 写 src/ 且 coding-process 已 confirm → 放行

        🆕 v3.8.2 存端兜底：还需 memory enter 才放行（memory 是关联节点强制工具集）。
        """
        project_dir = self._make_state_with_session(
            tmp_path, phase="coding",
            confirmed_phases=[{"phase": "task-reviewed"}, {"phase": "coding-process"},
                              {"phase": "spec-change"}]  # 齐全
        )
        # 🆕 v3.10.3：coding 属关联 phase，写 src/ 前须 memory 存在（新语义：create_memory 替代 enter）
        from lib import memory_store
        scope = memory_store.locate_scope(
            project=str(tmp_path), entity_type="coding", entity_id="STORY-001")
        memory_store.create_memory(scope, source_contexts={})
        allowed, reason = check_intercept(
            "Write",
            file_path=str(tmp_path / "src/main/java/Foo.java"),
            project_dir=project_dir, forced_engaged=True,
        )
        assert allowed, f"coding-process 已 confirm 且 memory 已 enter 应放行写 src/: {reason}"


# ─── 🆕 v3.9.2 ae-sdd memory 命令放行（修复设计阶段死锁）──────────────────────

class TestMemoryCommandPassage:
    """🆕 v3.9.2：设计阶段跑 ae-sdd memory 命令必须放行。

    v3.8.2 引入 memory gate 后，设计阶段 6 个 phase（ra/design/coding-plan/
    review 域）推进前必须完成 ae-sdd memory enter/write/exit，但这三个是 Bash
    命令，而这些 phase 的 PHASE_PERMIT 不含 Bash → AI 跑 memory 被自己设的
    门禁拦死，形成死锁。step 3c 给 memory 命令开特殊通道（同 step 3 对
    state write 的处理），本类覆盖回归。
    """

    def _make_design_phase_project(self, tmp_path, phase="task-generated"):
        """构造设计阶段 .ae-sdd 项目（task-generated 不含 Bash 权限）。"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        _write_work_item_state(tmp_path, {
            "version": "1", "projectKey": "test",
            "phase": phase, "scale": "微",
            "currentStory": "OPT-LIFE-RC-001", "currentTask": "OPT-LIFE-RC-001",
            "history": [],
        }, key="Story-OPT-LIFE-RC-001")
        return tmp_path

    def test_memory_enter_in_design_phase_allowed(self, tmp_path):
        """核心回归：task-generated 跑 ae-sdd memory enter → 放行（修复前死锁）"""
        project_dir = self._make_design_phase_project(tmp_path, phase="task-generated")
        cmd = "ae-sdd memory enter --phase coding-plan --story OPT-LIFE-RC-001"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert allowed, (
            f"设计阶段 ae-sdd memory enter 应放行（v3.9.2 修复死锁），但被拒: {reason}"
        )

    def test_memory_write_in_coding_phase_allowed(self, tmp_path):
        """不回归：coding phase（本就含 Bash）跑 memory write 也放行"""
        project_dir = self._make_design_phase_project(tmp_path, phase="coding")
        cmd = (
            'ae-sdd memory write --phase coding --story STORY-001 '
            '--summary "done" --evidence "src/Foo.java:1"'
        )
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        assert allowed, f"coding phase memory write 应放行: {reason}"

    def test_chained_memory_cmd_not_fast_pathed(self, tmp_path):
        """链式防护：'ae-sdd memory enter && rm -rf' 不被 step 3c 误放，交回后续检查"""
        project_dir = self._make_design_phase_project(tmp_path, phase="task-generated")
        cmd = "ae-sdd memory enter --phase coding-plan && rm -rf .ae-sdd/"
        allowed, reason = check_intercept(
            "Bash", bash_command=cmd, project_dir=project_dir, forced_engaged=True
        )
        # 含链式分隔符 → step 3c 不快速放行 → 落到 step 6 phase permit
        # task-generated 不含 Bash → 被拦
        assert not allowed, "链式 memory 命令不应被 step 3c 误放行"
        assert "phase=task-generated" in reason


# ─── 🆕 v3.9.7 fix-life-deadlock：life 项目设计阶段死环修复 ────────────────────

class TestMemoryDirLazyInit:
    """🆕 v3.9.7：_check_memory_entered 入口惰性 mkdir memory_root。

    life 项目实测：全新 .ae-sdd 项目从未跑过 ae-sdd memory enter 时，
    .ae-sdd/memory/ 目录不存在。第一次写操作触达 _check_memory_entered 时，
    原实现依赖 locate_scope + is_scope_active 链，_read_json 静默返回 {}，
    结果 is_scope_active 始终 False，等价于"判未 enter"——但实际是目录缺失。
    修复：进函数立即 best-effort mkdir，确保后续 locate_scope 路径可达。
    本类覆盖 3 个场景：空目录主动修复 / 已有目录幂等 / 权限拒绝时不阻断。
    """

    def _make_project(self, tmp_path, *, nested=False):
        """构造最小 .ae-sdd 项目结构（设计阶段 + 嵌套/平铺 state 任选）。"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text(
            f"projectKey: {tmp_path.name}\nversion: 1\n", encoding="utf-8"
        )
        if nested:
            # v3.9.0 嵌套 state：顶层无 phase，靠 get_active_phase/story 回退
            state = {
                "version": "2", "projectKey": tmp_path.name,
                "stateModel": "nested", "entryNode": "STORY",
                "activeStory": "OPT-LIFE-RC-001",
                "storyStates": {"OPT-LIFE-RC-001": {
                    "phase": "story-generated", "completedSteps": [],
                    "codingRound": 0, "resetHistory": [],
                }},
                "history": [], "events": [],
            }
        else:
            state = {
                "version": "1", "projectKey": tmp_path.name,
                "phase": "story-generated", "scale": "微",
                "currentStory": "OPT-LIFE-RC-001",
                "history": [],
            }
        _write_work_item_state(tmp_path, state, key="Story-OPT-LIFE-RC-001")
        return tmp_path

    def test_memory_dir_auto_created_on_first_check(self, tmp_path):
        """核心场景：项目无 .ae-sdd/memory/ 目录时，第一次进入
        _check_memory_entered 后 memory 根目录应被自动创建（best-effort），
        但活跃态判定仍按 stage token 是否有 → 无 token 仍应拒绝。
        这正是 life 项目实测死环的修复：fix 防止"目录缺失 = 永假"，
        不削弱"未 enter = 拒绝"门禁语义。
        """
        project_dir = self._make_project(tmp_path, nested=True)
        ae_sdd = project_dir / ".ae-sdd"
        assert not (ae_sdd / "memory").exists(), "前置：memory 目录不存在"

        from lib.gate_intercept import _check_memory_entered
        from lib import state as state_mod
        st = state_mod.read_state(project_dir / ".auto-engineering" / "Story-OPT-LIFE-RC-001" / "state.json")
        allowed, reason = _check_memory_entered("story-generated", ae_sdd, st)

        # 关键断言 1：memory 目录被惰性创建（fix-life-deadlock 主目标）
        assert (ae_sdd / "memory").exists(), (
            "v3.9.7 修复：_check_memory_entered 首次触达应惰性 mkdir memory_root"
        )
        # 关键断言 2：但活跃态判定不变（fix 不削弱门禁）→ 无 stage token 仍拒
        assert not allowed, (
            "无 stage token 时仍应拒绝写操作，fix 只补目录不复活化门禁: " + reason
        )
        assert "memory create" in reason or "memory" in reason  # 🆕 v3.10.3: 消息改为 memory create

    def test_memory_dir_existing_no_op(self, tmp_path):
        """幂等：memory 目录已存在时，进 _check_memory_entered 不应报错或重建。"""
        project_dir = self._make_project(tmp_path)
        memory_dir = project_dir / ".ae-sdd" / "memory"
        memory_dir.mkdir(parents=True, exist_ok=True)
        (memory_dir / "sentinel.txt").write_text("do-not-touch", encoding="utf-8")

        # 直接用 _check_memory_entered 验证幂等
        from lib.gate_intercept import _check_memory_entered
        from lib import state as state_mod
        st = state_mod.read_state(project_dir / ".auto-engineering" / "Story-OPT-LIFE-RC-001" / "state.json")
        allowed, _ = _check_memory_entered("story-generated", project_dir / ".ae-sdd", st)

        # 即使是占位 token 也应放行（写文件后才触发）
        # story-generated → memory_phase=design，未 enter → 应当放行或拒绝
        # 主要验证：mkdir 不破坏 sentinel 文件
        assert (memory_dir / "sentinel.txt").read_text(encoding="utf-8") == "do-not-touch"

    def test_memory_dir_write_respects_active_stage(self, tmp_path):
        """验证 fix-life-deadlock 不改变 is_scope_active 的真实活跃态语义：
        即便 mkdir 成功，若 scope 没真正 enter，仍不放行写。
        """
        project_dir = self._make_project(tmp_path)
        # 不创建任何 stage token，写操作应被拒
        from lib.gate_intercept import _check_memory_entered
        from lib import state as state_mod
        st = state_mod.read_state(project_dir / ".auto-engineering" / "Story-OPT-LIFE-RC-001" / "state.json")
        allowed, reason = _check_memory_entered(
            "story-generated", project_dir / ".ae-sdd", st
        )
        assert not allowed, (
            "无 stage token 时仍应拦截写操作（防 fix 削弱门禁语义）: " + reason
        )
        assert "memory create" in reason or "memory" in reason  # 🆕 v3.10.3: 消息改为 memory create


# ─── 🆕 v3.9.21 engage 按需启用门禁 ────────────────────────────────────────────

class TestSessionEngageGate:
    """门禁按会话 engage 按需启用：未 engage 放行，engaged 后按 phase 锁。

    核心语义：用户调 /ae-sdd 前 hook 完全不工作；调了之后定位 state 并启用校验。
    engage 标记按 session_key 维度，子 Agent 继承父会话 session_id 时随主会话 engage。
    """

    def _make_engaged_project(self, tmp_path: Path, phase: str = "task-reviewed") -> Path:
        """建一个有 active work-item 的项目（用于 engage 测试）。"""
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        (ade_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        _write_work_item_state(tmp_path, {
            "version": "1", "projectKey": "test", "phase": phase,
            "scale": "小", "currentStory": "STORY-001",
            "currentTask": None, "history": [],
        })
        return ade_sdd

    def test_unengaged_session_passthrough_even_with_active_state(self, tmp_path):
        """未 engage 的会话，即使项目有 active work-item 也放行（修复核心目标）。"""
        self._make_engaged_project(tmp_path, phase="task-reviewed")
        # task-reviewed 正常会锁 Bash，但未 engage → 放行
        allowed, reason = check_intercept(
            "Bash", bash_command="rm -rf something",
            project_dir=tmp_path, session_key="never-engaged-xyz",
        )
        assert allowed, f"未 engage 会话应放行，但被拒: {reason}"
        assert reason == ""

    def test_unengaged_write_passthrough(self, tmp_path):
        """未 engage 的会话写文件也放行。"""
        self._make_engaged_project(tmp_path, phase="initialized")
        allowed, reason = check_intercept(
            "Write", file_path="src/main/java/Foo.java",
            project_dir=tmp_path, session_key="never-engaged-write",
        )
        assert allowed, f"未 engage 写应放行: {reason}"

    def test_engaged_session_enforces_phase_lock(self, tmp_path):
        """已 engage 的会话按 phase 正常锁（engaged 后门禁生效）。"""
        ade_sdd = self._make_engaged_project(tmp_path, phase="initialized")
        # initialized 的 PHASE_PERMIT 不含 Bash → engaged 后应锁
        work_item_context.mark_session_engaged(ade_sdd, "engaged-session-1")
        allowed, reason = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="engaged-session-1",
        )
        assert not allowed, "engaged 会话在 initialized phase 应锁 Bash"
        assert "phase=initialized" in reason

    def test_engaged_session_allows_permitted_tool(self, tmp_path):
        """已 engage 的会话，phase 允许的工具正常放行。"""
        ade_sdd = self._make_engaged_project(tmp_path, phase="coding")
        work_item_context.mark_session_engaged(ade_sdd, "engaged-session-2")
        # coding 的 PHASE_PERMIT 含 Bash → 放行（非只读命令也放行）
        allowed, reason = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="engaged-session-2",
        )
        assert allowed, f"engaged + coding phase 应放行 Bash: {reason}"

    def test_disengage_restores_passthrough(self, tmp_path):
        """调 /ae-sdd 后说'退出 ae-sdd' → 清除标记 → 恢复放行。"""
        ade_sdd = self._make_engaged_project(tmp_path, phase="initialized")
        work_item_context.mark_session_engaged(ade_sdd, "engaged-then-exit")
        # engage 后确实锁（initialized 不允许 Bash）
        allowed_before, _ = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="engaged-then-exit",
        )
        assert not allowed_before
        # disengage 后恢复放行
        work_item_context.disengage_session(ade_sdd, "engaged-then-exit")
        allowed_after, reason = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="engaged-then-exit",
        )
        assert allowed_after, f"disengage 后应放行: {reason}"
        assert reason == ""

    def test_empty_session_key_treated_as_unengaged(self, tmp_path):
        """无 session_key 的会话视为未 engage（放行）。"""
        self._make_engaged_project(tmp_path, phase="initialized")
        allowed, reason = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="",
        )
        assert allowed, "空 session_key 应视为未 engage 放行"

    def test_legacy_engaged_marker_is_not_an_activation_signal(self, tmp_path):
        ade_sdd = self._make_engaged_project(tmp_path, phase="initialized")
        legacy = ade_sdd / ".session-engaged"
        legacy.mkdir(parents=True)
        (legacy / work_item_context._safe_session_file_name("legacy-session")).write_text(
            '{"engagedAt":"2026-07-01T00:00:00Z"}', encoding="utf-8"
        )

        allowed, reason = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="legacy-session",
        )

        assert allowed, reason

    def test_real_ae_sdd_write_command_starts_turn_activity(self, tmp_path):
        ade_sdd = self._make_engaged_project(tmp_path, phase="initialized")
        session_id = "direct-ae-sdd-entry"

        check_intercept(
            "Bash",
            bash_command="ae-sdd state write --work-item Story-001 --phase initialized",
            project_dir=tmp_path,
            session_key=session_id,
        )

        assert work_item_context.is_session_engaged(ade_sdd, session_id)
        work_item_context.disengage_session(ade_sdd, session_id)

    def test_engage_is_per_session_not_global(self, tmp_path):
        """engage 标记按 session 隔离：A 会话 engage 不影响 B 会话。"""
        ade_sdd = self._make_engaged_project(tmp_path, phase="initialized")
        work_item_context.mark_session_engaged(ade_sdd, "session-A")
        # A 已 engage → 锁（initialized 不允许 Bash）
        allowed_a, _ = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="session-A",
        )
        assert not allowed_a
        # B 未 engage → 放行（同一项目，不同会话）
        allowed_b, reason_b = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="session-B",
        )
        assert allowed_b, f"不同会话不应被 A 的 engage 影响: {reason_b}"

    def test_forced_engaged_true_bypasses_marker_check(self, tmp_path):
        """forced_engaged=True 跳过标记检查（测试用入口）。"""
        self._make_engaged_project(tmp_path, phase="initialized")
        # 不写 engage 标记，但 forced_engaged=True → 走门禁（initialized 不允许 Bash）
        allowed, reason = check_intercept(
            "Bash", bash_command="mvn compile",
            project_dir=tmp_path, session_key="any",
            forced_engaged=True,
        )
        assert not allowed, "forced_engaged=True 应走门禁"
        assert "phase=initialized" in reason

