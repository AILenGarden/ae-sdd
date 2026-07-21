"""
test_flow_enums.py — flow_enums.py + state.py events 集成测试

覆盖：
  - FlowNode / FlowSkill / FlowEventType 枚举值格式（str, Enum）
  - FlowEvent.to_dict() None 字段过滤
  - 工厂函数：make_routed_to / make_skill_completed / make_gate_blocked /
              make_phase_changed / make_user_confirmed
  - state.py append_event：seq 自增、ts 自动填充、原子写
  - state.py get_events：全量 / txnName 过滤 / event_type 过滤 / node 过滤
  - 与现有 set_phase / history 兼容（events 独立，不影响旧字段）
"""
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import state as state_mod  # noqa: E402
from lib import flow_monitor  # noqa: E402
from lib.flow_enums import (  # noqa: E402
    FlowEvent,
    FlowEventType,
    FlowNode,
    FlowSkill,
    make_gate_blocked,
    make_phase_changed,
    make_routed_to,
    make_skill_completed,
    make_user_confirmed,
)


class TestFlowNodeEnum(unittest.TestCase):
    """FlowNode 枚举测试"""

    def test_values_are_strings(self):
        for node in FlowNode:
            self.assertIsInstance(node.value, str)

    def test_all_nodes_present(self):
        # 🆕 v3.10.0：新增 BUG（微任务入口），TASK deprecated 但保留
        expected = {"PRD", "RA", "DR", "STORY", "TASK", "PLAN", "BUG"}
        actual = {n.value for n in FlowNode}
        self.assertEqual(actual, expected)

    def test_str_subclass(self):
        """继承 str 后可直接做字符串比较"""
        self.assertEqual(FlowNode.RA, "RA")
        self.assertEqual(FlowNode.PLAN, "PLAN")


class TestFlowSkillEnum(unittest.TestCase):
    """FlowSkill 枚举测试"""

    def test_values_end_with_skill(self):
        for skill in FlowSkill:
            self.assertTrue(
                skill.value.endswith("-skill"),
                f"{skill.name} value '{skill.value}' 应以 -skill 结尾",
            )

    def test_coding_skill_value(self):
        self.assertEqual(FlowSkill.CODING, "coding-skill")

    def test_str_subclass(self):
        self.assertEqual(FlowSkill.REQUIREMENT_ANALYSIS, "requirement-analysis-skill")


class TestFlowEventTypeEnum(unittest.TestCase):
    """FlowEventType 枚举测试"""

    def test_all_types_present(self):
        expected = {
            "routed-to", "skill-completed", "gate-blocked", "gate-cleared",
            "user-confirmed", "phase-changed", "reopened", "aborted",
        }
        actual = {e.value for e in FlowEventType}
        self.assertEqual(actual, expected)

    def test_str_subclass(self):
        self.assertEqual(FlowEventType.ROUTED_TO, "routed-to")


class TestFlowEventToDict(unittest.TestCase):
    """FlowEvent.to_dict() 测试"""

    def test_none_fields_filtered(self):
        ev = FlowEvent(
            seq=1, ts="2026-06-26T10:00:00Z",
            event="routed-to", node="RA", by="ae-sdd",
        )
        d = ev.to_dict()
        # None 字段不出现
        self.assertNotIn("skill", d)
        self.assertNotIn("txnName", d)
        self.assertNotIn("reason", d)
        self.assertNotIn("output", d)
        self.assertNotIn("meta", d)

    def test_required_fields_present(self):
        ev = FlowEvent(
            seq=2, ts="2026-06-26T10:00:00Z",
            event="routed-to", node="RA", by="ae-sdd",
        )
        d = ev.to_dict()
        self.assertEqual(d["seq"], 2)
        self.assertEqual(d["event"], "routed-to")
        self.assertEqual(d["node"], "RA")
        self.assertEqual(d["by"], "ae-sdd")

    def test_optional_fields_included_when_set(self):
        ev = FlowEvent(
            seq=3, ts="2026-06-26T10:00:00Z",
            event="skill-completed", node="RA", by="requirement-analysis-skill",
            skill="requirement-analysis-skill",
            txnName="STORY-001-BE",
            output={"type": "RADoc", "path": "ae-sdd-doc/RA.md"},
        )
        d = ev.to_dict()
        self.assertEqual(d["skill"], "requirement-analysis-skill")
        self.assertEqual(d["txnName"], "STORY-001-BE")
        self.assertEqual(d["output"]["type"], "RADoc")


class TestFactoryFunctions(unittest.TestCase):
    """工厂函数测试"""

    TS = "2026-06-26T10:00:00Z"

    def test_make_routed_to_basic(self):
        ev = make_routed_to(1, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS)
        self.assertEqual(ev.event, FlowEventType.ROUTED_TO)
        self.assertEqual(ev.node, "RA")
        self.assertEqual(ev.skill, "requirement-analysis-skill")
        self.assertEqual(ev.by, "ae-sdd")

    def test_make_routed_to_with_from_node(self):
        ev = make_routed_to(
            2, self.TS, FlowNode.STORY, FlowSkill.STORY_GENERATE,
            from_node=FlowNode.RA, reason="规模裁定/中任务",
        )
        self.assertEqual(ev.from_node, "RA")
        self.assertEqual(ev.reason, "规模裁定/中任务")

    def test_make_skill_completed_with_output(self):
        ev = make_skill_completed(
            3, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS,
            output_type="RADoc", output_path="ae-sdd-doc/RA.md",
            artifact_id="RA-CS-001",
        )
        self.assertEqual(ev.event, FlowEventType.SKILL_COMPLETED)
        self.assertEqual(ev.by, "requirement-analysis-skill")
        self.assertIsNotNone(ev.output)
        self.assertEqual(ev.output["type"], "RADoc")
        self.assertEqual(ev.output["artifact_id"], "RA-CS-001")

    def test_make_skill_completed_no_output(self):
        ev = make_skill_completed(4, self.TS, FlowNode.RA, FlowSkill.DR_REVIEW)
        self.assertIsNone(ev.output)

    def test_make_gate_blocked(self):
        ev = make_gate_blocked(5, self.TS, FlowNode.STORY, "G-03", "Story Review 未通过")
        self.assertEqual(ev.event, FlowEventType.GATE_BLOCKED)
        self.assertEqual(ev.gate_id, "G-03")
        self.assertEqual(ev.reason, "Story Review 未通过")
        self.assertEqual(ev.by, "ae-sdd")

    def test_make_phase_changed(self):
        ev = make_phase_changed(6, self.TS, FlowNode.RA, "ra-generated")
        self.assertEqual(ev.event, FlowEventType.PHASE_CHANGED)
        self.assertEqual(ev.phase, "ra-generated")
        self.assertEqual(ev.by, "ae-sdd state write")

    def test_make_user_confirmed(self):
        ev = make_user_confirmed(7, self.TS, FlowNode.STORY, "Story Review 通过，AC 已补全")
        self.assertEqual(ev.event, FlowEventType.USER_CONFIRMED)
        self.assertEqual(ev.by, "user")
        self.assertEqual(ev.reason, "Story Review 通过，AC 已补全")


class TestAppendEvent(unittest.TestCase):
    """state.append_event 测试"""

    TS = "2026-06-26T10:00:00Z"

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_append_creates_events_list(self):
        s = state_mod.read_state(self.state_path)
        self.assertNotIn("events", s)
        ev = make_routed_to(1, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS)
        state_mod.append_event(s, ev)
        self.assertIn("events", s)
        self.assertEqual(len(s["events"]), 1)

    def test_append_seq_auto_increment(self):
        s = state_mod.read_state(self.state_path)
        # seq=0 应自动赋值为 1
        ev1 = FlowEvent(seq=0, ts=self.TS, event="routed-to", node="RA", by="ae-sdd")
        state_mod.append_event(s, ev1)
        self.assertEqual(s["events"][0]["seq"], 1)
        # 再追加一条 seq=0，应自动赋值为 2
        ev2 = FlowEvent(seq=0, ts=self.TS, event="skill-completed", node="RA", by="ae-sdd")
        state_mod.append_event(s, ev2)
        self.assertEqual(s["events"][1]["seq"], 2)

    def test_append_ts_auto_fill(self):
        s = state_mod.read_state(self.state_path)
        ev = FlowEvent(seq=1, ts="", event="routed-to", node="RA", by="ae-sdd")
        state_mod.append_event(s, ev)
        ts = s["events"][0]["ts"]
        self.assertRegex(ts, r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

    def test_append_persists_via_write_read(self):
        """append_event 后通过 write/read 持久化，数据保持一致"""
        s = state_mod.read_state(self.state_path)
        ev = make_routed_to(1, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS,
                            reason="测试路由")
        state_mod.append_event(s, ev)
        state_mod.write_state(self.state_path, s)

        s2 = state_mod.read_state(self.state_path)
        self.assertEqual(len(s2["events"]), 1)
        self.assertEqual(s2["events"][0]["reason"], "测试路由")
        self.assertEqual(s2["events"][0]["node"], "RA")

    def test_append_multiple_events_order(self):
        s = state_mod.read_state(self.state_path)
        ev1 = make_routed_to(1, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS)
        ev2 = make_skill_completed(2, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS)
        state_mod.append_event(s, ev1)
        state_mod.append_event(s, ev2)
        self.assertEqual(s["events"][0]["event"], "routed-to")
        self.assertEqual(s["events"][1]["event"], "skill-completed")

    def test_append_does_not_affect_history(self):
        """events 独立，不影响现有 history / phase 字段。
        🆕 v3.10.0：大链从 dr-generated 起（ra-generated 已移除）。"""
        s = state_mod.read_state(self.state_path)
        state_mod.set_phase(s, "dr-generated")
        ev = make_routed_to(1, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS)
        state_mod.append_event(s, ev)
        # phase 和 history 不变
        self.assertEqual(s["phase"], "dr-generated")
        self.assertEqual(len(s["history"]), 1)
        # events 独立增长
        self.assertEqual(len(s["events"]), 1)


class TestGetEvents(unittest.TestCase):
    """state.get_events 过滤测试"""

    TS = "2026-06-26T10:00:00Z"

    def _make_state_with_events(self) -> dict:
        s = state_mod.read_state(Path("/nonexistent/state.json"))
        events = [
            make_routed_to(1, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS),
            make_skill_completed(2, self.TS, FlowNode.RA, FlowSkill.REQUIREMENT_ANALYSIS,
                                 txn_name=None),
            make_routed_to(3, self.TS, FlowNode.STORY, FlowSkill.STORY_GENERATE,
                           txn_name="STORY-001-BE"),
            make_skill_completed(4, self.TS, FlowNode.STORY, FlowSkill.STORY_GENERATE,
                                 txn_name="STORY-001-BE"),
            make_gate_blocked(5, self.TS, FlowNode.STORY, "G-03", "AC 缺失",
                              txn_name="STORY-001-BE"),
            make_routed_to(6, self.TS, FlowNode.STORY, FlowSkill.STORY_GENERATE,
                           txn_name="STORY-002-BE"),
        ]
        for ev in events:
            state_mod.append_event(s, ev)
        return s

    def test_get_all_events(self):
        s = self._make_state_with_events()
        all_ev = state_mod.get_events(s)
        self.assertEqual(len(all_ev), 6)

    def test_filter_by_txn_name(self):
        s = self._make_state_with_events()
        story1_ev = state_mod.get_events(s, txn_name="STORY-001-BE")
        self.assertEqual(len(story1_ev), 3)
        for ev in story1_ev:
            self.assertEqual(ev["txnName"], "STORY-001-BE")

    def test_filter_by_event_type(self):
        s = self._make_state_with_events()
        routed = state_mod.get_events(s, event_type=FlowEventType.ROUTED_TO)
        self.assertEqual(len(routed), 3)
        for ev in routed:
            self.assertEqual(ev["event"], "routed-to")

    def test_filter_by_node(self):
        s = self._make_state_with_events()
        ra_ev = state_mod.get_events(s, node=FlowNode.RA)
        self.assertEqual(len(ra_ev), 2)

    def test_filter_combined(self):
        s = self._make_state_with_events()
        blocked = state_mod.get_events(
            s,
            txn_name="STORY-001-BE",
            event_type=FlowEventType.GATE_BLOCKED,
        )
        self.assertEqual(len(blocked), 1)
        self.assertEqual(blocked[0]["gate_id"], "G-03")

    def test_result_sorted_by_seq(self):
        s = self._make_state_with_events()
        all_ev = state_mod.get_events(s)
        seqs = [e["seq"] for e in all_ev]
        self.assertEqual(seqs, sorted(seqs))

    def test_empty_events_returns_empty_list(self):
        s = state_mod.read_state(Path("/nonexistent/state.json"))
        self.assertEqual(state_mod.get_events(s), [])


class TestBackwardCompatibility(unittest.TestCase):
    """向后兼容：旧 state.json（无 events 字段）不报错"""

    def test_read_old_state_no_events(self):
        import tempfile
        tmp = Path(tempfile.mktemp(suffix=".json"))
        old = {"version": "1", "phase": "story-generated", "history": []}
        tmp.write_text(json.dumps(old), encoding="utf-8")
        s = state_mod.read_state(tmp)
        # get_events 不报错，返回空列表
        self.assertEqual(state_mod.get_events(s), [])
        tmp.unlink(missing_ok=True)


class TestCompactFlowMonitor(unittest.TestCase):

    def test_compact_map_uses_structured_process_artifacts(self):
        gate_map = flow_monitor.get_phase_gate_map({"processPolicy": "compact"})

        self.assertNotIn("testcase-generated", gate_map)
        self.assertEqual(gate_map["coding-process"], ["G-08"])
        self.assertEqual(gate_map["coding"], ["G-CODEPLAN-SRC", "G-HTTP-1"])
        self.assertEqual(gate_map["test-running"], ["G-10"])
        self.assertEqual(gate_map["code-reviewed"], ["G-12"])

    def test_legacy_map_keeps_historical_testcase_compatibility(self):
        gate_map = flow_monitor.get_phase_gate_map({"phase": "testcase-generated"})

        self.assertEqual(gate_map["testcase-generated"], ["G-04"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
