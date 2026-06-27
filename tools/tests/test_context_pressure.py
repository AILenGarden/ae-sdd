"""
test_context_pressure.py — context_pressure 模块单元测试（🆕 v3.5.5）

覆盖：
- 5 档评级（low / medium / high / critical）
- 单信号触发 vs 多信号 OR 取高
- config.yaml override 合并
- 无 .ae-sdd/ 项目返回 low（不报错）
- 落盘文档字节扫描准确性
- critical 时 suggestions 填充
- to_dict 结构完整性
"""
from __future__ import annotations

import sys
import json
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import context_pressure as cp


# ─── 辅助：构造 .ae-sdd/ 项目目录 + 状态文件 ──────────────────────────────────

def _make_project(tmp_path: Path) -> Path:
    """建 .ae-sdd/ + config.yaml，返回 ade_sdd 路径。"""
    ade_sdd = tmp_path / ".ae-sdd"
    ade_sdd.mkdir(parents=True, exist_ok=True)
    (ade_sdd / "config.yaml").write_text("# config\nprojectKey: test\n", encoding="utf-8")
    return ade_sdd


def _write_session(ade_sdd: Path, story_id: str, confirmed: int) -> None:
    """写 session.json 含 N 条 userConfirmedPhases。"""
    sess = {
        "sessionId": "test-sess",
        "projectKey": "test",
        "storyId": story_id,
        "entryPhase": "initialized",
        "userConfirmedPhases": [
            {"phase": f"phase-{i}", "confirmedAt": "2026-06-27T10:00:00Z", "confirmedBy": "user"}
            for i in range(confirmed)
        ],
    }
    sp = ade_sdd if not story_id else (ade_sdd.parent / ".auto-engineering" / story_id)
    sp.mkdir(parents=True, exist_ok=True)
    (sp / "session.json").write_text(json.dumps(sess, ensure_ascii=False, indent=2), encoding="utf-8")


def _write_state(ade_sdd: Path, story_id: str, *, events: int = 0,
                 history: int = 0, active_agents: int = 0) -> None:
    """写 state.json 含指定 events/history/activeAgents 数。"""
    sp = ade_sdd if not story_id else (ade_sdd.parent / ".auto-engineering" / story_id)
    sp.mkdir(parents=True, exist_ok=True)
    state = {
        "version": "1",
        "projectKey": "test",
        "phase": "initialized",
        "events": [{"seq": i} for i in range(events)],
        "history": [{"phase": "x"} for _ in range(history)],
        "activeAgents": [{"agentId": f"a-{i}"} for i in range(active_agents)],
    }
    (sp / "state.json").write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


# ─── 基础：5 档评级 ──────────────────────────────────────────────────────────

class TestLevels:
    def test_low_default(self, tmp_path):
        """空项目（无 state/session/doc）→ low"""
        ade_sdd = _make_project(tmp_path)
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "low"
        assert report.suggestions == []
        assert report.signals["confirmedPhases"] == 0

    def test_medium_from_doc_bytes(self, tmp_path):
        """docBytes ≥ 500KB → medium"""
        ade_sdd = _make_project(tmp_path)
        # 写 600KB 文件
        (ade_sdd / "big.bin").write_bytes(b"x" * (600 * 1024))
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "medium"
        assert any(t.signal == "docBytes" for t in report.triggered)

    def test_high_from_events(self, tmp_path):
        """events ≥ 200 → high"""
        ade_sdd = _make_project(tmp_path)
        _write_state(ade_sdd, "", events=250)
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "high"
        assert any(t.signal == "events" and t.threshold == "high" for t in report.triggered)

    def test_critical_from_confirmed_phases(self, tmp_path):
        """confirmedPhases ≥ 5 → critical + suggestions"""
        ade_sdd = _make_project(tmp_path)
        _write_session(ade_sdd, "", confirmed=6)
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "critical"
        assert len(report.suggestions) >= 1
        assert any("PRD" in s for s in report.suggestions)

    def test_critical_from_history(self, tmp_path):
        """historyLen ≥ 10 → critical"""
        ade_sdd = _make_project(tmp_path)
        _write_state(ade_sdd, "", history=12)
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "critical"


# ─── OR 触发：取最高档 ──────────────────────────────────────────────────────

class TestOrTrigger:
    def test_multiple_signals_take_max(self, tmp_path):
        """medium 信号 + high 信号 → overall high（OR 取最高）"""
        ade_sdd = _make_project(tmp_path)
        # docBytes=600KB (medium) + events=250 (high)
        (ade_sdd / "m.bin").write_bytes(b"x" * (600 * 1024))
        _write_state(ade_sdd, "", events=250)
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "high"

    def test_critical_dominates(self, tmp_path):
        """medium + critical → critical"""
        ade_sdd = _make_project(tmp_path)
        _write_session(ade_sdd, "", confirmed=5)  # critical
        _write_state(ade_sdd, "", events=150)     # medium
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "critical"


# ─── Override 合并 ──────────────────────────────────────────────────────────

class TestConfigOverride:
    def test_override_lowers_threshold(self, tmp_path):
        """config.yaml 调低 medium 阈值 → 同样的信号触发 medium"""
        ade_sdd = _make_project(tmp_path)
        # 调低 docBytes medium 阈值到 100KB
        (ade_sdd / "config.yaml").write_text(
            "contextPressure:\n  thresholds:\n    medium:\n      docBytes: 102400\n",
            encoding="utf-8",
        )
        (ade_sdd / "small.bin").write_bytes(b"x" * (200 * 1024))  # 200KB > 100KB
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "medium"
        # 实际生效的阈值是 override 后的
        assert report.thresholds["medium"]["docBytes"] == 102400

    def test_invalid_override_keeps_default(self, tmp_path):
        """config.yaml 字段非法 → 保留缺省，不抛异常"""
        ade_sdd = _make_project(tmp_path)
        (ade_sdd / "config.yaml").write_text(
            "contextPressure:\n  thresholds:\n    medium:\n      docBytes: 'not-a-number'\n",
            encoding="utf-8",
        )
        report = cp.run_all(ade_sdd, "")
        # 缺省 docBytes medium 阈值 = 500_000
        assert report.thresholds["medium"]["docBytes"] == 500_000


# ─── 无项目上下文 ──────────────────────────────────────────────────────────

class TestNoProject:
    def test_no_ae_sdd_returns_low(self):
        """ade_sdd=None → low，不报错"""
        report = cp.run_all(None, "")
        assert report.pressure == "low"
        assert report.signals["docBytes"] == 0

    def test_missing_state_json_returns_low(self, tmp_path):
        """只有 .ae-sdd/ 无 state.json → low（默认值）"""
        ade_sdd = _make_project(tmp_path)
        report = cp.run_all(ade_sdd, "")
        assert report.pressure == "low"


# ─── 落盘文档扫描 ──────────────────────────────────────────────────────────

class TestDocBytesScan:
    def test_scan_includes_ade_sdd(self, tmp_path):
        """扫描 .ae-sdd/ 下文件"""
        ade_sdd = _make_project(tmp_path)
        (ade_sdd / "big.bin").write_bytes(b"x" * (600 * 1024))
        report = cp.run_all(ade_sdd, "")
        assert report.signals["docBytes"] >= 600 * 1024

    def test_scan_with_story_id_includes_auto_engineering(self, tmp_path):
        """--story 时扫描 .auto-engineering/{story}/ + design/ + task/"""
        ade_sdd = _make_project(tmp_path)
        proj_root = tmp_path
        # 写 Story 级产物
        story_dir = proj_root / ".auto-engineering" / "STORY-001"
        story_dir.mkdir(parents=True, exist_ok=True)
        (story_dir / "doc.md").write_text("x" * (300 * 1024), encoding="utf-8")
        report = cp.run_all(ade_sdd, "STORY-001")
        # design/ + task/ 不存在但不影响
        assert report.signals["docBytes"] >= 300 * 1024


# ─── 输出结构 ──────────────────────────────────────────────────────────────

class TestOutputStructure:
    def test_to_dict_keys(self, tmp_path):
        ade_sdd = _make_project(tmp_path)
        report = cp.run_all(ade_sdd, "")
        d = report.to_dict()
        for key in ("pressure", "signals", "triggeredSignals", "thresholds",
                    "suggestions", "nextAction"):
            assert key in d, f"missing key: {key}"
        assert d["nextAction"] == "context-pressure is informational only; no action required"

    def test_triggered_signals_have_signal_value_threshold_limit(self, tmp_path):
        ade_sdd = _make_project(tmp_path)
        _write_session(ade_sdd, "", confirmed=5)
        report = cp.run_all(ade_sdd, "")
        d = report.to_dict()
        assert len(d["triggeredSignals"]) >= 1
        t = d["triggeredSignals"][0]
        for key in ("signal", "value", "threshold", "limit"):
            assert key in t, f"missing triggered key: {key}"