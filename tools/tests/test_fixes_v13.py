"""
test_fixes_v13.py — v1.3 修复验证测试

覆盖：
  1. MultiEdit 被 PreToolUse hook 正确拦截
  2. Stop hook 历史状态头不再误判（只检查最后一段）
  3. 快速通道通过 .ae-sdd/.quick_channel 文件跨 hook 传递
  4. extract_last_assistant_text 多格式支持
  5. install_cli.py 基本逻辑
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib.gate_intercept import (
    PHASE_PERMIT,
    WRITE_TOOLS,
    check_intercept,
    _check_path_permission,
)
from lib.stop_check import (
    check_output,
    extract_last_assistant_text,
    MAX_RETRY,
    increment_retry,
)
from lib.prompt_inject import inject, QUICK_CHANNEL_MARKERS

SCRIPTS_DIR = Path(__file__).resolve().parent.parent.parent / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))
import install_cli


# ─── 1. MultiEdit 拦截 ───────────────────────────────────────────────────────

class TestMultiEditInterception:

    def test_multiedit_in_write_tools(self):
        assert "MultiEdit" in WRITE_TOOLS

    def test_multiedit_in_all_phase_permits(self):
        """每个允许写的 phase 都必须包含 MultiEdit"""
        for phase, tools in PHASE_PERMIT.items():
            if "Write" in tools:
                assert "MultiEdit" in tools, (
                    f"phase={phase} 允许 Write 但不允许 MultiEdit，存在绕过风险"
                )

    def test_multiedit_blocked_in_design_phase_with_source_file(self, tmp_path):
        """initialized phase + MultiEdit 写 Java → 被拦截"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # 🆕 v3.9.13：state 源从项目级 .ae-sdd/state.json 改为 task-scoped
        # .auto-engineering/<work-item>/state.json。resolve_default_state 扫描该目录，
        # 恰好 1 个未 completed 的 work-item 就命中（见 work_item_context.py:349）。
        wi_dir = tmp_path / ".auto-engineering" / "Story-001"
        wi_dir.mkdir(parents=True, exist_ok=True)
        (wi_dir / "state.json").write_text(json.dumps({
            "stateModel": "nested",
            "activeStory": "STORY-001",
            "storyStates": {"STORY-001": {"phase": "initialized"}},
        }, ensure_ascii=False, indent=2), encoding="utf-8")
        allowed, reason = check_intercept(
            "MultiEdit",
            file_path="src/main/java/Service.java",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert not allowed
        assert "设计阶段" in reason

    def test_multiedit_allowed_in_coding_phase(self, tmp_path):
        """coding phase + MultiEdit 写 Java → 允许（🆕 v3.4.0 须先确认 task-reviewed 审核点 token）"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        # 🆕 v3.9.13：state 源改为 task-scoped .auto-engineering/<work-item>/state.json。
        # session_mod.enter(story_id="STORY-001") 会写 .auto-engineering/Story-001/session.json，
        # 故 state 必须放同一目录，让 resolve_default_state + is_phase_confirmed 同步命中。
        wi_dir = tmp_path / ".auto-engineering" / "Story-001"
        wi_dir.mkdir(parents=True, exist_ok=True)
        (wi_dir / "state.json").write_text(json.dumps({
            "stateModel": "nested",
            "activeStory": "STORY-001",
            "storyStates": {"STORY-001": {"phase": "coding"}},
        }, ensure_ascii=False, indent=2), encoding="utf-8")
        # 🆕 v3.4.0 关卡3：coding phase 写 src/ 须有 task-reviewed 审核点确认 token
        from lib import session as session_mod
        session_mod.enter("test", story_id="STORY-001", ade_sdd=ae_sdd)
        session_mod.confirm_phase(ae_sdd, "task-reviewed", story_id="STORY-001")
        # 🆕 v3.5.16 C1 硬层：coding phase 写 src/ 还须 coding-process confirm
        session_mod.confirm_phase(ae_sdd, "coding-process", story_id="STORY-001")
        # v3.10.6 §B0.5: non-micro source writes also require the explicit
        # spec-change decision token.
        session_mod.confirm_phase(ae_sdd, "spec-change", story_id="STORY-001")
        # 🆕 v3.10.3 存端兜底：coding 属关联 phase，写 src/ 须 memory 存在（create_memory 替代 enter）
        from lib import memory_store
        scope = memory_store.locate_scope(project=str(tmp_path), entity_type="coding", entity_id="STORY-001")
        memory_store.create_memory(scope, source_contexts={})
        allowed, _ = check_intercept(
            "MultiEdit",
            file_path="src/main/java/Service.java",
            project_dir=tmp_path, forced_engaged=True,
        )
        assert allowed

    def test_multiedit_path_check_uses_write_tools(self):
        """_check_path_permission 对 MultiEdit 和 Write 行为一致"""
        for phase in ["initialized", "dr-generated", "story-generated"]:
            allowed_write, _ = _check_path_permission(
                "Write", "src/main/java/X.java", phase
            )
            allowed_multi, _ = _check_path_permission(
                "MultiEdit", "src/main/java/X.java", phase
            )
            assert allowed_write == allowed_multi, (
                f"phase={phase}: Write={allowed_write} 但 MultiEdit={allowed_multi}"
            )

    def test_settings_json_matcher_includes_multiedit(self, tmp_path):
        """init-hooks 写入的 settings.json matcher 包含 MultiEdit"""
        result = subprocess.run(
            ["python", "tools/bin/ae-sdd", "init-hooks", str(tmp_path), "--dry-run"],
            capture_output=True, text=True, encoding="utf-8", errors="replace", cwd=str(Path(__file__).parent.parent.parent)
        )
        assert "MultiEdit" in result.stdout, (
            "init-hooks dry-run 输出里没有 MultiEdit，matcher 配置有误"
        )


# ─── 2. Stop hook 历史状态头误判修复 ─────────────────────────────────────────

class TestStopHookTranscriptExtraction:

    def test_extract_last_jsonl_assistant(self):
        """JSONL 格式：正确提取最后一条 assistant"""
        transcript = "\n".join([
            json.dumps({"role": "user", "content": "帮我写代码"}),
            json.dumps({"role": "assistant", "content": "◆ STATE: coding/STORY-001\n完成了"}),
            json.dumps({"role": "user", "content": "继续"}),
            json.dumps({"role": "assistant", "content": "Task-4 写完了，没有状态头"}),
        ])
        last = extract_last_assistant_text(transcript)
        assert "Task-4" in last
        assert "STATE" not in last  # 最后一条没有状态头

    def test_extract_last_plain_text_marker(self):
        """纯文本 [ASSISTANT] 格式：只取最后一段"""
        transcript = """
[ASSISTANT]
◆ STATE: coding/STORY-001
◆ GATE: ✅ CLEAR
完成了 Task-2

[HUMAN]
继续做 Task-3

[ASSISTANT]
Task-3 做完了，代码已提交。
"""
        last = extract_last_assistant_text(transcript)
        assert "Task-3" in last
        assert "STATE" not in last

    def test_extract_fallback_tail(self):
        """无格式标记时取末尾 2000 字符"""
        # 超过 2000 字符的旧内容 + 新内容
        old_content = "◆ STATE: coding/OLD\n" + "x" * 2500
        new_content = "最新响应，没有状态头。"
        transcript = old_content + new_content
        last = extract_last_assistant_text(transcript)
        # 末尾 2000 字符：应该不包含旧的 STATE 头（超出 2000 字符范围）
        assert "最新响应" in last

    def test_history_header_no_longer_fools_stop_hook(self, tmp_path):
        """v3.6（决策 1B）：废弃 ◆ STATE 自报标记检测，最新响应无状态头 → 放行（allow stop）。

        历史背景：v1.3 修复"历史轮次状态头被误判为最新"，v3.6 进一步废弃自报标记检测
        （改由 flow_monitor 产物核查判定流程合规），故无论最新响应有无状态头都放行。
        本测试保留：验证历史轮次有状态头、最新无状态头时不会误判为"被截断"。
        """
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()

        # JSONL 格式 transcript
        transcript = "\n".join([
            json.dumps({"role": "user", "content": "帮我写代码"}),
            json.dumps({"role": "assistant", "content": "◆ STATE: coding/STORY-001\n完成了"}),
            json.dumps({"role": "user", "content": "继续"}),
            json.dumps({"role": "assistant", "content": "Task-4 写完了。"}),  # 无状态头
        ])

        should_stop, msg = check_output(transcript, ade_sdd=ae_sdd)
        assert should_stop, "v3.6 废弃自报标记：最新响应无状态头应放行（非 ae-sdd 截断）"
        assert msg == ""

    def test_latest_header_allows_stop(self, tmp_path):
        """最新响应有状态头 → 允许停止"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()

        transcript = "\n".join([
            json.dumps({"role": "user", "content": "继续"}),
            json.dumps({"role": "assistant", "content":
                "Task-4 写完了。\n◆ STATE: coding/STORY-001\n◆ GATE: ✅ CLEAR\n◆ LAST: Task-4\n◆ NEXT: Task-5"
            }),
        ])

        should_stop, _ = check_output(transcript, ade_sdd=ae_sdd)
        assert should_stop, "最新响应有状态头，应该允许停止"

    def test_empty_transcript_blocked(self, tmp_path):
        """空 transcript → 被 block（没有最新响应）"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        should_stop, _ = check_output("", ade_sdd=ae_sdd)
        assert not should_stop


# ─── 3. 快速通道跨 hook 文件传递 ─────────────────────────────────────────────

class TestQuickChannelFilePersistence:

    def test_inject_writes_quick_channel_file(self, tmp_path):
        """用户消息含快速通道标记 → .quick_channel 文件被创建"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))

        inject(project_dir=tmp_path, user_prompt="/ae-sdd-quick 改个字段名")
        qc_file = ae_sdd / ".quick_channel"
        assert qc_file.is_file(), ".quick_channel 文件未创建"
        assert "ae-sdd-quick" in qc_file.read_text()

    def test_inject_clears_quick_channel_file_on_normal_message(self, tmp_path):
        """普通消息 → .quick_channel 文件被清除"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        # 先创建快速通道文件
        (ae_sdd / ".quick_channel").write_text("ae-sdd-quick")

        inject(project_dir=tmp_path, user_prompt="正常消息，继续做 Story")
        assert not (ae_sdd / ".quick_channel").exists(), "普通消息后 .quick_channel 应被清除"

    def test_gate_intercept_reads_quick_channel_file(self, tmp_path):
        """gate-intercept CLI 从 .quick_channel 文件读快速通道状态"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))
        # 写入快速通道文件（模拟 UserPromptSubmit 已处理）
        (ae_sdd / ".quick_channel").write_text("ae-sdd-quick")

        payload = json.dumps({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "mvn compile"},
        })

        result = subprocess.run(
            ["python", "tools/bin/ae-sdd", "gate-intercept", "--project", str(tmp_path)],
            input=payload, text=True, encoding="utf-8", errors="replace", capture_output=True,
            cwd=str(Path(__file__).parent.parent.parent)
        )
        parsed = json.loads(result.stdout)
        decision = parsed.get("hookSpecificOutput", {}).get("permissionDecision", "allow")
        assert decision != "deny", (
            "快速通道文件已存在，mvn compile 不应被拒绝"
        )

    @pytest.mark.parametrize("marker", list(QUICK_CHANNEL_MARKERS))
    def test_all_markers_create_file(self, tmp_path, marker):
        """所有快速通道标记词都能创建文件"""
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        (ae_sdd / "config.yaml").write_text("projectKey: test\n")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": "initialized", "currentStory": None,
            "currentTask": None, "history": [],
        }))

        inject(project_dir=tmp_path, user_prompt=f"请{marker}处理")
        assert (ae_sdd / ".quick_channel").is_file(), f"标记 '{marker}' 未创建快速通道文件"


# ─── 4. install_cli.py 基础逻辑 ──────────────────────────────────────────────

class TestInstallCli:

    def test_install_cli_check_exits_correctly(self):
        """install_cli.py --check 能正常运行"""
        result = subprocess.run(
            ["python", "scripts/install_cli.py", "--check"],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
            cwd=str(Path(__file__).parent.parent.parent)
        )
        # exit 0 = 已在 PATH，exit 1 = 不在 PATH，都是合法结果
        assert result.returncode in (0, 1), f"意外 exit code: {result.returncode}"
        # 至少应该输出 CLI 路径信息
        output = result.stdout + result.stderr
        assert "ae-sdd" in output.lower()

    def test_repo_root_detection(self):
        """install_cli 能正确定位仓库根"""
        result = subprocess.run(
            ["python", "-c",
             "import sys; sys.path.insert(0,'scripts'); "
             "from install_cli import _repo_root, _cli_target; "
             "cli = _cli_target(); print(cli.exists())"],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
            cwd=str(Path(__file__).parent.parent.parent)
        )
        assert "True" in result.stdout, f"CLI 文件不存在: {result.stderr}"

    def test_windows_path_append_is_case_insensitive_and_idempotent(self):
        """同一路径的大小写、斜杠和尾分隔符差异不得造成重复追加。"""
        original = "C:\\Windows;C:\\Users\\EDY\\AppData\\Local\\Programs\\AE-SDD\\"
        updated, changed = install_cli._append_windows_path_entry(
            original,
            r"c:/users/edy/appdata/local/programs/ae-sdd",
        )

        assert not changed
        assert updated == original

    def test_windows_path_remove_only_target_entry(self):
        """卸载时只移除 ae-sdd 目录，保留前后 PATH 条目。"""
        original = r"C:\Windows;C:\Tools;C:\Users\EDY\AppData\Local\Programs\ae-sdd"
        updated, changed = install_cli._remove_windows_path_entry(
            original,
            r"c:/users/edy/appdata/local/programs/AE-SDD/",
        )

        assert changed
        assert updated == r"C:\Windows;C:\Tools"

    def test_windows_path_mutation_preserves_empty_components(self):
        """PATH 中已有的空组件不得因安装或卸载被顺带归一化。"""
        original = r"C:\Windows;;C:\Tools;"
        appended, changed = install_cli._append_windows_path_entry(original, r"C:\AeSdd")

        assert changed
        assert appended == r"C:\Windows;;C:\Tools;;C:\AeSdd"

        removed, changed = install_cli._remove_windows_path_entry(
            r"C:\Windows;;C:\AeSdd;;C:\Tools;",
            r"c:/aesdd/",
        )

        assert changed
        assert removed == r"C:\Windows;;;C:\Tools;"

    def test_windows_check_requires_persisted_user_path(self, tmp_path, monkeypatch):
        """仅当前进程可见、但未写入用户 PATH 时，--check 必须失败。"""
        cli = tmp_path / "tools" / "bin" / "ae-sdd"
        cli.parent.mkdir(parents=True)
        cli.write_text("print('ok')\n", encoding="utf-8")
        shim_dir = tmp_path / "shim"
        shim_dir.mkdir()
        cmd_shim = shim_dir / "ae-sdd.cmd"
        cmd_shim.write_text("@echo off\n", encoding="utf-8")

        monkeypatch.setattr(install_cli, "_cli_target", lambda: cli)
        monkeypatch.setattr(install_cli, "_windows_shim_dir", lambda: shim_dir)
        monkeypatch.setattr(install_cli.platform, "system", lambda: "Windows")
        monkeypatch.setattr(
            install_cli,
            "_read_windows_user_path",
            lambda: (r"C:\Windows", 2),
        )
        monkeypatch.setattr(install_cli.shutil, "which", lambda *_args, **_kwargs: str(cmd_shim))

        assert install_cli.check_status() == 1

    def test_windows_check_rejects_conflicting_command_resolution(self, tmp_path, monkeypatch):
        """用户 PATH 已持久化时，其他目录中的同名命令也不能被误报为成功。"""
        cli = tmp_path / "tools" / "bin" / "ae-sdd"
        cli.parent.mkdir(parents=True)
        cli.write_text("print('ok')\n", encoding="utf-8")
        shim_dir = tmp_path / "shim"
        shim_dir.mkdir()
        wrong_shim = tmp_path / "other" / "ae-sdd.cmd"
        wrong_shim.parent.mkdir()
        wrong_shim.write_text("@echo off\n", encoding="utf-8")

        monkeypatch.setattr(install_cli, "_cli_target", lambda: cli)
        monkeypatch.setattr(install_cli, "_windows_shim_dir", lambda: shim_dir)
        monkeypatch.setattr(install_cli.platform, "system", lambda: "Windows")
        monkeypatch.setattr(
            install_cli,
            "_read_windows_user_path",
            lambda: (str(shim_dir), 2),
        )
        monkeypatch.setattr(install_cli.shutil, "which", lambda *_args, **_kwargs: str(wrong_shim))

        assert install_cli.check_status() == 1

    def test_windows_check_fails_when_version_command_fails(self, tmp_path, monkeypatch):
        """shim 可解析但实际执行失败时，--check 必须返回非零。"""
        cli = tmp_path / "tools" / "bin" / "ae-sdd"
        cli.parent.mkdir(parents=True)
        cli.write_text("print('ok')\n", encoding="utf-8")
        shim_dir = tmp_path / "shim"
        shim_dir.mkdir()
        cmd_shim = shim_dir / "ae-sdd.cmd"
        cmd_shim.write_text("@echo off\n", encoding="utf-8")

        monkeypatch.setattr(install_cli, "_cli_target", lambda: cli)
        monkeypatch.setattr(install_cli, "_windows_shim_dir", lambda: shim_dir)
        monkeypatch.setattr(install_cli.platform, "system", lambda: "Windows")
        monkeypatch.setattr(
            install_cli,
            "_read_windows_user_path",
            lambda: (str(shim_dir), 2),
        )
        monkeypatch.setattr(install_cli.shutil, "which", lambda *_args, **_kwargs: str(cmd_shim))
        monkeypatch.setattr(
            subprocess,
            "run",
            lambda *_args, **_kwargs: subprocess.CompletedProcess([], 1, "", "boom"),
        )

        assert install_cli.check_status() == 1

    def test_windows_install_writes_shims_and_persists_path_once(self, tmp_path, monkeypatch):
        """连续安装两次应保持 shim 内容稳定，用户 PATH 只出现一次。"""
        shim_dir = tmp_path / "ae-sdd-bin"
        cli = tmp_path / "tools" / "bin" / "ae-sdd"
        cli.parent.mkdir(parents=True)
        cli.write_text("print('ok')\n", encoding="utf-8")
        shim_dir.mkdir()
        (shim_dir / "ae-sdd.ps1").write_text("legacy\n", encoding="utf-8")

        registry = {"value": r"C:\Windows", "type": 2, "writes": 0}

        monkeypatch.setattr(install_cli, "_windows_shim_dir", lambda: shim_dir)
        monkeypatch.setattr(install_cli, "_python_exe", lambda: r"C:\Python\python.exe")
        monkeypatch.setattr(
            install_cli,
            "_read_windows_user_path",
            lambda: (registry["value"], registry["type"]),
        )

        def write_user_path(value, value_type):
            registry["value"] = value
            registry["type"] = value_type
            registry["writes"] += 1

        monkeypatch.setattr(install_cli, "_write_windows_user_path", write_user_path)
        monkeypatch.setattr(install_cli, "_broadcast_environment_change", lambda: None)
        monkeypatch.setenv("PATH", r"C:\Windows")

        assert install_cli.install_windows(cli) == 0
        assert install_cli.install_windows(cli) == 0

        assert (shim_dir / "ae-sdd.cmd").read_text(encoding="utf-8") == (
            "@echo off\n"
            f'"C:\\Python\\python.exe" "{cli}" %*\n'
        )
        assert not (shim_dir / "ae-sdd.ps1").exists()
        assert registry["writes"] == 1
        assert sum(
            install_cli._canonical_windows_path(item)
            == install_cli._canonical_windows_path(shim_dir)
            for item in install_cli._split_windows_path(registry["value"])
        ) == 1
        assert install_cli._canonical_windows_path(shim_dir) in {
            install_cli._canonical_windows_path(item)
            for item in install_cli._split_windows_path(os.environ["PATH"])
        }
