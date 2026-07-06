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
        assert "drState" in s
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

    def test_reset_nonexistent_story_returns_false(self):
        """重置不存在的 Story 返回 False。"""
        s = self._make_nested()
        assert state.reset_story_substate(s, "STORY-999-BE") is False


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
        assert FlowNode.PRD.container_fields() == ["prdState", "drState", "storyStates"]

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

    def test_match_life_project_create_nested(self):
        """life 项目无嵌套 state → create_nested。"""
        f = extract_requirement_features("撰写 STORY-003-BE STORY-004-BE STORY-005-BE 的 Story 文档")
        mr = match_state(Path("D:/Item/life"), f)
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
