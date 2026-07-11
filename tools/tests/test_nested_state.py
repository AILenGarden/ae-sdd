"""v3.9.0 嵌套状态模型（Nested State Model）单测。

覆盖 7 条规则的核心实现：
  R1 单文件嵌套 state
  R2 任意节点出发 + 向上归入
  R3 子状态容器按 entryNode 选填
  R4 Bug/微任务独立扁平 state
  R5 改已管理 Story 重定位 + 重置子状态
  R6 顶层主体命名
  R7 路由自动匹配/新建
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

# 让 tools/lib 可导入
_TOOLS = Path(__file__).resolve().parent.parent
if str(_TOOLS) not in sys.path:
    sys.path.insert(0, str(_TOOLS))

from lib import state
from lib.flow_enums import FlowNode
from lib.paths import build_state_machine_name
from lib.classify import extract_requirement_features, match_state


# ─── R1/R3：嵌套 state schema + 子状态容器 ────────────────────────────────────


class TestNestedStateSchema:
    """R1/R3：嵌套 state 创建 + 容器选填。"""

    def test_init_nested_state_prd_has_all_containers(self):
        """R3: entryNode=PRD 含 prdState + drState + storyStates。"""
        s = state.init_nested_state(
            project_key="life", entry_node="PRD",
            state_machine_id="PRD-IM-CS", state_machine_name="IM CS",
            story_ids=["STORY-003-BE"], prd_id="PRD-IM-CS", dr_id="DR-CS",
        )
        assert state.is_nested_state(s)
        assert s["entryNode"] == "PRD"
        assert "prdState" in s
        assert "drStates" in s
        assert "drState" not in s
        assert "storyStates" in s
        assert "STORY-003-BE" in s["storyStates"]

    def test_init_nested_state_dr_has_dr_and_stories(self):
        """R3: entryNode=DR 含 drState + storyStates，无 prdState。"""
        s = state.init_nested_state(
            project_key="life", entry_node="DR",
            state_machine_id="DR-CS", state_machine_name="CS",
            story_ids=["STORY-003-BE"], dr_id="DR-CS",
        )
        assert "drState" in s
        assert "storyStates" in s
        assert "prdState" not in s

    def test_init_nested_state_story_only(self):
        """R3: entryNode=STORY 只含 storyStates。"""
        s = state.init_nested_state(
            project_key="life", entry_node="STORY",
            state_machine_id="Story-003-004-005", state_machine_name="Story 003/004/005",
            story_ids=["STORY-003-BE", "STORY-004-BE", "STORY-005-BE"],
        )
        assert "storyStates" in s
        assert "prdState" not in s
        assert "drState" not in s
        assert len(s["storyStates"]) == 3

    def test_init_nested_state_prd_requires_prd_id(self):
        """entryNode=PRD 必须提供 prd_id。"""
        try:
            state.init_nested_state(
                project_key="life", entry_node="PRD",
                state_machine_id="PRD-X", state_machine_name="X",
                dr_id="DR-X",
            )
            assert False, "应抛 ValueError"
        except ValueError as e:
            assert "prd_id" in str(e)

    def test_init_nested_state_invalid_entry_node(self):
        """非法 entryNode 抛 ValueError。"""
        try:
            state.init_nested_state(
                project_key="life", entry_node="TASK",
                state_machine_id="X", state_machine_name="X",
            )
            assert False, "应抛 ValueError"
        except ValueError:
            pass


# ─── R5：改已管理 Story 重定位 + 重置子状态 ──────────────────────────────────


class TestResetStorySubstate:
    """R5：重置只动目标 Story，兄弟不动。"""

    def _make_nested(self):
        return state.init_nested_state(
            project_key="life", entry_node="STORY",
            state_machine_id="Story-003-004-005", state_machine_name="test",
            story_ids=["STORY-003-BE", "STORY-004-BE"],
        )

    def test_reset_only_target_story(self):
        """R5 核心：重置 STORY-003，STORY-004 不动。"""
        s = self._make_nested()
        # 推进 STORY-003 到 coding
        state.set_story_substate_phase(s, "STORY-003-BE", "coding")
        # 重置
        result = state.reset_story_substate(s, "STORY-003-BE")
        assert result is True
        assert s["storyStates"]["STORY-003-BE"]["phase"] == state.STORY_RESET_TARGET_PHASE
        # STORY-004 仍是 initialized
        assert s["storyStates"]["STORY-004-BE"]["phase"] == "initialized"

    def test_reset_clears_completed_steps_and_coding_round(self):
        """重置清空 completedSteps + codingRound。"""
        s = self._make_nested()
        sub = s["storyStates"]["STORY-003-BE"]
        sub["completedSteps"] = ["step-1", "step-2"]
        sub["codingRound"] = 3
        state.reset_story_substate(s, "STORY-003-BE")
        assert sub["completedSteps"] == []
        assert sub["codingRound"] == 0

    def test_reset_keeps_history_audit(self):
        """重置保留 resetHistory 审计轨迹。"""
        s = self._make_nested()
        state.set_story_substate_phase(s, "STORY-003-BE", "coding")
        state.reset_story_substate(s, "STORY-003-BE")
        rh = s["storyStates"]["STORY-003-BE"]["resetHistory"]
        assert len(rh) == 1
        assert rh[0]["fromPhase"] == "coding"
        assert rh[0]["toPhase"] == state.STORY_RESET_TARGET_PHASE

    def test_reset_writes_artifact_invalidation(self):
        """v3.9.21：重置目标 Story 时写入 artifactInvalidated 信号，兄弟 Story 不写。"""
        s = self._make_nested()
        state.set_story_substate_phase(s, "STORY-003-BE", "coding")
        state.reset_story_substate(s, "STORY-003-BE", by="test")
        inv = s["storyStates"]["STORY-003-BE"]["artifactInvalidated"]
        assert inv is not None
        assert inv["by"] == "test"
        assert inv["reason"] == "story-substate-reset"
        assert set(inv["scopes"]) == {"TASK", "TESTCASE", "CODING_PLAN"}
        # 兄弟 STORY-004 无信号（init_nested_state 未写字段 → .get 返回 None）
        assert s["storyStates"]["STORY-004-BE"].get("artifactInvalidated") is None

    def test_consume_artifact_invalidation_is_one_shot(self):
        """v3.9.21：consume 一次性消费——首次返回记录并清除，再次返回 None。"""
        s = self._make_nested()
        state.reset_story_substate(s, "STORY-003-BE", by="test")
        # 首次消费：拿到记录
        rec = state.consume_artifact_invalidation(s, "STORY-003-BE")
        assert rec is not None
        assert rec["reason"] == "story-substate-reset"
        assert rec["scopes"] == ["TASK", "TESTCASE", "CODING_PLAN"]
        # 字段已清零
        assert s["storyStates"]["STORY-003-BE"]["artifactInvalidated"] is None
        # 再次消费：无信号
        assert state.consume_artifact_invalidation(s, "STORY-003-BE") is None

    def test_consume_artifact_invalidation_handles_edge_cases(self):
        """v3.9.21：非 nested / 不存在的 Story / 无信号均返回 None。"""
        # 非 nested（v1 flat）
        flat = {"version": "1", "stateModel": "flat"}
        assert state.consume_artifact_invalidation(flat, "STORY-003-BE") is None
        # nested 但 Story 不存在
        s = self._make_nested()
        assert state.consume_artifact_invalidation(s, "STORY-999-BE") is None
        # Story 存在但从未 reset（无信号）
        assert state.consume_artifact_invalidation(s, "STORY-003-BE") is None

    def test_reset_nonexistent_story_returns_false(self):
        """重置不存在的 Story 返回 False。"""
        s = self._make_nested()
        assert state.reset_story_substate(s, "STORY-999-BE") is False

    def test_completed_substate_syncs_workflow_projection(self):
        """Story 子状态推进 completed 时同步 currentPhase/currentStep/pendingOutputs/codingRound。"""
        s = self._make_nested()
        sub = s["storyStates"]["STORY-003-BE"]
        sub["phase"] = "code-reviewed"
        sub["currentPhase"] = "coding"
        sub["currentStep"] = "step-5-task-review-passed-awaiting-human-confirm"
        sub["pendingOutputs"] = {"humanConfirm": True}
        sub["codingRound"] = "r0"

        result = state.set_story_substate_phase(s, "STORY-003-BE", "completed")

        assert result is True
        assert sub["phase"] == "completed"
        assert sub["currentPhase"] == "completed"
        assert sub["currentStep"] == "completed"
        assert sub["pendingOutputs"] == {}
        assert sub["codingRound"] == 1
        assert "step-5-task-review-passed-awaiting-human-confirm" in sub["completedSteps"]

    def test_repeated_completed_substate_repairs_projection_without_history_dup(self):
        """phase 已是 completed 时仍修复投影字段，但不追加 phase history。"""
        s = self._make_nested()
        sub = s["storyStates"]["STORY-003-BE"]
        sub["phase"] = "completed"
        sub["currentPhase"] = "coding"
        sub["currentStep"] = "awaiting-human-confirm"
        sub["pendingOutputs"] = ["confirm"]
        sub["codingRound"] = 0
        history_len = len(s["history"])

        result = state.set_story_substate_phase(s, "STORY-003-BE", "completed")

        assert result is True
        assert len(s["history"]) == history_len
        assert sub["currentPhase"] == "completed"
        assert sub["currentStep"] == "completed"
        assert sub["pendingOutputs"] == []
        assert sub["codingRound"] == 1


# ─── R6：顶层主体命名 ────────────────────────────────────────────────────────


class TestBuildStateMachineName:
    """R6：只以顶层主体特征命名。"""

    def test_prd_naming(self):
        assert build_state_machine_name("PRD", {"prd_feature": "IM-CS"}) == "PRD-IM-CS"

    def test_dr_naming(self):
        assert build_state_machine_name("DR", {"dr_feature": "CS"}) == "DR-CS"

    def test_story_single(self):
        assert build_state_machine_name("STORY", {"story_ids": ["STORY-003-BE"]}) == "Story-003"

    def test_story_multi_merge(self):
        """多 Story 合并命名。"""
        result = build_state_machine_name(
            "STORY", {"story_ids": ["STORY-003-BE", "STORY-004-BE", "STORY-005-BE"]}
        )
        assert result == "Story-003-004-005"

    def test_invalid_top_node(self):
        try:
            build_state_machine_name("TASK", {})
            assert False
        except ValueError:
            pass


# ─── R2：entryNode 容器选择器 ────────────────────────────────────────────────


class TestEntryNodeContainerSelector:
    """R2：FlowNode.container_fields() + is_nested_entry()。"""

    def test_prd_containers(self):
        assert FlowNode.PRD.container_fields() == ["prdState", "drStates", "storyStates"]

    def test_dr_containers(self):
        assert FlowNode.DR.container_fields() == ["drState", "storyStates"]

    def test_story_containers(self):
        assert FlowNode.STORY.container_fields() == ["storyStates"]

    def test_task_flat_no_containers(self):
        """TASK 是 flat 节点，返回空列表。"""
        assert FlowNode.TASK.container_fields() == []

    def test_is_nested_entry(self):
        assert FlowNode.is_nested_entry("PRD") is True
        assert FlowNode.is_nested_entry("DR") is True
        assert FlowNode.is_nested_entry("STORY") is True
        assert FlowNode.is_nested_entry("TASK") is False
        assert FlowNode.is_nested_entry("PLAN") is False


# ─── R7：路由自动匹配/新建 ──────────────────────────────────────────────────


class TestExtractRequirementFeatures:
    """R7：需求特征提取。"""

    def test_extract_bug_fix(self):
        f = extract_requirement_features("修个 typo in config")
        assert f.is_bug_fix is True
        assert f.modifies_story is False
        assert f.top_node == "TASK"

    def test_extract_story_ids(self):
        f = extract_requirement_features("撰写 STORY-003-BE STORY-004-BE 的 Story 文档")
        assert f.story_ids == ["STORY-003-BE", "STORY-004-BE"]
        assert f.modifies_story is True
        assert f.top_node == "STORY"

    def test_extract_dr_id(self):
        f = extract_requirement_features("基于 DR-CS 生成 STORY-006-BE")
        assert f.dr_id == "DR-CS"
        assert "STORY-006-BE" in f.story_ids

    def test_extract_prd_id(self):
        f = extract_requirement_features("PRD-IM-CS 的 Story 生成")
        assert f.prd_id == "PRD-IM-CS"


class TestMatchState:
    """R7：match_state 匹配优先级。"""

    def test_match_bug_fix_creates_flat(self):
        """R4: Bug 不改 Story → create_flat。"""
        f = extract_requirement_features("修个 typo")
        # 用 ae-sdd 母版目录（无 .ae-sdd）测试
        mr = match_state(Path("D:/Item/ae-sdd"), f)
        assert mr.action == "create_flat"

    def test_match_no_project_creates_nested_with_naming(self):
        """无 .ae-sdd 目录时仍生成命名。"""
        f = extract_requirement_features("撰写 STORY-003-BE STORY-004-BE")
        mr = match_state(Path("D:/Item/ae-sdd"), f)
        assert mr.action == "create_nested"
        assert mr.entry_node == "STORY"
        assert mr.naming == "Story-003-004"

    def test_match_empty_project_create_nested(self, tmp_path):
        """Project without existing nested state → create_nested."""
        (tmp_path / ".ae-sdd").mkdir()
        f = extract_requirement_features("撰写 STORY-003-BE STORY-004-BE STORY-005-BE 的 Story 文档")
        mr = match_state(tmp_path, f)
        assert mr.action == "create_nested"
        assert mr.entry_node == "STORY"
        assert mr.naming == "Story-003-004-005"


# ─── 兼容性：v1 flat state 不受影响 ──────────────────────────────────────────


class TestV1FlatCompatibility:
    """v1 扁平 state 读写兼容。"""

    def test_flat_state_is_not_nested(self):
        """v1 state 无 stateModel 字段 → is_nested_state 返回 False。"""
        flat = {"version": "1", "phase": "initialized", "currentStory": "STORY-001"}
        assert state.is_nested_state(flat) is False

    def test_get_active_phase_flat(self):
        """flat state 的 get_active_phase 返回顶层 phase。"""
        flat = {"phase": "coding", "currentStory": "STORY-001"}
        assert state.get_active_phase(flat) == "coding"

    def test_get_active_story_flat(self):
        """flat state 的 get_active_story 返回 currentStory。"""
        flat = {"currentStory": "STORY-001"}
        assert state.get_active_story(flat) == "STORY-001"

    def test_get_active_phase_nested(self):
        """nested state 的 get_active_phase 返回 activeStory 子状态 phase。"""
        s = state.init_nested_state(
            project_key="life", entry_node="STORY",
            state_machine_id="Story-003", state_machine_name="test",
            story_ids=["STORY-003-BE"],
        )
        state.set_story_substate_phase(s, "STORY-003-BE", "story-reviewed")
        assert state.get_active_phase(s) == "story-reviewed"


# ─── v3.9.1 回归：gate_intercept 对嵌套 state 的感知 ──────────────────────────
# 病灶：v3.9.0 prompt_inject 迁移到 get_active_phase/get_active_story，但
#       gate_intercept.check_intercept 漏迁，仍读顶层 phase/currentStory。
#       嵌套 state 顶层无这些字段 → hook 永远看到 phase=initialized ∈ _DESIGN_PHASES
#       → 所有 src/ 写入被误拦为"设计阶段禁止写入源码目录"。
# 本组测试用真实 state.json 文件驱动 check_intercept（不传 forced_phase），
# 确保 hook 真正读嵌套 state 的 activeStory 子状态 phase。


class TestNestedNextStepSuggestion(unittest.TestCase):

    def test_next_step_suggestion_uses_nested_active_phase(self):
        s = state.init_nested_state(
            project_key="life", entry_node="STORY",
            state_machine_id="Story-003", state_machine_name="test",
            story_ids=["STORY-003-BE"],
        )
        state.set_scale(s, "中")  # 🆕 v3.10.0：Story 入口 = 中链
        state.set_story_substate_phase(s, "STORY-003-BE", "story-generated")

        suggestion = state.next_step_suggestion(s)

        assert suggestion["current"] == "story-generated"
        assert suggestion["next"] == "testcase-generated"  # v3.10.1 子系列合并


class TestNestedFlowMonitor:
    """Flow monitor must evaluate the nested activeStory phase, not top-level phase."""

    def test_detect_drift_uses_nested_active_phase(self, tmp_path, monkeypatch):
        from lib import flow_monitor

        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir()
        s = state.init_nested_state(
            project_key="test",
            entry_node="STORY",
            state_machine_id="Story-003",
            state_machine_name="story",
            story_ids=["STORY-003-BE"],
        )
        state.set_story_substate_phase(s, "STORY-003-BE", "story-generated")

        monkeypatch.setattr(
            flow_monitor,
            "_run_gates_check",
            lambda gate_id, _ade_sdd: (False, f"{gate_id} failed"),
        )

        drift = flow_monitor.detect_drift(s, ae_sdd)

        assert drift.phase == "story-generated"
        assert drift.gate_id == "G-02"
        assert drift.drift_type == "fake-complete"


class TestGateInterceptNestedState:
    """v3.9.1：gate_intercept 必须用统一接口读嵌套 state 的 active phase/story。"""

    def _make_nested_project(self, tmp_path, story_id, sub_phase, scale="微"):
        """构造嵌套 state 的 .ae-sdd/ 项目目录。

        - state.json 为 v3.9.0 nested schema：storyStates[story_id].phase = sub_phase
        - config.yaml 含 projectKey
        - assets/<key>.assets.md 含 docWorkspacePath
        """
        import json
        from lib.state import init_nested_state, set_story_substate_phase, set_scale

        ae_sdd = tmp_path / ".ae-sdd"
        (ae_sdd / "assets").mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")

        s = init_nested_state(
            project_key="test", entry_node="STORY",
            state_machine_id="Story-X", state_machine_name="tn",
            story_ids=[story_id],
        )
        set_scale(s, scale)
        set_story_substate_phase(s, story_id, sub_phase)
        state_path = tmp_path / ".auto-engineering" / "Story-X" / "state.json"
        state_path.parent.mkdir(parents=True, exist_ok=True)
        state_path.write_text(
            json.dumps(s, ensure_ascii=False), encoding="utf-8"
        )

        (ae_sdd / "assets" / "test.assets.md").write_text(
            f"| gitPath | `{tmp_path}` |\n| docWorkspacePath | `{tmp_path}` |\n",
            encoding="utf-8",
        )
        return tmp_path

    def _stub_session_and_memory(self, monkeypatch):
        """打桩关卡3（is_phase_confirmed）与关卡5.5（memory enter）放行，聚焦验证 phase 读取。"""
        from lib import session as session_mod
        from lib import memory_gate

        monkeypatch.setattr(session_mod, "is_phase_confirmed", lambda *a, **kw: True)
        monkeypatch.setattr(
            memory_gate, "check_state_transition",
            lambda **kw: {"blocked": False},
        )
        # 关卡5.5 _check_memory_entered 调 memory_store.is_scope_active
        from lib import memory_store
        monkeypatch.setattr(memory_store, "is_scope_active", lambda *a, **kw: True)

    def test_nested_coding_phase_allows_src_write(self, tmp_path, monkeypatch):
        """嵌套 state activeStory 子状态 phase=coding → src/ 写入放行（不被误判为 initialized）。"""
        from lib.gate_intercept import check_intercept

        project = self._make_nested_project(
            tmp_path, story_id="STORY-003-BE", sub_phase="coding"
        )
        self._stub_session_and_memory(monkeypatch)

        src_file = str(tmp_path / "backend" / "src" / "main" / "java" / "X.java")
        allowed, reason = check_intercept(
            "Write", file_path=src_file, project_dir=project, forced_engaged=True
        )
        assert allowed, f"嵌套 state coding phase 应放行 src/ 写入，但被拦: {reason}"

    def test_nested_design_phase_blocks_src_write(self, tmp_path, monkeypatch):
        """嵌套 state activeStory 子状态 phase=story-generated → src/ 写入被关卡2拦截。"""
        from lib.gate_intercept import check_intercept

        project = self._make_nested_project(
            tmp_path, story_id="STORY-003-BE", sub_phase="story-generated"
        )
        # design phase 不触达关卡3/5.5，但打桩以防环境差异
        self._stub_session_and_memory(monkeypatch)

        src_file = str(tmp_path / "backend" / "src" / "main" / "java" / "X.java")
        allowed, reason = check_intercept(
            "Write", file_path=src_file, project_dir=project, forced_engaged=True
        )
        assert not allowed, "嵌套 state story-generated phase 应拦截 src/ 写入"
        assert "设计阶段" in reason, f"拒绝理由应含'设计阶段'，实际: {reason}"

    def test_product_landing_unknown_story_denies_without_exception(self, tmp_path, monkeypatch):
        """Product STORY-ID ownership gate should deny cleanly, not crash on doc_save_hint."""
        from lib.gate_intercept import check_intercept

        project = self._make_nested_project(
            tmp_path, story_id="STORY-003-BE", sub_phase="task-reviewed", scale="小"
        )
        self._stub_session_and_memory(monkeypatch)

        target = tmp_path / "ae-sdd-doc" / "Coding" / "STORY-999-BE" / "STORY-999-BE-CodingPlan.md"
        allowed, reason = check_intercept(
            "Write", file_path=str(target), project_dir=project, forced_engaged=True
        )

        assert not allowed
        assert "未登记到当前 state" in reason

    def test_source_write_session_check_exception_fails_closed(self, tmp_path, monkeypatch):
        """Coding source writes must not pass when the session confirmation check crashes."""
        from lib import session as session_mod
        from lib.gate_intercept import check_intercept

        project = self._make_nested_project(
            tmp_path, story_id="STORY-003-BE", sub_phase="coding", scale="小"
        )
        monkeypatch.setattr(
            session_mod,
            "is_phase_confirmed",
            lambda *a, **kw: (_ for _ in ()).throw(RuntimeError("boom")),
        )

        src_file = str(tmp_path / "backend" / "src" / "main" / "java" / "X.java")
        allowed, reason = check_intercept(
            "Write", file_path=src_file, project_dir=project, forced_engaged=True
        )

        assert not allowed
        assert "门禁自检异常" in reason

    def test_flat_state_coding_phase_allows_src_write(self, tmp_path, monkeypatch):
        """flat state phase=coding → src/ 写入放行（v1 行为回归保护）。"""
        import json
        from lib.gate_intercept import check_intercept

        ae_sdd = tmp_path / ".ae-sdd"
        (ae_sdd / "assets").mkdir(parents=True, exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        state_path = tmp_path / ".auto-engineering" / "Story-001" / "state.json"
        state_path.parent.mkdir(parents=True, exist_ok=True)
        state_path.write_text(json.dumps({
            "version": "1",
            "projectKey": "test",
            "phase": "coding",
            "scale": "微",
            "workItemKey": "Story-001",
            "stateMachineId": "Story-001",
            "currentWorkItem": "Story-001",
            "currentStory": "STORY-001",
            "currentTask": None,
            "history": [],
        }, ensure_ascii=False), encoding="utf-8")
        (ae_sdd / "assets" / "test.assets.md").write_text(
            f"| gitPath | `{tmp_path}` |\n| docWorkspacePath | `{tmp_path}` |\n",
            encoding="utf-8",
        )
        self._stub_session_and_memory(monkeypatch)

        src_file = str(tmp_path / "backend" / "src" / "main" / "java" / "X.java")
        allowed, reason = check_intercept(
            "Write", file_path=src_file, project_dir=tmp_path, forced_engaged=True
        )
        assert allowed, f"flat state coding phase 应放行 src/ 写入，但被拦: {reason}"
