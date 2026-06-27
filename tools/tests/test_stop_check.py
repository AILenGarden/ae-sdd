"""
test_stop_check.py — stop_check 模块单元测试

覆盖：
- check_output 状态头校验基础行为
- 🆕 v3.5.4 HS-8：PRD compact 失败检测（卡在 awaiting_compact 无 summary.md）
"""
from __future__ import annotations

import sys
import json
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import stop_check


# ─── 辅助：构造 .ae-sdd/ 项目目录 ──────────────────────────────────────────────

def _make_ae_sdd_project(tmp_path: Path) -> Path:
    """在 tmp_path 下建 .ae-sdd/ 并返回 ade_sdd 路径。"""
    ade_sdd = tmp_path / ".ae-sdd"
    ade_sdd.mkdir(parents=True, exist_ok=True)
    return ade_sdd


def _make_prd_state(tmp_path: Path, prd_id: str, prd_status: str,
                    with_summary: bool = False) -> Path:
    """构造 PRD 级 state.json，prdStatus=prd_status，可选生成 summary.md。"""
    prd_dir = tmp_path / ".auto-engineering" / prd_id
    prd_dir.mkdir(parents=True, exist_ok=True)
    ps = {
        "prdId": prd_id,
        "prdStatus": prd_status,
        "storyIds": [],
        "compactHistory": [],
    }
    (prd_dir / "state.json").write_text(
        json.dumps(ps, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    if with_summary:
        (prd_dir / "summary.md").write_text("# summary\n", encoding="utf-8")
    return prd_dir


# ─── 基础行为：无 .ae-sdd/ 放行 ────────────────────────────────────────────────

class TestNonAeSddProject:
    def test_no_ae_sdd_dir_allows_stop(self, tmp_path):
        """非 ae-sdd 项目（无 .ae-sdd/）直接放行"""
        # ade_sdd=None 模拟无 .ae-sdd/
        allowed, msg = stop_check.check_output("some response", ade_sdd=None)
        assert allowed
        assert msg == ""


# ─── 🆕 v3.5.4 HS-8：compact 失败检测 ─────────────────────────────────────────

class TestHS8CompactFailure:
    """HS-8：PRD compact 卡在 awaiting_compact 无 summary.md → 阻断 + 报警"""

    def test_stuck_awaiting_compact_without_summary_blocks(self, tmp_path):
        """prdStatus=awaiting_compact 且无 summary.md → 阻断"""
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _make_prd_state(tmp_path, "PRD-CS-001", "awaiting_compact", with_summary=False)
        # 状态头存在的响应（触发 HS-8 检测路径）
        response = (
            "◆ STATE:  code-reviewed/STORY-001\n"
            "◆ GATE:   ✅ CLEAR\n"
            "◆ LAST:   完成收尾\n"
            "◆ NEXT:   无\n"
        )
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        assert not allowed, "awaiting_compact 无 summary 应被 HS-8 阻断"
        assert "HS-8" in msg
        assert "awaiting_compact" in msg
        assert "PRD-CS-001" in msg

    def test_compacted_status_allows(self, tmp_path):
        """prdStatus=compacted → 放行（compact 已成功）"""
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _make_prd_state(tmp_path, "PRD-CS-001", "compacted", with_summary=True)
        response = (
            "◆ STATE:  completed/STORY-001\n"
            "◆ GATE:   ✅ CLEAR\n"
            "◆ LAST:   完成\n"
            "◆ NEXT:   无\n"
        )
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        # compacted 不触发 HS-8；可能因重试上限放行，但 msg 不应含 HS-8
        if not allowed:
            assert "HS-8" not in msg, "compacted 状态不应触发 HS-8"

    def test_awaiting_compact_with_summary_allows(self, tmp_path):
        """prdStatus=awaiting_compact 但有 summary.md → 放行（compact 实质完成，状态待刷）"""
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _make_prd_state(tmp_path, "PRD-CS-001", "awaiting_compact", with_summary=True)
        response = (
            "◆ STATE:  completed/STORY-001\n"
            "◆ GATE:   ✅ CLEAR\n"
            "◆ LAST:   完成\n"
            "◆ NEXT:   无\n"
        )
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        if not allowed:
            assert "HS-8" not in msg, "有 summary.md 不应触发 HS-8"

    def test_in_progress_status_allows(self, tmp_path):
        """prdStatus=in_progress → 放行（未进入 compact 流程）"""
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _make_prd_state(tmp_path, "PRD-CS-001", "in_progress", with_summary=False)
        response = (
            "◆ STATE:  coding/STORY-001\n"
            "◆ GATE:   ✅ CLEAR\n"
            "◆ LAST:   编码\n"
            "◆ NEXT:   测试\n"
        )
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        if not allowed:
            assert "HS-8" not in msg, "in_progress 不应触发 HS-8"

    def test_multiple_stuck_prds_all_listed(self, tmp_path):
        """多个 PRD 卡住 → 报警列全部"""
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _make_prd_state(tmp_path, "PRD-CS-001", "awaiting_compact", with_summary=False)
        _make_prd_state(tmp_path, "PRD-IM-002", "awaiting_compact", with_summary=False)
        response = (
            "◆ STATE:  code-reviewed/STORY-001\n"
            "◆ GATE:   ✅ CLEAR\n"
            "◆ LAST:   收尾\n"
            "◆ NEXT:   无\n"
        )
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        assert not allowed
        assert "PRD-CS-001" in msg
        assert "PRD-IM-002" in msg

    def test_no_auto_engineering_dir_allows(self, tmp_path):
        """无 .auto-engineering/ → 放行（无 PRD 级 state）"""
        ade_sdd = _make_ae_sdd_project(tmp_path)
        response = (
            "◆ STATE:  coding/STORY-001\n"
            "◆ GATE:   ✅ CLEAR\n"
            "◆ LAST:   编码\n"
            "◆ NEXT:   测试\n"
        )
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        if not allowed:
            assert "HS-8" not in msg
