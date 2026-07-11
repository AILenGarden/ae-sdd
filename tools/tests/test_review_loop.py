"""
test_review_loop.py — review-loop 编排层状态机测试（🆕 v3.5.12 第 1 波）

覆盖 5 个核心函数 + 端到端状态机闭环。
"""
import sys
import tempfile
from pathlib import Path

TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import review_loop as rl
from lib import state as st


# ─── Tier 机械派生 ─────────────────────────────────────────────────────────

def test_tier1_micro_no_decision():
    r = rl.derive_tier("微", "")
    assert r.tier == 1, f"微规模无决策应 Tier 1，实 {r.tier}"

def test_tier1_small_no_decision():
    r = rl.derive_tier("小", "改个枚举值")
    assert r.tier == 1

def test_tier2_medium():
    r = rl.derive_tier("中", "")
    assert r.tier == 2, f"中规模应 Tier 2，实 {r.tier}"

def test_tier2_with_single_decision():
    r = rl.derive_tier("小", "涉及状态机流转")
    assert r.tier == 2, f"含状态机应 Tier 2，实 {r.tier}"
    assert "state-machine" in r.key_decisions

def test_tier3_large():
    r = rl.derive_tier("大", "")
    assert r.tier == 3, f"大规模应 Tier 3，实 {r.tier}"

def test_tier3_high_risk_multi_decision():
    r = rl.derive_tier("中", "涉及资金支付和状态机")
    assert r.tier == 3, f"含2+高危决策应 Tier 3，实 {r.tier}"

def test_tier_idempotent():
    """相同输入恒定输出相同 tier（幂等）。"""
    r1 = rl.derive_tier("中", "涉及事务")
    r2 = rl.derive_tier("中", "涉及事务")
    assert r1.tier == r2.tier


# ─── 锚点去重新增判定 ───────────────────────────────────────────────────────

def test_anchor_validation():
    assert rl.validate_anchor("DR§6.5 R11") is True
    assert rl.validate_anchor("FILE:Service.java:42") is True
    assert rl.validate_anchor("FIELD:userName") is True
    assert rl.validate_anchor("API:/user/login") is True
    assert rl.validate_anchor("裸结论无前缀") is False
    assert rl.validate_anchor("") is False

def test_compute_new_findings_dedup():
    historical = {"DR§6.5 R11", "FILE:S.java:1"}
    current = [
        {"id": "F1", "anchor": "DR§6.5 R11", "severity": "O"},  # 重复
        {"id": "F2", "anchor": "FILE:New.java:5", "severity": "Y"},  # 新
        {"id": "F3", "anchor": "裸结论", "severity": "Y"},  # 格式非法
    ]
    new, rejected = rl.compute_new_findings(current, historical)
    assert len(new) == 1, f"应1个新增，实 {len(new)}"
    assert new[0]["id"] == "F2"
    assert len(rejected) == 1
    assert rejected[0]["id"] == "F3"


# ─── session 独立性（G-09B）─────────────────────────────────────────────────

def test_session_independence_pass():
    r = rl.check_session_independence(["sid-A", "sid-B"], "root-sid", 2)
    assert r.passed is True

def test_session_self_impersonation_blocked():
    """root 自扮 reviewer（同 session）→ 阻断。"""
    r = rl.check_session_independence(["root-sid", "sid-B"], "root-sid", 2)
    assert r.passed is False
    assert any("自扮" in v for v in r.violations)

def test_session_insufficient_reviewers():
    """Tier 2 只给 1 个 reviewer → 阻断。"""
    r = rl.check_session_independence(["sid-A"], "root-sid", 2)
    assert r.passed is False
    assert any("< Tier" in v for v in r.violations)

def test_session_tier1_single_ok():
    r = rl.check_session_independence(["sid-A"], "root-sid", 1)
    assert r.passed is True


# ─── 推进轮次（dryCounter + 退出判定）────────────────────────────────────────

def test_advance_round_new_resets_dry():
    rs = {"round": 1, "dryCounter": 1, "findings": []}
    current = [{"id": "F1", "anchor": "DR§6.5 New", "severity": "O"}]
    r = rl.advance_round(rs, current)
    assert r.round == 2
    assert r.dry_counter == 0, "有新 finding 应归零"
    assert len(r.new_findings) == 1

def test_advance_round_no_new_increments_dry():
    rs = {"round": 1, "dryCounter": 0, "findings": [{"anchor": "DR§6.5 R11"}]}
    current = [{"id": "F1", "anchor": "DR§6.5 R11", "severity": "O"}]  # 重复
    r = rl.advance_round(rs, current)
    assert r.dry_counter == 1, "无新增应 +1"

def test_advance_round_exit_normal_at_2():
    rs = {"round": 1, "dryCounter": 1, "findings": []}
    r = rl.advance_round(rs, [])  # 无新增
    assert r.dry_counter == 2
    assert r.exit_reason == "normal"
    assert r.next_action == "exit-normal"

def test_advance_round_escalate_on_red_after_max():
    rs = {"round": 3, "dryCounter": 0, "findings": []}
    current = [{"id": "F1", "anchor": "DR§6.5 New", "severity": "O"}]
    r = rl.advance_round(rs, current, has_red_blocker=True)
    # round=4 > MAX_ROUNDS(3) 且有 red → escalate（注意：有新增归零dry，但round超限+red→escalate）
    assert r.round == 4
    assert r.exit_reason == "escalate"


# ─── verify_exit ────────────────────────────────────────────────────────────

def test_verify_exit_normal():
    passed, _ = rl.verify_exit({"exitReason": "normal", "dryCounter": 2, "round": 5})
    assert passed is True

def test_verify_exit_inconsistent():
    """exitReason=normal 但 dryCounter<3 → 数据不一致，阻断。"""
    passed, _ = rl.verify_exit({"exitReason": "normal", "dryCounter": 2, "round": 5})
    assert passed is False

def test_verify_exit_escalate():
    passed, _ = rl.verify_exit({"exitReason": "escalate", "round": 4})
    assert passed is True

def test_verify_exit_not_ready():
    passed, _ = rl.verify_exit({"exitReason": None, "dryCounter": 1, "round": 2})
    assert passed is False


# ─── 端到端状态机（start → collect 多轮 → verify-exit）──────────────────────

def test_e2e_full_loop(tmp_path=None):
    """完整跑一遍：start → 1轮有新增 → 3轮无新增退出。"""
    import tempfile
    td = tempfile.TemporaryDirectory()
    try:
        sp = Path(td.name) / "state.json"

        # start
        s = st.read_state(sp)
        r = rl.start(s, "story-review", "中", "涉及事务")
        st.write_state(sp, s)
        assert r["tier"] == 2

        # r1: 有新增
        s = st.read_state(sp)
        reports = [
            {"sessionId": "sid-A", "report": "r1.md",
             "findings": [{"id": "F1", "anchor": "DR§6.5 R11", "severity": "O"}]},
            {"sessionId": "sid-B", "report": "r2.md",
             "findings": [{"id": "F2", "anchor": "FILE:S.java:1", "severity": "Y"}]},
        ]
        r = rl.collect(s, "story-review", reports, "root-sid", has_red_blocker=True)
        st.write_state(sp, s)
        assert r["round"] == 1 and r["dryCounter"] == 0 and len(r["newFindings"]) == 2

        # r2-4: 无新增，dryCounter 1→2→3 退出
        for rnd in [2, 3, 4]:
            s = st.read_state(sp)
            reports_ok = [
                {"sessionId": f"sid-A{rnd}", "report": f"r{rnd}.md", "findings": []},
                {"sessionId": f"sid-B{rnd}", "report": f"r{rnd}b.md", "findings": []},
            ]
            r = rl.collect(s, "story-review", reports_ok, "root-sid")
            st.write_state(sp, s)

        assert r["round"] == 3 and r["dryCounter"] == 2
        assert r["exitReason"] == "normal"

        # verify-exit
        s = st.read_state(sp)
        passed, _ = rl.verify_exit(s.get("reviewLoop") or {})
        assert passed is True
    finally:
        td.cleanup()


def test_e2e_self_impersonation_blocks():
    """root 自扮 reviewer → collect 阻断，不推进。"""
    import tempfile
    td = tempfile.TemporaryDirectory()
    try:
        sp = Path(td.name) / "state.json"
        s = st.read_state(sp)
        rl.start(s, "story-review", "中", "事务")
        st.write_state(sp, s)

        s = st.read_state(sp)
        r = rl.collect(s, "story-review",
                        [{"sessionId": "root-sid", "report": "r.md", "findings": []}],
                        "root-sid")
        assert r["nextAction"] == "blocked-session-not-independent"
        assert r["sessionCheck"]["passed"] is False
        assert r["round"] == 0  # 没推进
    finally:
        td.cleanup()


# ─── state.py 写入 helper（多 Agent + 重入）──────────────────────────────────

def test_register_and_complete_agent():
    s = {}
    st.register_agent(s, "sub-A", "story-reviewer", "sid-A", "生成前端契约")
    assert len(s["activeAgents"]) == 1
    assert s["activeAgents"][0]["role"] == "story-reviewer"
    # 幂等
    st.register_agent(s, "sub-A", "story-reviewer", "sid-A")
    assert len(s["activeAgents"]) == 1
    # 完成
    st.complete_agent(s, "sub-A", "r1.md", "完成")
    assert len(s["activeAgents"]) == 0
    assert len(s["agentReports"]) == 1
    assert s["agentReports"][0]["reportPath"] == "r1.md"

def test_set_current_step_accumulates():
    s = {}
    st.set_current_step(s, "step-1")
    assert s["currentStep"] == "step-1"
    st.set_current_step(s, "step-2")
    assert s["currentStep"] == "step-2"
    assert "step-1" in s["completedSteps"]

def test_bump_coding_round():
    s = {}
    r1 = st.bump_coding_round(s)
    assert r1 == "r1" and s["codingRound"] == 1
    r2 = st.bump_coding_round(s)
    assert r2 == "r2" and s["codingRound"] == 2


if __name__ == "__main__":
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_") and callable(v)]
    passed = failed = 0
    for t in tests:
        try:
            t()
            print(f"  ✅ {t.__name__}")
            passed += 1
        except AssertionError as e:
            print(f"  ❌ {t.__name__}: {e}")
            failed += 1
        except Exception as e:
            print(f"  💥 {t.__name__}: {type(e).__name__}: {e}")
            failed += 1
    print(f"\n{passed} passed, {failed} failed, {len(tests)} total")
    sys.exit(1 if failed else 0)
