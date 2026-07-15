"""test_prompt_inject_plugin.py -- 集成测试：B1 修复（外挂运行时接入）

验证 prompt_inject.inject() 会把 next_step_suggestion 的 skill 文件名过一遍
plugin_loader，命中外挂时注入 "plugin: ..." 行，引导 Agent 加载外挂路径。

覆盖场景（补 v3.5.0 的测试盲区 B3）：
1. 命中 L1 项目层外挂 → additionalContext 含 "plugin:" + 外挂路径
2. 无任何注册表 → additionalContext 不含 "plugin:"（保持原 skill 行）
3. plugin_loader 异常 → 降级，不抛错（失败优先原则）

接入点：tools/lib/prompt_inject.py 的 _resolve_skill_path() + inject()。
"""
import sys
import tempfile
import json
import subprocess
import unittest
from pathlib import Path
from unittest import mock

# Make 'lib' importable
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import prompt_inject, work_item_context  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
CLI_PATH = REPO_ROOT / "tools" / "bin" / "ae-sdd"


def _additional_context(payload: dict) -> str:
    hook_output = payload["hookSpecificOutput"]
    assert hook_output["hookEventName"] == "UserPromptSubmit"
    return hook_output["additionalContext"]


def _run_prompt_inject_cli(project_dir: Path, payload: dict) -> dict:
    proc = subprocess.run(
        [sys.executable, str(CLI_PATH), "prompt-inject", "--project", str(project_dir)],
        input=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert proc.returncode == 0, proc.stderr.decode("utf-8", errors="replace")
    raw = proc.stdout.decode("utf-8", errors="replace").strip()
    return json.loads(raw or "{}")


def _write_nested_work_item(tmp: Path, name: str, story_id: str, phase: str) -> Path:
    state_path = tmp / ".auto-engineering" / name / "state.json"
    state_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "version": "2",
        "projectKey": "test-proj",
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


def _write_flat_work_item(tmp: Path, name: str, story_id: str, phase: str) -> Path:
    state_path = tmp / ".auto-engineering" / name / "state.json"
    state_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "version": "1",
        "projectKey": "test-proj",
        "phase": phase,
        "scale": "\u5c0f",
        "stateMachineId": name,
        "workItemKey": name,
        "currentWorkItem": name,
        "storyId": story_id,
        "currentStory": story_id,
        "history": [],
    }
    state_path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    return state_path


def _make_project_with_plugin(tmp: Path, skill_target: str = "coding-skill.md") -> Path:
    """构造一个带 L1 项目层外挂注册表的项目目录，返回其 .ae-sdd 路径。

    skill_target: 要外挂替换的 SKILL 裸文件名（默认 coding-skill.md）。
    """
    ade_sdd = tmp / ".ae-sdd"
    plugins_dir = ade_sdd / "plugins"
    plugins_dir.mkdir(parents=True)

    # 外挂 SKILL 文件
    ext = plugins_dir / "my-skill.md"
    ext.write_text("# My Custom SKILL", encoding="utf-8")

    # registry.yaml（replaces 用内置完整路径）
    builtin_target = prompt_inject._SKILL_FILE_TO_BUILTIN_TARGET[skill_target]
    registry = plugins_dir / "registry.yaml"
    registry.write_text(
        "schema_version: 1\n"
        "plugins:\n"
        "  - name: my-plugin\n"
        "    type: skill-override\n"
        "    version: 1.0.0\n"
        f"    description: test\n"
        "    path: ./my-skill.md\n"
        f"    replaces: {builtin_target}\n",
        encoding="utf-8",
    )

    # 项目最小配置（prompt_inject 需读 config.yaml 的 projectKey）
    (ade_sdd / "config.yaml").write_text("projectKey: test-proj\n", encoding="utf-8")

    # 让 phase 停在会触发 skill=coding-skill.md 的阶段
    # 🆕 v3.5.16: coding-process → coding（skill=coding-skill.md）
    # （旧 task-reviewed → coding-skill.md 已变更：task-reviewed 现指向 coding-process-skill.md）
    _write_nested_work_item(tmp, "Story-001", "STORY-001", "coding-process")

    return ade_sdd


class TestResolveSkillPath(unittest.TestCase):
    """_resolve_skill_path 单元测试。"""

    def test_returns_none_for_empty_or_dash(self):
        self.assertIsNone(prompt_inject._resolve_skill_path("", None, None))
        self.assertIsNone(prompt_inject._resolve_skill_path("—", None, None))
        self.assertIsNone(prompt_inject._resolve_skill_path("?", None, None))

    def test_returns_none_for_unmapped_file(self):
        # 映射表未覆盖的文件名 → None（保持原行为）
        self.assertIsNone(prompt_inject._resolve_skill_path("unknown-skill.md", None, None))

    def test_returns_none_when_no_registry(self):
        # 无注册表 → plugin_loader fallback → None
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-"))
        ade_sdd = tmp / ".ae-sdd"
        ade_sdd.mkdir()
        result = prompt_inject._resolve_skill_path("coding-skill.md", ade_sdd, None)
        self.assertIsNone(result)

    def test_returns_plugin_line_when_hit(self):
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-"))
        ade_sdd = _make_project_with_plugin(tmp)
        result = prompt_inject._resolve_skill_path("coding-skill.md", ade_sdd, None)
        self.assertIsNotNone(result)
        self.assertIn("my-plugin", result)
        self.assertIn("L1-project", result)

    def test_degrades_on_plugin_loader_exception(self):
        # plugin_loader.resolve_skill 抛异常 → 返回 None，不抛错
        with mock.patch("lib.plugin_loader.resolve_skill", side_effect=RuntimeError("boom")):
            tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-"))
            ade_sdd = _make_project_with_plugin(tmp)
            result = prompt_inject._resolve_skill_path("coding-skill.md", ade_sdd, None)
        self.assertIsNone(result)


class TestInjectPluginLine(unittest.TestCase):
    """inject() 端到端：additionalContext 是否含 plugin 行。"""

    def test_inject_contains_plugin_line_when_hit(self):
        """命中外挂 → additionalContext 含 'plugin:' 行 + 外挂路径。"""
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-"))
        _make_project_with_plugin(tmp)
        payload = prompt_inject.inject(project_dir=tmp, user_prompt="/ae-sdd 继续编码")
        msg = _additional_context(payload)
        self.assertIn("plugin:", msg)
        self.assertIn("my-plugin", msg)
        self.assertIn("⚠️ 本次必须加载此 外挂路径", msg)
        # harness 闭合标签仍在
        self.assertIn("<!-- /ae-sdd harness -->", msg)

    def test_inject_no_plugin_line_when_no_registry(self):
        """无注册表 → additionalContext 不含 'plugin:'，仅含原 skill 行。"""
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-"))
        ade_sdd = tmp / ".ae-sdd"
        ade_sdd.mkdir(parents=True)
        import json
        _write_nested_work_item(tmp, "Story-001", "STORY-001", "task-reviewed")
        (ade_sdd / "config.yaml").write_text("projectKey: test-proj\n", encoding="utf-8")

        payload = prompt_inject.inject(project_dir=tmp, user_prompt="/ae-sdd 继续编码")
        msg = _additional_context(payload)
        self.assertNotIn("plugin:", msg)
        # 原 skill 行仍在
        self.assertIn("skill:", msg)


class TestPromptInjectParallelWorkItemIsolation(unittest.TestCase):
    """Prompt hook must not consume a stale project-global mirror in multi-work-item projects."""

    def _make_multi_work_item_project(self) -> tuple[Path, Path, Path]:
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-parallel-"))
        ade_sdd = tmp / ".ae-sdd"
        ade_sdd.mkdir(parents=True)
        (ade_sdd / "config.yaml").write_text("projectKey: test-proj\n", encoding="utf-8")
        sp_a = _write_nested_work_item(tmp, "Story-004", "STORY-004-BE", "story-generated")
        sp_b = _write_nested_work_item(tmp, "Story-005", "STORY-005-BE", "coding")
        mirror = json.loads(sp_a.read_text(encoding="utf-8"))
        mirror["activeWorkItem"] = "Story-004"
        mirror["activeStatePath"] = str(sp_a)
        (ade_sdd / "state.json").write_text(json.dumps(mirror, ensure_ascii=False), encoding="utf-8")
        return tmp, sp_a, sp_b

    def test_inject_mentioned_story_uses_matching_work_item_not_mirror(self):
        tmp, _, _ = self._make_multi_work_item_project()

        payload = prompt_inject.inject(project_dir=tmp, user_prompt="/ae-sdd 处理 STORY-005-BE")
        msg = _additional_context(payload)

        self.assertIn("STORY-005-BE", msg)
        self.assertNotIn("story:    STORY-004-BE", msg)

    def test_inject_blocks_ambiguous_multi_work_item_without_story(self):
        tmp, _, _ = self._make_multi_work_item_project()

        payload = prompt_inject.inject(project_dir=tmp, user_prompt="/ae-sdd 继续")
        msg = _additional_context(payload)

        self.assertIn("--work-item", msg)
        self.assertIn("Story-004", msg)
        self.assertIn("Story-005", msg)
        self.assertNotIn("story:    STORY-004-BE", msg)

    def test_cli_prompt_field_binds_full_work_item_key_session(self):
        tmp, _, _ = self._make_multi_work_item_project()
        _write_flat_work_item(tmp, "cs-ai-STORY-005-BE", "cs-ai-STORY-005-BE", "initialized")

        payload = _run_prompt_inject_cli(tmp, {
            "hook_event_name": "UserPromptSubmit",
            "session_id": "claude-session-005",
            "prompt": "/ae-sdd continue cs-ai-STORY-005-BE",
            "cwd": str(tmp),
        })
        msg = _additional_context(payload)
        bound = work_item_context.read_session_binding(tmp / ".ae-sdd", "claude-session-005")

        self.assertIn("cs-ai-STORY-005-BE", msg)
        self.assertIsNotNone(bound)
        self.assertEqual(bound.key, "cs-ai-STORY-005-BE")
        self.assertNotIn("WORK-ITEM AMBIGUITY", msg)
        self.assertNotIn("state new --id STORY-005-BE", msg)

    def test_inject_degrades_gracefully_on_resolve_failure(self):
        """plugin_loader 异常 → inject 仍返回有效 payload，不含 plugin 行。"""
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-"))
        _make_project_with_plugin(tmp)
        with mock.patch("lib.plugin_loader.resolve_skill", side_effect=RuntimeError("boom")):
            payload = prompt_inject.inject(project_dir=tmp, user_prompt="/ae-sdd 继续编码")
        msg = _additional_context(payload)
        self.assertNotIn("plugin:", msg)
        self.assertIn("skill:", msg)  # 降级为原 skill 裸文件名


class TestPromptInjectCli(unittest.TestCase):
    def test_returns_additional_context_with_utf8_stdin(self):
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-cli-"))
        ade_sdd = tmp / ".ae-sdd"
        ade_sdd.mkdir(parents=True)
        (ade_sdd / "config.yaml").write_text("projectKey: test-proj\n", encoding="utf-8")
        _write_nested_work_item(tmp, "Story-REQ-001", "REQ-001", "ra-generated")

        payload = _run_prompt_inject_cli(tmp, {
            "hook_event_name": "UserPromptSubmit",
            "session_id": "prompt-cli-session",
            "user_prompt": "/ae-sdd continue DR",
        })
        msg = _additional_context(payload)
        self.assertIn("REQ-001", msg)
        self.assertNotIn("systemMessage", payload)

    def test_plain_prompt_does_not_inject_or_keep_stale_activity(self):
        tmp = Path(tempfile.mkdtemp(prefix="ae-sdd-inj-plain-"))
        ade_sdd = tmp / ".ae-sdd"
        ade_sdd.mkdir(parents=True)
        (ade_sdd / "config.yaml").write_text("projectKey: test-proj\n", encoding="utf-8")
        _write_nested_work_item(tmp, "Story-001", "STORY-001", "story-generated")
        work_item_context.mark_session_engaged(ade_sdd, "plain-session")

        payload = prompt_inject.inject(
            project_dir=tmp,
            user_prompt="检查 Story-003 和 Story-004 文档对齐",
            session_key="plain-session",
        )

        self.assertEqual(payload, {})
        self.assertFalse(work_item_context.is_session_engaged(ade_sdd, "plain-session"))


if __name__ == "__main__":
    unittest.main()
