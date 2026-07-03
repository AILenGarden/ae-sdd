"""
test_stop_check.py — stop_check 模块单元测试

覆盖：
- check_output 状态头校验基础行为
- 🆕 v3.5.4 HS-8：PRD compact 失败检测（卡在 awaiting_compact 无 summary.md）
"""
from __future__ import annotations

import sys
import json
import subprocess
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import stop_check

REPO_ROOT = Path(__file__).resolve().parents[2]
CLI_PATH = REPO_ROOT / "tools" / "bin" / "ae-sdd"


# ─── 辅助：构造 .ae-sdd/ 项目目录 ──────────────────────────────────────────────

def _make_ae_sdd_project(tmp_path: Path, config_text: str | None = None) -> Path:
    """在 tmp_path 下建 .ae-sdd/ 并返回 ade_sdd 路径。"""
    ade_sdd = tmp_path / ".ae-sdd"
    ade_sdd.mkdir(parents=True, exist_ok=True)
    (ade_sdd / "config.yaml").write_text(
        config_text or "projectKey: test-proj\n",
        encoding="utf-8",
    )
    return ade_sdd


def _write_state(ade_sdd: Path, phase: str, story: str = "STORY-001") -> None:
    (ade_sdd / "state.json").write_text(
        json.dumps({
            "version": "1",
            "projectKey": "test-proj",
            "phase": phase,
            "currentStory": story,
        }, ensure_ascii=False),
        encoding="utf-8",
    )


def _run_stop_check_cli(project_dir: Path, payload: dict) -> dict:
    proc = subprocess.run(
        [sys.executable, str(CLI_PATH), "stop-check"],
        input=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=str(project_dir),
        check=False,
    )
    assert proc.returncode == 0, proc.stderr.decode("utf-8", errors="replace")
    raw = proc.stdout.decode("utf-8", errors="replace").strip()
    return json.loads(raw or "{}")


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

class TestStopCheckCli:
    def test_uses_last_assistant_message_utf8(self, tmp_path):
        _make_ae_sdd_project(tmp_path)
        response = (
            f"{chr(0x25C6)} STATE:  ra-generated/REQ-001\n"
            f"{chr(0x25C6)} GATE:   {chr(0x2705)} CLEAR\n"
            f"{chr(0x25C6)} LAST:   generated RA\n"
            f"{chr(0x25C6)} NEXT:   wait user confirmation\n"
        )
        result = _run_stop_check_cli(
            tmp_path,
            {
                "hook_event_name": "Stop",
                "last_assistant_message": response,
            },
        )
        assert result == {}

    def test_allows_missing_state_header(self, tmp_path):
        """v3.6（决策 1B）：废弃 ◆ STATE 自报标记检测——无状态头 → 放行（空 dict）。"""
        _make_ae_sdd_project(tmp_path)
        result = _run_stop_check_cli(
            tmp_path,
            {
                "hook_event_name": "Stop",
                "last_assistant_message": "done without ae-sdd status header",
            },
        )
        # 放行 = 空 dict（无 decision 字段）
        assert result == {}


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


class TestManualReviewPointFormat:
    """B-3：人工审核点必须在对话内直接呈现关键结构。"""

    def test_review_point_1_complete_format_allows(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "testcase-reviewed")
        response = """
🔍 审核点1 设计完成确认
| AC | 验收标准 |
| AC-1 | 用户能登录 |
核心接口一览：POST /api/login
关键设计决策：使用现有 AuthService。
已识别风险点：第三方接口超时。
测试用例数量：8 个
"""
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        assert allowed
        assert msg == ""

    def test_review_point_1_path_only_blocks(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "testcase-reviewed")
        allowed, msg = stop_check.check_output(
            "🔍 审核点1：设计完成，请查看 design/STORY-001.md",
            ade_sdd=ade_sdd,
        )
        assert not allowed
        assert "审核点1" in msg
        assert "AC验收标准列表" in msg

    def test_review_point_2_file_checklist_allows(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "task-reviewed")
        response = """
🔍 审核点2 Task文档逐文件核对
按文件名字典序逐文件核对：
- STORY-001-task-001.md ✅ 已完整读出并说明重点
- STORY-001-task-002.md ⚠️ 已指出待确认点
"""
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        assert allowed
        assert msg == ""

    def test_review_point_25_complete_format_allows(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "coding-process")
        response = """
🔍 审核点2.5 CodingPlan评审
14条门禁通过状态：全部通过。
CodingModel 11维决策：分层、事务、异常、复用均已记录。
风险Task：Task-2 第三方接口限流。
关键类骨架：LoginService【已读源码：src/AuthService.java】。
"""
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        assert allowed
        assert msg == ""

    def test_review_point_4_complete_format_allows(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "code-reviewed")
        response = """
🔍 审核点4 CodeReview完成确认
| 问题清单 | 严重度 | 处理 |
| Issue-1 | P1 | 已修复 |
| AC覆盖对账表 | 状态 |
| AC-1 覆盖 | ✅ |
"""
        allowed, msg = stop_check.check_output(response, ade_sdd=ade_sdd)
        assert allowed
        assert msg == ""

    def test_non_review_point_response_allows(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "coding")
        allowed, msg = stop_check.check_output(
            "普通编码进展：已完成 LoginService。",
            ade_sdd=ade_sdd,
        )
        assert allowed
        assert msg == ""

    def test_automation_enabled_skips_review_format_check(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(
            tmp_path,
            "projectKey: test-proj\nautomation:\n  enabled: true\n",
        )
        _write_state(ade_sdd, "testcase-reviewed")
        allowed, msg = stop_check.check_output(
            "🔍 审核点1：设计完成，请查看 design/STORY-001.md",
            ade_sdd=ade_sdd,
        )
        assert allowed
        assert msg == ""

    def test_review_format_retry_limit_allows_third_attempt(self, tmp_path):
        ade_sdd = _make_ae_sdd_project(tmp_path)
        _write_state(ade_sdd, "testcase-reviewed")
        response = "🔍 审核点1：设计完成，请查看 design/STORY-001.md"

        first_allowed, _ = stop_check.check_output(response, ade_sdd=ade_sdd)
        second_allowed, _ = stop_check.check_output(response, ade_sdd=ade_sdd)
        third_allowed, third_msg = stop_check.check_output(response, ade_sdd=ade_sdd)

        assert not first_allowed
        assert not second_allowed
        assert third_allowed
        assert third_msg == ""
