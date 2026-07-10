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
        """大链 initialized → ra-generated"""
        s = {"phase": "initialized", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "ra-generated")

    def test_micro_initialized_to_task_generated(self):
        """微链 initialized → task-generated（BUG/调整从 Task 系列入）"""
        s = {"phase": "initialized", "scale": "微"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "task-generated")
        self.assertIn("task-generate-skill.md", sug["skill"])

    def test_small_initialized_to_story_generated(self):
        """小链 initialized → story-generated（已有Story，从Story系列入）"""
        s = {"phase": "initialized", "scale": "小"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "story-generated")

    def test_story_reviewed_to_testcase_generated(self):
        """🆕 v3.7.0 大/中/小链 story-reviewed → testcase-generated（TestCase 独立系列）"""
        for scale in ("大", "中", "小"):
            s = {"phase": "story-reviewed", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "testcase-generated", f"scale={scale}")
            self.assertIn("testcase-generate-skill.md", sug["skill"])

    def test_testcase_generated_to_testcase_reviewed(self):
        """🆕 v3.7.0 testcase-generated → testcase-reviewed"""
        for scale in ("大", "中", "小"):
            s = {"phase": "testcase-generated", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "testcase-reviewed", f"scale={scale}")
            self.assertIn("testcase-review-skill.md", sug["skill"])

    def test_testcase_reviewed_to_task_generated(self):
        """🆕 v3.7.0 testcase-reviewed → task-generated"""
        for scale in ("大", "中", "小"):
            s = {"phase": "testcase-reviewed", "scale": scale}
            sug = state_mod.next_step_suggestion(s)
            self.assertEqual(sug["next"], "task-generated", f"scale={scale}")

    def test_medium_initialized_to_dr_generated(self):
        """中链 initialized → dr-generated（已有DR，从DR系列入，跳RA）"""
        s = {"phase": "initialized", "scale": "中"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "dr-generated")

    def test_large_ra_to_dr(self):
        """大链 ra-generated → dr-generated"""
        s = {"phase": "ra-generated", "scale": "大"}
        sug = state_mod.next_step_suggestion(s)
        self.assertEqual(sug["next"], "dr-generated")

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

    def test_large_chain_has_14_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["大"]), 14)

    def test_medium_chain_has_13_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["中"]), 13)

    def test_small_chain_has_12_phases(self):
        self.assertEqual(len(state_mod.PHASE_FLOWS["小"]), 12)

    def test_micro_chain_has_8_phases(self):
        # 🆕 2026-07-03(B1): 微链从 7 phase → 8 phase，加回 code-reviewed。
        # 设计文档 conventions.md §3.1 明确"出 CodeReview 报告 ❌不豁免"，
        # 此前微链物理跳过 code-reviewed 导致 gate_intercept 门禁不可达。
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

    def test_infer_small_from_task_step(self):
        s = {"completedSteps": ["step-1-task"], "phase": "task-reviewed"}
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
        """旧 state 无 scale → _resolve_scale 反推后 set_phase 正常工作"""
        s = {"phase": "initialized", "history": []}  # 无 scale 字段
        # 反推：无 completedSteps + initialized → 默认大链
        # initialized 在大链内，合法
        state_mod.set_phase(s, "ra-generated")
        self.assertEqual(s["phase"], "ra-generated")
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

    def test_mavis_generates_summary_and_status(self):
        """mavis runtime：生成 summary.md + prdStatus → awaiting_compact，无 trigger"""
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ps = self._make_prd_state()
            r = state_mod.prd_complete(ps, "PRD-CS-001", "mavis", root)
            self.assertTrue(r["summaryPath"].endswith("summary.md"))
            self.assertFalse(r["compactTrigger"])
            self.assertIn("mavis session rotate", r["runtimeHint"])
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
            r = state_mod.prd_complete(ps, "PRD-CS-001", "mavis", root)
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
            state_mod.prd_complete(ps, "PRD-CS-001", "mavis", root)
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
