"""
test_state.py — state.py 单元测试

覆盖：
- read_state（空 + 已有）
- write_state（原子写：tmp + rename）
- record_history
- set_phase（合法/非法 phase）
- next_step_suggestion（所有 10 个 phase）
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import state as state_mod  # noqa: E402


class TestReadState(unittest.TestCase):
    """read_state 测试"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_read_nonexistent_returns_default(self):
        s = state_mod.read_state(self.state_path)
        self.assertEqual(s["version"], "1")
        self.assertEqual(s["phase"], "initialized")
        self.assertIsNone(s["currentStory"])
        self.assertIsNone(s["currentTask"])
        self.assertEqual(s["history"], [])

    def test_read_existing(self):
        self.state_path.write_text(json.dumps({
            "version": "1",
            "projectKey": "test",
            "phase": "story-generated",
            "currentStory": "STORY-001",
            "currentTask": None,
            "history": [{"phase": "initialized", "timestamp": "2026-06-18T00:00:00Z", "by": "init"}],
        }, ensure_ascii=False), encoding="utf-8")
        s = state_mod.read_state(self.state_path)
        self.assertEqual(s["phase"], "story-generated")
        self.assertEqual(s["currentStory"], "STORY-001")
        self.assertEqual(len(s["history"]), 1)


class TestWriteState(unittest.TestCase):
    """write_state 测试（原子写）"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_write_creates_file(self):
        s = state_mod.read_state(self.state_path)
        state_mod.set_phase(s, "dr-generated")
        state_mod.write_state(self.state_path, s)
        self.assertTrue(self.state_path.is_file())
        reloaded = json.loads(self.state_path.read_text(encoding="utf-8"))
        self.assertEqual(reloaded["phase"], "dr-generated")

    def test_write_atomic_no_leftover_tmp(self):
        """写完后不应残留 .tmp 文件"""
        s = state_mod.read_state(self.state_path)
        state_mod.write_state(self.state_path, s)
        self.assertFalse((self.state_path.with_suffix(".json.tmp")).exists())

    def test_write_overwrites_existing(self):
        self.state_path.write_text('{"phase": "old"}', encoding="utf-8")
        s = state_mod.read_state(self.state_path)
        s["phase"] = "new"
        state_mod.write_state(self.state_path, s)
        reloaded = json.loads(self.state_path.read_text(encoding="utf-8"))
        self.assertEqual(reloaded["phase"], "new")


class TestSetPhase(unittest.TestCase):
    """set_phase 测试"""

    def test_set_valid_phase(self):
        s = {"history": []}
        state_mod.set_phase(s, "dr-generated", by="test")
        self.assertEqual(s["phase"], "dr-generated")
        self.assertEqual(len(s["history"]), 1)
        self.assertEqual(s["history"][0]["by"], "test")
        self.assertIn("timestamp", s["history"][0])

    def test_set_invalid_phase_raises(self):
        s = {"history": []}
        with self.assertRaises(ValueError) as ctx:
            state_mod.set_phase(s, "totally-bogus-phase")
        self.assertIn("未知 phase", str(ctx.exception))

    def test_default_by_is_ae_sdd(self):
        s = {"history": []}
        state_mod.set_phase(s, "initialized")
        self.assertEqual(s["history"][0]["by"], "ae-sdd")

    def test_completed_phase_synchronizes_workflow_projection(self):
        s = {
            "phase": "code-reviewed",
            "scale": "大",
            "history": [{"phase": "coding", "timestamp": "2026-07-09T00:00:00Z", "by": "test"}],
            "currentPhase": "coding",
            "currentStep": "step-5-task-review-passed-awaiting-human-confirm",
            "completedSteps": [],
            "pendingOutputs": {"humanConfirm": True},
            "codingRound": "r0",
        }
        changed = state_mod.set_phase(s, "completed", by="test")
        self.assertTrue(changed)
        self.assertEqual(s["phase"], "completed")
        self.assertEqual(s["currentPhase"], "completed")
        self.assertEqual(s["currentStep"], "completed")
        self.assertEqual(s["pendingOutputs"], {})
        self.assertEqual(s["codingRound"], 1)
        self.assertIn("step-5-task-review-passed-awaiting-human-confirm", s["completedSteps"])
        self.assertEqual(s["history"][-1]["phase"], "completed")

    def test_repeated_completed_repairs_projection_without_history_dup(self):
        s = {
            "phase": "completed",
            "scale": "大",
            "history": [{"phase": "completed", "timestamp": "2026-07-09T00:00:00Z", "by": "test"}],
            "currentPhase": "coding",
            "currentStep": "awaiting-human-confirm",
            "completedSteps": [],
            "pendingOutputs": ["confirm"],
            "codingRound": 0,
        }
        changed = state_mod.set_phase(s, "completed", by="test")
        self.assertTrue(changed)
        self.assertEqual(len(s["history"]), 1)
        self.assertEqual(s["currentPhase"], "completed")
        self.assertEqual(s["currentStep"], "completed")
        self.assertEqual(s["pendingOutputs"], [])
        self.assertEqual(s["codingRound"], 1)
        self.assertIn("awaiting-human-confirm", s["completedSteps"])

    def test_completed_projection_already_synced_is_idempotent(self):
        s = {
            "phase": "completed",
            "scale": "大",
            "history": [],
            "currentPhase": "completed",
            "currentStep": "completed",
            "completedSteps": [],
            "pendingOutputs": {},
            "codingRound": 1,
        }
        changed = state_mod.set_phase(s, "completed", by="test")
        self.assertFalse(changed)

    def test_paused_does_not_complete_current_step(self):
        s = {
            "phase": "coding",
            "scale": "大",
            "history": [],
            "currentPhase": "coding",
            "currentStep": "step-4-coding-r1",
            "completedSteps": [],
            "codingRound": 1,
        }
        changed = state_mod.set_phase(s, "paused", by="test")
        self.assertTrue(changed)
        self.assertEqual(s["phase"], "paused")
        self.assertEqual(s["currentPhase"], "paused")
        self.assertEqual(s["currentStep"], "step-4-coding-r1")
        self.assertEqual(s["completedSteps"], [])


class TestStateInvariants(unittest.TestCase):
    """终态不变量校验"""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.state_path = self.tmp / "state.json"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_write_state_rejects_completed_projection_violation(self):
        s = {
            "phase": "completed",
            "scale": "大",
            "currentPhase": "coding",
            "currentStep": "awaiting-human-confirm",
            "pendingOutputs": {"confirm": True},
            "codingRound": 0,
            "history": [],
        }
        with self.assertRaises(ValueError) as ctx:
            state_mod.write_state(self.state_path, s)
        self.assertIn("state invariant violation", str(ctx.exception))
        self.assertFalse(self.state_path.exists())

    def test_write_state_accepts_synced_completed_projection(self):
        s = {
            "phase": "completed",
            "scale": "大",
            "currentPhase": "completed",
            "currentStep": "completed",
            "pendingOutputs": {},
            "codingRound": 1,
            "history": [],
        }
        state_mod.write_state(self.state_path, s)
        self.assertTrue(self.state_path.exists())


class TestRecordHistory(unittest.TestCase):
    """record_history 测试"""

    def test_records_timestamp(self):
        s = {"history": []}
        state_mod.record_history(s, "story-generated", by="story-generate-skill")
        self.assertEqual(len(s["history"]), 1)
        entry = s["history"][0]
        self.assertEqual(entry["phase"], "story-generated")
        self.assertEqual(entry["by"], "story-generate-skill")
        # ISO 8601 UTC 格式
        self.assertRegex(entry["timestamp"], r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

    def test_appends_in_order(self):
        s = {"history": []}
        state_mod.record_history(s, "phase-1")
        state_mod.record_history(s, "phase-2")
        state_mod.record_history(s, "phase-3")
        self.assertEqual([h["phase"] for h in s["history"]],
                         ["phase-1", "phase-2", "phase-3"])


class TestNextStepSuggestion(unittest.TestCase):
    """next_step_suggestion 测试 — 🆕 v3.5.15 按 scale 选子链"""

    def test_initialized_to_ra_generate_large(self):
        """🆕 v3.10.2 大链 initialized -> ra-generated（4 loop 从 RA 起）"""
        s = {"phase": "initialized", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "route-selected")

    def test_micro_initialized_to_coding_process(self):
        """🆕 v3.10.0 微链 initialized -> coding-process（无文档直出 CodingPlan）"""
        s = {"phase": "initialized", "scale": "微"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "route-selected")
        self.assertIn("classify", sug["skill"])

    def test_small_initialized_to_coding_process(self):
        """🆕 v3.10.0 小链 initialized -> coding-process（CodingPlan 入口，已有 Story+TestCase）"""
        s = {"phase": "initialized", "scale": "小"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "route-selected")

    def test_story_generated_to_testcase_generated(self):
        """🆕 v3.10.1 子系列合并：story-generated（=generate+review loop 完成）-> testcase-generated。
        大/中链有效；小链从 coding-process 起，不含 story 系列。"""
        for scale in ("大", "中"):
            s = {"phase": "story-generated", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "testcase-generated", f"scale={scale}")

    def test_testcase_generated_to_coding_process(self):
        """🆕 v3.10.1 子系列合并：testcase-generated（=generate+review loop 完成）-> coding-process"""
        for scale in ("大", "中"):
            s = {"phase": "testcase-generated", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "coding-process", f"scale={scale}")

    def test_medium_initialized_to_dr_generated(self):
        """🆕 v3.10.2 中链 initialized -> dr-generated（3 loop 从 DR 起，跳 RA）"""
        s = {"phase": "initialized", "scale": "中"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "route-selected")

    def test_large_dr_to_story(self):
        """🆕 v3.10.0 大链 dr-generated -> story-generated"""
        s = {"phase": "dr-generated", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "story-generated")

    def test_completed_terminates(self):
        s = {"phase": "completed", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertIn("已结束", sug["next"])

    def test_coding_to_test_series(self):
        """coding → test-running 必须进入 Test 系列，而非由 CodingSkill 代跑测试。"""
        for scale in ("大", "中", "小", "微"):
            s = {"phase": "coding", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "test-running", f"scale={scale}")
            self.assertIn("test-generate-skill.md", sug["skill"])

    def test_test_running_to_code_review_requires_test_review_first(self):
        """test-running → code-reviewed 文案必须保留 Test Review 前置语义。"""
        for scale in ("大", "中", "小"):
            s = {"phase": "test-running", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "code-reviewed", f"scale={scale}")
            self.assertIn("Test Review", sug["action"])

    def test_all_phases_have_suggestion_per_chain(self):
        """🆕 v3.5.15 每条子链每个 phase 都该有建议"""
        for scale, chain in state_mod.PHASE_FLOWS.items():
            for phase in chain:
                sug = state_mod.next_step_suggestion({"phase": phase, "scale": scale})
                self.assertIn("current", sug, f"scale={scale} phase={phase} 缺 current")
                self.assertIn("next", sug, f"scale={scale} phase={phase} 缺 next")
                self.assertIn("action", sug, f"scale={scale} phase={phase} 缺 action")
                self.assertIn("skill", sug, f"scale={scale} phase={phase} 缺 skill")


class TestPhaseFlowCoverage(unittest.TestCase):
    """🆕 v3.5.15 PHASE_FLOWS 4 子链完整性测试"""

    def test_phase_flows_has_4_scales(self):
        self.assertEqual(set(state_mod.PHASE_FLOWS.keys()),
                         {"大", "中", "小", "微"})

    def test_large_chain_has_10_phases(self):
        # 🆕 v3.10.2：4 loop（RA-DR-Story-TestCase）+ Coding/Testing 2 phase
        self.assertEqual(len(state_mod.PHASE_FLOWS["大"]), 11)

    def test_medium_chain_has_9_phases(self):
        # 🆕 v3.10.2：3 loop（DR-Story-TestCase）+ Coding/Testing 2 phase，跳 RA
        self.assertEqual(len(state_mod.PHASE_FLOWS["中"]), 10)

    def test_small_chain_has_6_phases(self):
        # 🆕 v3.10.0：小=CodingPlan 入口，直出 coding-process
        self.assertEqual(len(state_mod.PHASE_FLOWS["小"]), 8)

    def test_micro_chain_has_6_phases(self):
        # 🆕 v3.10.0：微=无文档直出 coding-process，移除 task-generated/task-reviewed
        self.assertEqual(len(state_mod.PHASE_FLOWS["微"]), 8)

    def test_micro_chain_includes_code_reviewed(self):
        """🆕 2026-07-03(B1): 微链必须含 code-reviewed（CodeReview 报告不豁免）"""
        self.assertIn("code-reviewed", state_mod.PHASE_FLOWS["微"])

    def test_all_chains_start_with_initialized(self):
        for scale, chain in state_mod.PHASE_FLOWS.items():
            self.assertEqual(chain[0], "initialized", f"{scale} 链起点非 initialized")

    def test_all_chains_end_with_completed(self):
        for scale, chain in state_mod.PHASE_FLOWS.items():
            self.assertEqual(chain[-1], "completed", f"{scale} 链终点非 completed")

    def test_micro_chain_skips_ra(self):
        """微链不含 ra-generated（核心修复点）"""
        self.assertNotIn("ra-generated", state_mod.PHASE_FLOWS["微"])

    def test_phase_flow_alias_is_large_chain(self):
        """向后兼容：PHASE_FLOW 别名 = 大链"""
        self.assertEqual(state_mod.PHASE_FLOW, state_mod.PHASE_FLOWS["大"])

    def test_selected_dr_can_skip_story(self):
        s = {"phase": "requirement-analyzed", "scale": "大", "processPolicy": "compact"}
        state_mod.set_design_route(s, "DR", reason="architecture only")
        chain = state_mod.phase_chain_for_state(s)
        self.assertIn("dr-generated", chain)
        self.assertNotIn("story-generated", chain)

    def test_selected_story_skips_dr(self):
        s = {"phase": "requirement-analyzed", "scale": "大", "processPolicy": "compact"}
        state_mod.set_design_route(s, "STORY")
        chain = state_mod.phase_chain_for_state(s)
        self.assertNotIn("dr-generated", chain)
        self.assertIn("story-generated", chain)

    def test_selected_coding_plan_skips_design_documents(self):
        s = {"phase": "requirement-analyzed", "scale": "大", "processPolicy": "compact"}
        state_mod.set_design_route(s, "CODING_PLAN")
        self.assertEqual(
            state_mod.next_step_suggestion(s)["next"],
            "coding-process",
        )


class TestSetScale(unittest.TestCase):
    """🆕 v3.5.15 set_scale 测试"""

    def test_set_valid_scale(self):
        s = {}
        state_mod.set_scale(s, "微")
        self.assertEqual(s["scale"], "微")

    def test_set_scale_with_entry_node(self):
        s = {}
        state_mod.set_scale(s, "微", entry_node="BUG")
        self.assertEqual(s["scale"], "微")
        self.assertEqual(s["entryNode"], "BUG")

    def test_set_invalid_scale_raises(self):
        s = {}
        with self.assertRaises(ValueError) as ctx:
            state_mod.set_scale(s, "巨")
        self.assertIn("未知 scale", str(ctx.exception))


class TestInferScale(unittest.TestCase):
    """🆕 v3.5.15 _infer_scale 旧 state 兼容测试"""

    def test_infer_large_from_dr_step(self):
        s = {"completedSteps": ["step-1-dr2story"], "phase": "story-generated"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "大")
        self.assertGreaterEqual(conf, 0.8)

    def test_infer_medium_from_story_step(self):
        s = {"completedSteps": ["step-1-story"], "phase": "story-reviewed"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "中")

    def test_infer_small_from_coding_step(self):
        """🆕 v3.10.0：砍 Task 后，completedSteps 含 coding（无 story）-> 小（CodingPlan 入口）"""
        s = {"completedSteps": ["step-1-coding"], "phase": "coding"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "小")

    def test_infer_coding_phase_defaults_large(self):
        """🟡 v3.5.15 安全策略：coding 阶段无法可靠区分微 vs 大，默认大（最保守）。
        微任务必须显式 --scale=微，不靠反推。"""
        s = {"completedSteps": [], "phase": "coding"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "大")
        self.assertLess(conf, 0.5, "coding 阶段反推置信度应低，提示用户显式 --scale")

    def test_infer_default_large_when_unknown(self):
        """无法判定 → 默认大（最保守）"""
        s = {"completedSteps": [], "phase": "initialized"}
        scale, conf, _ = state_mod._infer_scale(s)
        self.assertEqual(scale, "大")
        self.assertLess(conf, 0.5, "无法判定时置信度应低，提示用户显式 --scale")


class TestSetPhasePerScale(unittest.TestCase):
    """🆕 v3.5.15 set_phase 按 scale 子链校验"""

    def test_micro_set_coding_from_initialized(self):
        """微链允许 initialized → coding"""
        s = {"phase": "initialized", "scale": "微", "history": []}
        state_mod.set_phase(s, "coding")
        self.assertEqual(s["phase"], "coding")

    def test_large_set_coding_is_valid_phase(self):
        """set_phase 只校验 phase ∈ 子链，不拦跨步跳跃（跨步由 gate_intercept 管）。
        coding 在大链内 → 合法 phase，不抛 ValueError。"""
        s = {"phase": "initialized", "scale": "大", "history": []}
        # coding 在大链内，set_phase 不拦（跨步跳跃拦截是 gate_intercept 职责，M2 处理）
        state_mod.set_phase(s, "coding")
        self.assertEqual(s["phase"], "coding")

    def test_set_phase_outside_chain_blocked(self):
        """phase 不在当前 scale 子链内 → ValueError。
        微链不含 ra-generated，微任务 set ra-generated 应被拦。"""
        s = {"phase": "initialized", "scale": "微", "history": []}
        with self.assertRaises(ValueError):
            state_mod.set_phase(s, "ra-generated")

    def test_set_bogus_phase_blocked(self):
        """完全非法 phase 名 → ValueError"""
        s = {"phase": "initialized", "scale": "大", "history": []}
        with self.assertRaises(ValueError):
            state_mod.set_phase(s, "totally-bogus-phase")

    def test_legacy_state_no_scale_infers_and_allows(self):
        """旧 state 无 scale -> _resolve_scale 反推后 set_phase 正常工作。
        🆕 v3.10.0：大链从 dr-generated 起（ra-generated 已移除）。"""
        s = {"phase": "initialized", "history": []}  # 无 scale 字段
        # 反推：无 completedSteps + initialized -> 默认大链
        # dr-generated 在大链内，合法
        state_mod.set_phase(s, "dr-generated")
        self.assertEqual(s["phase"], "dr-generated")
        # 反推结果应回写
        self.assertEqual(s["scale"], "大")


# ─── 🆕 v3.8.1 S-3：文件意图锁测试 ─────────────────────────────────────────────


class TestFileLocks(unittest.TestCase):
    """S-3 文件意图锁：acquire / check / release / TTL / 冲突。"""

    def test_acquire_and_check_lock(self):
        """获取锁后 check_file_lock 返回持锁信息"""
        s = {"phase": "coding", "history": []}
        ok, reason = state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-1")
        self.assertTrue(ok)
        self.assertEqual(reason, "")
        lock = state_mod.check_file_lock(s, "design/STORY-001.md")
        self.assertIsNotNone(lock)
        self.assertEqual(lock["agentId"], "agent-1")
        self.assertEqual(lock["ttlSeconds"], 1800)

    def test_acquire_conflict_blocks(self):
        """被他人持锁时获取失败，reason 含持锁 agentId"""
        s = {"phase": "coding", "history": []}
        state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-1")
        ok, reason = state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-2")
        self.assertFalse(ok)
        self.assertIn("agent-1", reason)
        # 原 lock 未被覆盖
        self.assertEqual(s["fileLocks"]["design/STORY-001.md"]["agentId"], "agent-1")

    def test_acquire_idempotent_same_agent(self):
        """同 agent 重复获取同一文件锁 → 幂等成功"""
        s = {"phase": "coding", "history": []}
        state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-1")
        ok, reason = state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-1")
        self.assertTrue(ok)
        self.assertEqual(reason, "")

    def test_release_only_by_holder(self):
        """仅持锁者能释放；非持锁者释放返回 False"""
        s = {"phase": "coding", "history": []}
        state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-1")
        # 非持锁者释放
        self.assertFalse(state_mod.release_file_lock(s, "design/STORY-001.md", "agent-2"))
        self.assertIsNotNone(state_mod.check_file_lock(s, "design/STORY-001.md"))
        # 持锁者释放
        self.assertTrue(state_mod.release_file_lock(s, "design/STORY-001.md", "agent-1"))
        self.assertIsNone(state_mod.check_file_lock(s, "design/STORY-001.md"))

    def test_ttl_expiry_allows_preempt(self):
        """TTL 过期的旧锁可被新 agent 抢占（防崩溃死锁）"""
        s = {"phase": "coding", "history": []}
        # 注入一个已过期的锁（acquiredAt 为 1 小时前）
        from datetime import datetime, timezone, timedelta
        old_ts = (datetime.now(timezone.utc) - timedelta(hours=1))
        old_ts_str = old_ts.strftime("%Y-%m-%dT%H:%M:%SZ")
        s["fileLocks"] = {"design/STORY-001.md": {
            "agentId": "agent-dead", "acquiredAt": old_ts_str, "ttlSeconds": 1800,
        }}
        # 过期锁 → check 视作未锁
        self.assertIsNone(state_mod.check_file_lock(s, "design/STORY-001.md"))
        # 新 agent 可抢占
        ok, _ = state_mod.acquire_file_lock(s, "design/STORY-001.md", "agent-2")
        self.assertTrue(ok)
        self.assertEqual(s["fileLocks"]["design/STORY-001.md"]["agentId"], "agent-2")

    def test_check_unlocked_returns_none(self):
        """未上锁的文件 check 返回 None"""
        s = {"phase": "coding", "history": []}
        self.assertIsNone(state_mod.check_file_lock(s, "nonexistent.md"))


# ─── 🆕 v3.8.1 S-5：PRD compact runtime 分支测试 ───────────────────────────────


class TestPrdComplete(unittest.TestCase):
    """S-5 prd_complete：3 runtime 分支 + summary.md 生成 + prdStatus 流转。"""

    def _make_prd_state(self):
        return {
            "prdId": "PRD-CS-001",
            "prdTitle": "测试 PRD",
            "prdStatus": "in_progress",
            "storyIds": [{"storyId": "STORY-001"}, {"storyId": "STORY-002"}],
            "events": [{"seq": 1}, {"seq": 2}],
        }

    def test_harness_generates_summary_and_status(self):
        """harness runtime：生成 summary.md + prdStatus → awaiting_compact，无 trigger"""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ps = self._make_prd_state()
            r = state_mod.prd_complete(ps, "PRD-CS-001", "harness", root)
            self.assertTrue(r["summaryPath"].endswith("summary.md"))
            self.assertFalse(r["compactTrigger"])
            self.assertIn("harness session rotate", r["runtimeHint"])
            # summary.md 真实生成
            self.assertTrue(Path(r["summaryPath"]).is_file())
            # prdStatus 流转
            self.assertEqual(ps["prdStatus"], "awaiting_compact")
            # 无 compact-trigger 文件
            self.assertFalse((root / ".ae-sdd" / "compact-trigger").is_file())

    def test_claude_code_writes_compact_trigger(self):
        """claude-code runtime：生成 summary.md + 写 compact-trigger 文件"""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ps = self._make_prd_state()
            r = state_mod.prd_complete(ps, "PRD-CS-001", "claude-code", root)
            self.assertTrue(r["compactTrigger"])
            self.assertIn("/compact", r["runtimeHint"])
            trigger = root / ".ae-sdd" / "compact-trigger"
            self.assertTrue(trigger.is_file())
            import json as _json
            payload = _json.loads(trigger.read_text(encoding="utf-8"))
            self.assertEqual(payload["prdId"], "PRD-CS-001")

    def test_codex_marks_pending_research(self):
        """codex runtime：生成 summary.md，runtimeHint 标注'待调研'"""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ps = self._make_prd_state()
            r = state_mod.prd_complete(ps, "PRD-CS-001", "codex", root)
            self.assertFalse(r["compactTrigger"])
            self.assertIn("待调研", r["runtimeHint"])
            self.assertTrue(Path(r["summaryPath"]).is_file())

    def test_summary_contains_prd_metadata(self):
        """summary.md 含 prdId/prdTitle/storyIds 等关键元数据"""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ps = self._make_prd_state()
            r = state_mod.prd_complete(ps, "PRD-CS-001", "harness", root)
            content = Path(r["summaryPath"]).read_text(encoding="utf-8")
            self.assertIn("PRD-CS-001", content)
            self.assertIn("测试 PRD", content)
            self.assertIn("STORY-001", content)
            self.assertIn("STORY-002", content)

    def test_already_compacted_not_overwritten(self):
        """prdStatus 已 compacted 时不重复流转（幂等）"""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ps = self._make_prd_state()
            ps["prdStatus"] = "compacted"
            state_mod.prd_complete(ps, "PRD-CS-001", "harness", root)
            self.assertEqual(ps["prdStatus"], "compacted")


# ─── 🆕 v3.9.3 R2 递归向上归入 ─────────────────────────────────────────────
class TestRecursiveR2Absorb(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="r2-"))
        self.ade_sdd = self.tmp / ".ae-sdd"
        self.ade_sdd.mkdir()
        self.design_dir = self.tmp / "design"
        self.design_dir.mkdir()

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _make_story_doc(self, story_id: str, parent_dr: str = "", parent_prd: str = "") -> Path:
        body = f"# {story_id}\n\n## 元信息\n\n- Story ID: {story_id}\n"
        if parent_prd:
            body += f"- 来源 PRD: {parent_prd}\n"
        if parent_dr:
            body += f"- 来源 DR: {parent_dr}\n"
        p = self.design_dir / f"{story_id}-Story.md"
        p.write_text(body, encoding="utf-8")
        return p

    def _make_dr_doc(self, dr_id: str, story_ids: list, parent_prd: str = "") -> Path:
        body = f"# {dr_id}\n\n## 元信息\n\n- DR ID: {dr_id}\n"
        if parent_prd:
            body += f"- PRD: {parent_prd}\n"
        body += "\n## Story 拆分\n\n" + "\n".join(f"- {s}" for s in story_ids) + "\n"
        p = self.design_dir / f"{dr_id}-some-title.md"
        p.write_text(body, encoding="utf-8")
        return p

    def test_no_parent_top_level(self):
        """无父级 → 当前 STORY 为顶层。"""
        doc = self._make_story_doc("STORY-006-BE")
        features = {"story_ids": ["STORY-006-BE"]}
        sp, st = state_mod.recursive_r2_absorb(
            self.ade_sdd, "STORY", features, self.design_dir,
            doc_path=doc, child_id="STORY-006-BE",
        )
        self.assertEqual(sp.parent.name, "Story-006")
        self.assertTrue(sp.is_file())

    def test_parent_dr_doc_not_found_treated_as_no_parent(self):
        """父级 DR 文档不存在 → 视为无父级（不阻塞）。"""
        doc = self._make_story_doc("STORY-006-BE", parent_dr="DR-999")
        features = {"story_ids": ["STORY-006-BE"]}
        sp, st = state_mod.recursive_r2_absorb(
            self.ade_sdd, "STORY", features, self.design_dir,
            doc_path=doc, child_id="STORY-006-BE",
        )
        self.assertEqual(sp.parent.name, "Story-006")

    def test_parent_dr_relation_mismatch_returns_top_level(self):
        """父级 DR 文档存在但关联性不对（DR 没列 STORY-006）→ 顶层（不阻塞）。"""
        self._make_dr_doc("DR-005", ["STORY-999-BE"])
        doc = self._make_story_doc("STORY-006-BE", parent_dr="DR-005")
        features = {"story_ids": ["STORY-006-BE"]}
        sp, st = state_mod.recursive_r2_absorb(
            self.ade_sdd, "STORY", features, self.design_dir,
            doc_path=doc, child_id="STORY-006-BE",
        )
        self.assertEqual(sp.parent.name, "Story-006")

    def test_parent_dr_relation_ok_absorbs_into_dr(self):
        """父级 DR 关联性对 + DR 无 state → 替 DR 递归建 state + Story 嵌进去。"""
        self._make_dr_doc("DR-005", ["STORY-006-BE", "STORY-007-BE"])
        doc = self._make_story_doc("STORY-006-BE", parent_dr="DR-005")
        features = {"story_ids": ["STORY-006-BE"]}
        sp, st = state_mod.recursive_r2_absorb(
            self.ade_sdd, "STORY", features, self.design_dir,
            doc_path=doc, child_id="STORY-006-BE",
        )
        self.assertEqual(sp.parent.name, "DR-005")
        self.assertTrue(sp.is_file())
        self.assertEqual(st["drState"]["drId"], "DR-005")
        self.assertIn("STORY-006-BE", st.get("storyStates", {}))

    def test_three_layer_chain(self):
        """三层链：PRD → DR → Story（验证 Story 嵌进 DR）。"""
        prd_body = "# PRD-001\n\n## DR 拆分\n\n- DR-005\n"
        (self.design_dir / "PRD-001-some-product.md").write_text(prd_body, encoding="utf-8")
        self._make_dr_doc("DR-005", ["STORY-006-BE"], parent_prd="PRD-001")
        doc = self._make_story_doc("STORY-006-BE", parent_dr="DR-005")
        features = {"story_ids": ["STORY-006-BE"]}
        sp, st = state_mod.recursive_r2_absorb(
            self.ade_sdd, "STORY", features, self.design_dir,
            doc_path=doc, child_id="STORY-006-BE",
        )
        self.assertEqual(sp.parent.name, "PRD-001")
        self.assertTrue(sp.is_file())
        self.assertFalse((self.tmp / ".auto-engineering" / "DR-005" / "state.json").exists())
        self.assertEqual(st["prdState"]["prdId"], "PRD-001")
        self.assertIn("DR-005", st.get("drStates", {}))
        self.assertIn("STORY-006-BE", st["drStates"]["DR-005"].get("storyStates", {}))
        self.assertIn("STORY-006-BE", st.get("storyStates", {}))


# ─── 🆕 v3.9.18 is_work_item_completed：聚合全部 Story 子状态判定整体完结 ──────
class TestIsWorkItemCompleted(unittest.TestCase):
    """get_active_phase() 只看 activeStory 指向的子状态，某 Story 完成后
    activeStory 不会自动前移，会把仍有未完成 Story 的 work-item 误判为整体
    已完结；is_work_item_completed() 改为聚合全部 Story 子状态判定。"""

    def _nested(self, active_story: str, story_states: dict) -> dict:
        return {
            "version": "2",
            "stateModel": "nested",
            "entryNode": "STORY",
            "activeStory": active_story,
            "storyStates": story_states,
        }

    def test_flat_state_completed(self):
        self.assertTrue(state_mod.is_work_item_completed({"phase": "completed"}))

    def test_flat_state_not_completed(self):
        self.assertFalse(state_mod.is_work_item_completed({"phase": "coding"}))

    def test_nested_single_story_completed(self):
        st = self._nested("STORY-004-BE", {"STORY-004-BE": {"phase": "completed"}})
        self.assertTrue(state_mod.is_work_item_completed(st))

    def test_nested_single_story_not_completed(self):
        st = self._nested("STORY-004-BE", {"STORY-004-BE": {"phase": "coding"}})
        self.assertFalse(state_mod.is_work_item_completed(st))

    def test_nested_active_story_completed_but_sibling_still_active(self):
        """activeStory 指向的 Story 已完成，但另一个 Story 仍在跑 → 整体未完结。"""
        st = self._nested(
            "STORY-004-A",
            {
                "STORY-004-A": {"phase": "completed"},
                "STORY-004-B": {"phase": "coding"},
            },
        )
        self.assertFalse(state_mod.is_work_item_completed(st))

    def test_nested_all_stories_completed(self):
        st = self._nested(
            "STORY-004-A",
            {
                "STORY-004-A": {"phase": "completed"},
                "STORY-004-B": {"phase": "completed"},
            },
        )
        self.assertTrue(state_mod.is_work_item_completed(st))

    def test_nested_dr_scoped_stories_aggregated(self):
        """PRD 入口下 storyStates 嵌在 drStates[*] 内时也要聚合到位。"""
        st = {
            "version": "2",
            "stateModel": "nested",
            "entryNode": "PRD",
            "activeStory": "STORY-006-BE",
            "drStates": {
                "DR-005": {
                    "storyStates": {
                        "STORY-006-BE": {"phase": "completed"},
                        "STORY-007-BE": {"phase": "coding"},
                    }
                }
            },
        }
        self.assertFalse(state_mod.is_work_item_completed(st))

    def test_nested_no_story_records_falls_back_to_active_phase(self):
        """尚未拆出任何 Story（仅 prdState/drState）时回退 get_active_phase()。"""
        st = {
            "version": "2",
            "stateModel": "nested",
            "entryNode": "PRD",
            "prdState": {"phase": "completed"},
        }
        self.assertTrue(state_mod.is_work_item_completed(st))


class TestInitNestedStateWithUuid(unittest.TestCase):
    """🆕 v3.10.1 init_nested_state(state_uuid=...) 生成带 UUID 前缀的 stateMachineId。"""

    def test_state_uuid_prefixes_state_machine_id(self):
        """传 state_uuid 时 stateMachineId = {uuid}-{业务名}，另写 stateMachineName + stateUuid。"""
        uuid_str = "550e8400-e29b-41d4-a716-446655440000"
        st = state_mod.init_nested_state(
            project_key="life",
            entry_node="PRD",
            state_machine_id="PRD-IM-CS",
            state_machine_name="PRD-IM-CS",
            prd_id="PRD-001",
            state_uuid=uuid_str,
        )
        self.assertEqual(st["stateMachineId"], f"{uuid_str}-PRD-IM-CS")
        self.assertEqual(st["stateMachineName"], "PRD-IM-CS")
        self.assertEqual(st["stateUuid"], uuid_str)

    def test_no_state_uuid_backward_compat(self):
        """不传 state_uuid 时保持旧行为：stateMachineId=业务名，不写 stateUuid。"""
        st = state_mod.init_nested_state(
            project_key="life",
            entry_node="DR",
            state_machine_id="DR-CS",
            state_machine_name="DR-CS",
            dr_id="DR-005",
        )
        self.assertEqual(st["stateMachineId"], "DR-CS")
        # 旧行为：stateMachineName 等于传入的 state_machine_name
        self.assertEqual(st["stateMachineName"], "DR-CS")
        # 不写 stateUuid 字段
        self.assertNotIn("stateUuid", st)

    def test_state_uuid_with_story_entry(self):
        """STORY 入口带 state_uuid 也正确拼接。"""
        uuid_str = "abcdef12-3456-7890-abcd-ef1234567890"
        st = state_mod.init_nested_state(
            project_key="life",
            entry_node="STORY",
            state_machine_id="Story-003-004",
            state_machine_name="Story-003-004",
            story_ids=["STORY-003-BE", "STORY-004-BE"],
            state_uuid=uuid_str,
        )
        self.assertEqual(st["stateMachineId"], f"{uuid_str}-Story-003-004")
        self.assertEqual(st["stateMachineName"], "Story-003-004")
        self.assertEqual(st["stateUuid"], uuid_str)


class TestStoryDocumentBinding(unittest.TestCase):
    def test_nested_binding_updates_story_substate_and_is_idempotent(self):
        st = state_mod.init_nested_state(
            project_key="life",
            entry_node="STORY",
            state_machine_id="Story-006",
            state_machine_name="Story-006",
            story_ids=["STORY-006-BE"],
        )
        name = "cs-ai-story-006-门店推荐对接与列表接口-BE"
        path = r"D:\Item\life\document\cs-ai-story-006-BE.md"

        changed = state_mod.bind_story_document(
            st, "STORY-006-BE", story_name=name, doc_path=path
        )
        again = state_mod.bind_story_document(
            st, "STORY-006-BE", story_name=name, doc_path=path
        )

        self.assertTrue(changed)
        self.assertFalse(again)
        sub = st["storyStates"]["STORY-006-BE"]
        self.assertEqual(sub["storyName"], name)
        self.assertEqual(sub["docPath"], path)
        self.assertEqual(
            state_mod.get_story_document_binding(st, "STORY-006-BE"),
            {"storyName": name, "docPath": path},
        )

    def test_flat_binding_uses_compatible_top_level_fields(self):
        st = {
            "version": "1",
            "phase": "initialized",
            "currentStory": "STORY-006-BE",
            "history": [],
        }

        changed = state_mod.bind_story_document(
            st,
            "STORY-006-BE",
            story_name="cs-ai-story-006-title-BE.md",
            doc_path=r"D:\docs\cs-ai-story-006-title-BE.md",
        )

        self.assertTrue(changed)
        self.assertEqual(st["storyName"], "cs-ai-story-006-title-BE")
        self.assertEqual(st["storyDocPath"], r"D:\docs\cs-ai-story-006-title-BE.md")

    def test_nested_binding_rejects_unknown_story(self):
        st = state_mod.init_nested_state(
            project_key="life",
            entry_node="STORY",
            state_machine_id="Story-006",
            state_machine_name="Story-006",
            story_ids=["STORY-006-BE"],
        )
        with self.assertRaises(ValueError):
            state_mod.bind_story_document(
                st,
                "STORY-999-BE",
                story_name="story-999",
                doc_path=r"D:\docs\story-999.md",
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
