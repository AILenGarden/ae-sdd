"""
test_fixes_v14.py — v1.4 修复验证测试

覆盖：
  1. _ALWAYS_ALLOW_PATTERNS 移除 .json/.yaml/.yml，
     src/main/resources/*.yaml 在设计阶段应被拦截
  2. 链式 Bash 命令（&&、||、;、|）不再被只读白名单放行
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib.gate_intercept import (
    _check_path_permission,
    _is_always_allowed_path,
    _is_readonly_bash,
    check_intercept,
)


# ─── 1. _ALWAYS_ALLOW_PATTERNS 修复 ─────────────────────────────────────────

class TestAlwaysAllowPatternsFix:
    """v1.4 修复：.json/.yaml/.yml 从 _ALWAYS_ALLOW_PATTERNS 中移除"""

    def test_json_in_src_resources_not_always_allowed(self):
        """src/main/resources/application.json 不再被 always_allow 放行"""
        assert not _is_always_allowed_path("src/main/resources/application.json")

    def test_yaml_in_src_resources_not_always_allowed(self):
        """src/main/resources/application.yaml 不再被 always_allow 放行"""
        assert not _is_always_allowed_path("src/main/resources/application.yaml")

    def test_yml_in_src_resources_not_always_allowed(self):
        """src/main/resources/config.yml 不再被 always_allow 放行"""
        assert not _is_always_allowed_path("src/main/resources/config.yml")

    def test_ae_sdd_json_still_allowed(self):
        """.ae-sdd/state.json 被 .ae-sdd/ 前缀放行（正确）"""
        assert _is_always_allowed_path(".ae-sdd/state.json")

    def test_design_yaml_still_allowed(self):
        """design/api.yaml 被 design/ 前缀放行（正确）"""
        assert _is_always_allowed_path("design/api.yaml")

    def test_readme_still_allowed(self):
        """README.md 仍被放行"""
        assert _is_always_allowed_path("README.md")

    @pytest.mark.parametrize("phase", [
        "initialized", "dr-generated", "story-generated",
        "story-reviewed", "task-generated",
    ])
    def test_resources_yaml_blocked_in_design_phases(self, phase):
        """src/main/resources/*.yaml 在设计阶段被拦截"""
        allowed, reason = _check_path_permission(
            "Write", "src/main/resources/application.yaml", phase
        )
        assert not allowed, f"phase={phase} 应该拦截 resources/application.yaml"
        assert "设计阶段" in reason

    @pytest.mark.parametrize("phase", [
        "initialized", "dr-generated", "story-generated",
        "story-reviewed", "task-generated",
    ])
    def test_design_yaml_still_allowed_in_design_phases(self, phase):
        """design/api.yaml 在设计阶段仍被放行"""
        allowed, _ = _check_path_permission("Write", "design/api.yaml", phase)
        assert allowed, f"phase={phase} 的 design/api.yaml 应该放行"

    def test_resources_yaml_allowed_in_coding_phase(self):
        """coding phase 允许写 src/main/resources/*.yaml（不在 _DESIGN_PHASES 中）"""
        allowed, _ = _check_path_permission(
            "Write", "src/main/resources/application.yaml", "coding"
        )
        assert allowed


# ─── 2. 链式 Bash 命令修复 ────────────────────────────────────────────────────

class TestChainedBashFix:
    """v1.4 修复：链式 Bash 命令不再被只读白名单放行"""

    @pytest.mark.parametrize("cmd", [
        "git status && rm -rf design/",
        "cat config.yaml; rm evil.py",
        "ls -la | tee /tmp/out; python malicious.py",
        "git log && python inject.py",
        "cat pom.xml || python bad.py",
    ])
    def test_chained_commands_not_readonly(self, cmd):
        """含 &&/;/|/|| 的链式命令不被判为只读"""
        assert not _is_readonly_bash(cmd), f"链式命令不应被判为只读: {cmd!r}"

    @pytest.mark.parametrize("cmd", [
        "git status",
        "ae-sdd gates check",
        "ae-sdd gates check --json",
        "git log --oneline -5",
        "mvn --version",
        "cat pom.xml",
        "ls -la",
    ])
    def test_simple_readonly_commands_still_allowed(self, cmd):
        """无链式分隔符的只读命令仍被放行"""
        assert _is_readonly_bash(cmd), f"只读命令应被放行: {cmd!r}"

    def test_chained_blocked_in_initialized_phase(self, tmp_path):
        """initialized phase 下，链式命令（含只读前缀）被拦截"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        allowed, reason = check_intercept(
            "Bash",
            bash_command="git status && rm -rf design/",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed, "链式 Bash 在 initialized phase 应被拦截"

    def test_chained_allowed_in_coding_phase(self, tmp_path):
        """coding phase 允许所有 Bash（含链式），因为 phase permit 包含 Bash"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # 🆕 v3.9.13：state 源改为 task-scoped .auto-engineering/<work-item>/state.json
        wi_dir = tmp_path / ".auto-engineering" / "Story-001"
        wi_dir.mkdir(parents=True, exist_ok=True)
        (wi_dir / "state.json").write_text(json.dumps({
            "stateModel": "nested",
            "activeStory": "STORY-001",
            "storyStates": {"STORY-001": {"phase": "coding"}},
        }, ensure_ascii=False, indent=2), encoding="utf-8")
        allowed, _ = check_intercept(
            "Bash",
            bash_command="mvn test && git add .",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert allowed, "coding phase 应允许链式 Bash（phase permit 包含 Bash）"

    def test_pipe_blocked_in_design_phase(self, tmp_path):
        """设计阶段 pipe 命令被拦截"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "story-generated", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        allowed, _ = check_intercept(
            "Bash",
            bash_command="cat pom.xml | python process.py",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed, "设计阶段 pipe 命令应被拦截"


# ─── 3. code-reviewed 阶段源码写入保护 ──────────────────────────────────────

class TestCodeReviewedSourceProtection:
    """v1.4 修复：code-reviewed 加入 _DESIGN_PHASES，禁止在 CR 阶段改源码"""

    def test_code_reviewed_in_design_phases(self):
        """code-reviewed 在 _DESIGN_PHASES 中"""
        from lib.gate_intercept import _DESIGN_PHASES
        assert "code-reviewed" in _DESIGN_PHASES

    def test_java_blocked_in_code_reviewed(self, tmp_path):
        """code-reviewed 阶段写 Java 文件被拦截"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # 🆕 v3.9.13：state 源改为 task-scoped .auto-engineering/<work-item>/state.json
        wi_dir = tmp_path / ".auto-engineering" / "Story-001"
        wi_dir.mkdir(parents=True, exist_ok=True)
        (wi_dir / "state.json").write_text(json.dumps({
            "stateModel": "nested",
            "activeStory": "STORY-001",
            "storyStates": {"STORY-001": {"phase": "code-reviewed"}},
        }, ensure_ascii=False, indent=2), encoding="utf-8")
        allowed, reason = check_intercept(
            "Write",
            file_path="src/main/java/Foo.java",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed, "code-reviewed 阶段不应允许写 Java 源码"
        assert "设计阶段" in reason

    def test_resources_yaml_blocked_in_code_reviewed(self, tmp_path):
        """code-reviewed 阶段写 src/main/resources/*.yaml 被拦截"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "code-reviewed", "currentStory": "STORY-001",
            "currentTask": None, "history": [],
        }))
        allowed, _ = check_intercept(
            "Write",
            file_path="src/main/resources/application.yaml",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed, "code-reviewed 阶段不应允许写 resources/application.yaml"

    def test_cr_report_allowed_in_code_reviewed(self, tmp_path):
        """code-reviewed 阶段写 CR 报告（design/ 下）被允许

        🆕 v3.8.2 存端兜底：code-reviewed 属关联 phase（review），须 memory enter 才放行。
        """
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # 🆕 v3.9.13：state 源改为 task-scoped .auto-engineering/<work-item>/state.json
        wi_dir = tmp_path / ".auto-engineering" / "Story-001"
        wi_dir.mkdir(parents=True, exist_ok=True)
        (wi_dir / "state.json").write_text(json.dumps({
            "stateModel": "nested",
            "activeStory": "STORY-001",
            "storyStates": {"STORY-001": {"phase": "code-reviewed"}},
        }, ensure_ascii=False, indent=2), encoding="utf-8")
        # 🆕 v3.10.3：code-reviewed 属关联 phase，写文件前须 memory 存在（create_memory 替代 enter）
        from lib import memory_store
        scope = memory_store.locate_scope(project=str(tmp_path), entity_type="coding", entity_id="STORY-001")
        memory_store.create_memory(scope, source_contexts={})
        allowed, _ = check_intercept(
            "Write",
            file_path="design/cr-report.md",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert allowed, "code-reviewed 阶段应允许写 CR 报告文档"


# ─── 4. init-hooks --force 保留用户自定义 hook ────────────────────────────────

class TestInitHooksPreserveCustomHooks:
    """v1.4 修复：--force 只删除 ae-sdd 的 hook，不清空整个 section"""

    def test_force_preserves_custom_userpromptsubmit_hook(self, tmp_path):
        """--force 重写 prompt-inject 时，保留用户自定义的 UserPromptSubmit hook"""
        import subprocess

        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        settings = {
            "hooks": {
                "UserPromptSubmit": [
                    {"hooks": [{"type": "command", "command": "my-custom-hook --arg"}]},
                    {"hooks": [{"type": "command", "command": "ae-sdd prompt-inject"}]},
                ]
            }
        }
        (claude_dir / "settings.json").write_text(json.dumps(settings))

        result = subprocess.run(
            ["python", "tools/bin/ae-sdd", "init-hooks", str(tmp_path), "--force", "--dry-run"],
            capture_output=True, text=True,
            cwd=str(Path(__file__).parent.parent.parent),
        )
        parsed = json.loads(result.stdout)
        us_hooks = parsed.get("hooks", {}).get("UserPromptSubmit", [])
        cmds = [h.get("hooks", [{}])[0].get("command", "") for h in us_hooks]
        assert any("my-custom-hook" in c for c in cmds), (
            f"--force 误删了用户自定义 hook，现有: {cmds}"
        )
        assert any("prompt-inject" in c for c in cmds), (
            "ae-sdd prompt-inject 应该保留"
        )

    def test_force_preserves_custom_stop_hook(self, tmp_path):
        """--force 重写 stop-check 时，保留用户自定义的 Stop hook"""
        import subprocess

        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        settings = {
            "hooks": {
                "Stop": [
                    {"hooks": [{"type": "command", "command": "my-stop-hook"}]},
                    {"hooks": [{"type": "command", "command": "ae-sdd stop-check"}]},
                ]
            }
        }
        (claude_dir / "settings.json").write_text(json.dumps(settings))

        result = subprocess.run(
            ["python", "tools/bin/ae-sdd", "init-hooks", str(tmp_path), "--force", "--dry-run"],
            capture_output=True, text=True,
            cwd=str(Path(__file__).parent.parent.parent),
        )
        parsed = json.loads(result.stdout)
        stop_hooks = parsed.get("hooks", {}).get("Stop", [])
        cmds = [h.get("hooks", [{}])[0].get("command", "") for h in stop_hooks]
        assert any("my-stop-hook" in c for c in cmds), (
            f"--force 误删了用户自定义 Stop hook，现有: {cmds}"
        )
        assert any("stop-check" in c for c in cmds), (
            "ae-sdd stop-check 应该保留"
        )



# ─── 5. .quick_channel 文件存在即激活快速通道 ─────────────────────────────────

class TestQuickChannelFileExistence:
    """v1.4 修复：.quick_channel 文件存在本身即激活信号（不依赖文件内容含标记词）"""

    def test_file_exists_without_marker_activates_channel(self, tmp_path):
        """.quick_channel 文件存在但内容无标记词 → 快速通道激活"""
        import subprocess

        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        # 写入不含任何标记词的文件
        (ae_sdd / ".quick_channel").write_text("普通消息内容，无标记词")

        payload = json.dumps({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "mvn compile"},
        })
        result = subprocess.run(
            ["python", "tools/bin/ae-sdd", "gate-intercept", "--project", str(tmp_path)],
            input=payload, text=True, capture_output=True,
            cwd=str(Path(__file__).parent.parent.parent),
        )
        parsed = json.loads(result.stdout)
        decision = parsed.get("hookSpecificOutput", {}).get("permissionDecision", "allow")
        assert decision != "deny", (
            ".quick_channel 文件存在（无标记词）时应激活快速通道"
        )

    def test_file_absent_blocks_bash_in_design_phase(self, tmp_path):
        """.quick_channel 文件不存在 + 设计阶段 → Bash 被拦截"""
        import subprocess

        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        # 确保文件不存在
        qc = ae_sdd / ".quick_channel"
        if qc.exists():
            qc.unlink()

        # 🆕 v3.9.21：engage 按需启用门禁——subprocess 调真实 CLI 须先 engage
        # （否则无 session_id 会在 engage 短路层放行，测不到门禁逻辑）
        from lib import work_item_context as _wic
        _test_session = "v14-quick-channel-subprocess-test"
        _wic.mark_session_engaged(ae_sdd, _test_session)

        payload = json.dumps({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "mvn compile"},
            "session_id": _test_session,
        })
        result = subprocess.run(
            ["python", "tools/bin/ae-sdd", "gate-intercept", "--project", str(tmp_path)],
            input=payload, text=True, capture_output=True,
            cwd=str(Path(__file__).parent.parent.parent),
        )
        parsed = json.loads(result.stdout)
        decision = parsed.get("hookSpecificOutput", {}).get("permissionDecision", "allow")
        assert decision == "deny", (
            ".quick_channel 不存在时，设计阶段 Bash 应被拦截"
        )


# ─── 6. BASH_READONLY_PREFIXES 扩充验证 ─────────────────────────────────────

class TestBashReadonlyPrefixesExpanded:
    """v1.4 新增：node/python3/git --version 等常用版本查询命令在任意 phase 放行"""

    @pytest.mark.parametrize("cmd,expected", [
        ("node --version", True),
        ("node -v", True),
        ("python3 --version", True),
        ("npm --version", True),
        ("npm -v", True),
        ("git --version", True),
        ("gradle --version", True),
        ("pip --version", True),
        ("pip3 --version", True),
        ("mvn -v", True),
        ("java -version", True),
        ("which java", True),
        ("which node", True),
        # 链式版本查询仍被拦截（链式分隔符）
        ("node --version && rm -rf", False),
    ])
    def test_version_commands(self, cmd, expected):
        from lib.gate_intercept import _is_readonly_bash
        assert _is_readonly_bash(cmd) == expected, (
            f"_is_readonly_bash({cmd!r}) = {_is_readonly_bash(cmd)}, expect {expected}"
        )

    @pytest.mark.parametrize("cmd", [
        "node --version", "python3 --version", "git --version",
    ])
    def test_version_commands_allowed_in_design_phase(self, tmp_path, cmd):
        """版本查询命令在设计阶段被放行"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        from lib.gate_intercept import check_intercept
        allowed, _ = check_intercept("Bash", bash_command=cmd, project_dir=tmp_path, forced_engaged=True)
        assert allowed, f"设计阶段 {cmd!r} 应被放行"
