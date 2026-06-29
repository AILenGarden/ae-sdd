"""
test_alignment_audit.py — AA 全维对齐验证器测试（🆕 v3.5.11）

覆盖 UC-08~UC-12 五个维度，每个维度至少：
  - 正例（对齐通过）
  - 反例（检测到 gap）
  - 边界（空文件 / 缺文件 / 已标注未来命令）
"""
import sys
import tempfile
from pathlib import Path

# 让 tests/ 能 import tools/lib
TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import alignment_audit as aa
from lib.alignment_audit import (
    check_uc08_gate_claim_alignment,
    check_uc09_gate_impl_authenticity,
    check_uc10_state_field_liveness,
    check_uc11_state_machine_closure,
    check_uc12_ghost_command_capture,
)
from lib.update_graph import UpdateCheckResult


def _make_repo() -> tempfile.TemporaryDirectory:
    """建一个最小仓库骨架供测试。"""
    td = tempfile.TemporaryDirectory()
    root = Path(td.name)
    (root / "source" / "skills" / "cross-cutting").mkdir(parents=True)
    (root / "tools" / "lib").mkdir(parents=True)
    (root / "tools" / "bin").mkdir(parents=True)
    (root / "scripts").mkdir(parents=True)
    return td


# ─── UC-08 门禁承诺↔注册 ──────────────────────────────────────────────────

def test_uc08_aligned_pass():
    """承诺绑定已对齐 G-XX id → 通过。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        (root / "source" / "SKILL.md").write_text(
            "## G-00 项目资产门卫（🔴 硬门禁）\n", encoding="utf-8")
        r = check_uc08_gate_claim_alignment(root)
        assert r.pass_ is True, f"应通过但报：{r.message}"
    finally:
        td.cleanup()


def test_uc08_orphan_claim_warn():
    """承诺硬门禁但无 G-XX 绑定 → warn（软门禁降级档）。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        (root / "source" / "SKILL.md").write_text(
            "## 第七步 7 道闸（🔴 硬门禁，全部阻断）\n", encoding="utf-8")
        r = check_uc08_gate_claim_alignment(root)
        assert r.pass_ is True and r.severity == "warn", \
            f"应 warn 通过，实 severity={r.severity} pass={r.pass_}：{r.message}"
        assert r.details["orphan_count"] >= 1
    finally:
        td.cleanup()


# ─── UC-09 门禁实现真实性 ─────────────────────────────────────────────────

def test_uc09_stub_pass_detected():
    """G-RA-FLOW-VIOLATION 走 CHECK_FUNCS（无特判）+ _sys NameError → error。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        gates_py = root / "tools" / "lib" / "gates.py"
        # 模拟当前 buggy 状态：注册了 + 走 CHECK_FUNCS + 用 _sys 但只 import sys
        gates_py.write_text(
            "import sys\n"
            'GATE_REGISTRY = [{"id": "G-RA-FLOW-VIOLATION", "name": "x"}]\n'
            "CHECK_FUNCS = {'G-RA-FLOW-VIOLATION': check_ra_flow_violation}\n"
            "def check_ra_flow_violation(*a, **k):\n"
            "    return _sys.executable\n"
            "# check_all 走 CHECK_FUNCS，无 G-RA-FLOW-VIOLATION 特判分支\n",
            encoding="utf-8")
        r = check_uc09_gate_impl_authenticity(root)
        assert r.pass_ is False and r.severity == "error", \
            f"应 error 阻断，实 severity={r.severity} pass={r.pass_}：{r.message}"
        assert len(r.details["stubs"]) >= 1
    finally:
        td.cleanup()


def test_uc09_authentic_pass():
    """无 stub-pass 假门禁 → 通过。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        gates_py = root / "tools" / "lib" / "gates.py"
        gates_py.write_text(
            "import sys\n"
            'GATE_REGISTRY = [{"id": "G-01", "name": "x"}]\n'
            "def check_g01(*a, **k):\n"
            "    return None\n",
            encoding="utf-8")
        r = check_uc09_gate_impl_authenticity(root)
        assert r.pass_ is True
    finally:
        td.cleanup()


# ─── UC-10 state 字段存活性 ───────────────────────────────────────────────

def test_uc10_dead_field_warn():
    """字段在 doc schema 但 tools/ 无写入方 → warn。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        # tools/lib 里完全没有 reviewLoop/dryCounter 写入 → 应报
        (root / "tools" / "lib" / "state.py").write_text(
            "def set_phase(s, p): s['phase'] = p\n", encoding="utf-8")
        r = check_uc10_state_field_liveness(root)
        assert r.pass_ is True and r.severity == "warn", \
            f"应 warn，实 severity={r.severity}：{r.message}"
        assert r.details["dead_count"] >= 1
        dead_fields = {d["field"] for d in r.details["fields"]}
        assert "dryCounter" in dead_fields, "dryCounter 应被判为死字段"
    finally:
        td.cleanup()


def test_uc10_live_field_pass():
    """字段有写入方 → 不报。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        # 给每个 probe 字段都加上写入代码（匹配探针正则的写入形态）
        live_code = (
            "s['activeAgents'] = []\n"
            "s['agentReports'] = []\n"
            "s['reviewLoop'] = {}\n"
            "s['dryCounter'] = 0\n"
            "s['codingRound'] = 'r1'\n"
            "s.setdefault('completedSteps', []).append('x')\n"
            "s['currentStep'] = 'x'\n"
            "s['prdId'] = 'p'\n"
            "s['crossStoryDeps'] = []\n"
            "s['crossStoryResidualRisks'] = []\n"
            "s['prdReview'] = {}\n"
            "s['gateRegistry'] = {}\n"
            "s.setdefault('compactHistory', []).append({})\n"
            "s['sizeBudget'] = {}\n"
            "s['storyIds'] = []\n"
        )
        (root / "tools" / "lib" / "state.py").write_text(live_code, encoding="utf-8")
        r = check_uc10_state_field_liveness(root)
        assert r.pass_ is True, f"应通过：{r.message}"
        assert r.severity != "warn" or r.details.get("dead_count", 0) == 0
    finally:
        td.cleanup()


# ─── UC-11 状态机闭环 ─────────────────────────────────────────────────────

def test_uc11_open_state_machine_warn():
    """承诺连续3轮但无 dryCounter 持久化 → warn。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        (root / "tools" / "lib" / "state.py").write_text(
            "def set_phase(s, p): s['phase'] = p\n", encoding="utf-8")
        r = check_uc11_state_machine_closure(root)
        assert r.pass_ is True and r.severity == "warn", \
            f"应 warn：{r.message}"
        semantics = {s["semantic"] for s in r.details["items"]}
        assert "连续 3 轮无新增" in semantics, "应检出 review-loop 状态机未闭环"
    finally:
        td.cleanup()


def test_uc11_closed_pass():
    """所有状态机都有持久化 → 通过。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        (root / "tools" / "lib" / "state.py").write_text(
            "dryCounter = 0\n"
            "max_rounds = 3\n"
            "reviewerTier = 1\n"
            'prdStatus = "compacted"\n',
            encoding="utf-8")
        r = check_uc11_state_machine_closure(root)
        assert r.pass_ is True
    finally:
        td.cleanup()


# ─── UC-12 幽灵命令全捕获 ─────────────────────────────────────────────────

def test_uc12_real_ghost_error():
    """引用非 historical 的幽灵命令且无标注 → error。

    注：`run` 本身在 HISTORICAL_UNIMPLEMENTED，`ae-sdd run dr-review-skill`
    会被自动降级为 warn（run 是已知历史命令）。真正的「子命令词幽灵」
    （如 run 后的 dr-review-skill）靠第 2 波 2b 当场删除引用处置，不靠 UC-12 自动抓。
    本测试用非 historical 的纯幽灵命令验证 UC-12 核心能力。
    """
    td = _make_repo()
    try:
        root = Path(td.name)
        # CLI 只注册了 version，没 ghost-review
        (root / "tools" / "bin" / "ae-sdd").write_text(
            'sub.add_parser("version")\n', encoding="utf-8")
        (root / "source" / "SKILL.md").write_text(
            "下游重审：`ae-sdd ghost-review --dr xxx`\n", encoding="utf-8")
        r = check_uc12_ghost_command_capture(root)
        assert r.pass_ is False and r.severity == "error", \
            f"应 error：{r.message}"
        cmds = {g["cmd"] for g in r.details["real_ghosts"]}
        assert "ghost-review" in cmds, "应捕获非 historical 幽灵命令"
    finally:
        td.cleanup()


def test_uc12_historical_future_command_pass():
    """sync-tools 标注未来命令 → warn 通过（不 error）。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        (root / "tools" / "bin" / "ae-sdd").write_text(
            'sub.add_parser("version")\n', encoding="utf-8")
        # sync-tools 在 HISTORICAL_UNIMPLEMENTED，自动降级
        (root / "source" / "SKILL.md").write_text(
            "用 `ae-sdd sync-tools` 同步\n", encoding="utf-8")
        r = check_uc12_ghost_command_capture(root)
        # sync-tools 在 HISTORICAL_UNIMPLEMENTED → 不进 ghosts
        assert r.pass_ is True
    finally:
        td.cleanup()


def test_uc12_implemented_pass():
    """引用的命令都注册了 → 通过。"""
    td = _make_repo()
    try:
        root = Path(td.name)
        (root / "tools" / "bin" / "ae-sdd").write_text(
            'sub.add_parser("gates")\n'
            'gates_sub.add_parser("check")\n', encoding="utf-8")
        (root / "source" / "SKILL.md").write_text(
            "跑 `ae-sdd gates check`\n", encoding="utf-8")
        r = check_uc12_ghost_command_capture(root)
        assert r.pass_ is True, f"应通过：{r.message}"
    finally:
        td.cleanup()


# ─── 注册集成测试 ─────────────────────────────────────────────────────────

def test_register_to_update_graph():
    """AA 5 维度注册到 update_graph.CHECK_FUNCS。"""
    aa.register_to_update_graph()
    from lib import update_graph as ug
    for cid in ("UC-08", "UC-09", "UC-10", "UC-11", "UC-12"):
        assert cid in ug.CHECK_FUNCS, f"{cid} 应注册到 CHECK_FUNCS"


if __name__ == "__main__":
    # 简单 runner，无 pytest 也能跑
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
