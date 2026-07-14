"""
test_pending_init_deadlock_fix.py — v3.10.2 修复验证测试

背景：本仓库（ae-sdd 工具自身源码仓）根目录曾残留一个游离的
`.ae-sdd-pending-init` 标记文件，把所有 Write/Edit/Bash 永久锁死。

根因分两层：
  1. prompt_inject.inject() 清除 pending-init 标记的代码只在
     "后来定位到了 .ae-sdd/"分支里才会执行；对永远不会存在 .ae-sdd/ 的
     仓库（如本仓库自身），标记一旦写入就没有任何自救路径。
  2. 拒绝提示建议"说'走快速通道'紧急绕过"，但快速通道机制
     （_update_quick_channel）同样要求先定位到 .ae-sdd/ 才生效——是一条
     指向死路的提示；已有的 disengage 词（"退出 ae-sdd"/"不锁了"）在
     ade_sdd is None 分支里也从未被检查。

修复：
  - prompt_inject.inject() 在 ade_sdd is None 分支里也检查
    AE_SDD_DISENGAGE_MARKERS，命中则清除标记（不依赖项目已初始化）。
  - gate_intercept._check_pending_init_intercept 的拒绝提示改为指向
    真正生效的 disengage 词，不再提"走快速通道"。
  - 🆕 补漏：_deny_response() 此前会无条件在任意 reason 后追加通用的
    "如需紧急绕过，请说...走快速通道"尾巴——即便上一条已经把 reason 本身
    改对了，用户看到的最终 systemMessage 里旧建议其实还在。现按 reason
    前缀识别 pending-init 场景，跳过这条尾巴（其余场景不受影响，仍保留
    原提示——那些场景下 .ae-sdd/ 已存在，快速通道是有效的）。

覆盖：
  1. 触发词写入标记后，disengage 词能在未初始化项目里清除标记
  2. 清除后 pending-init 拦截不再触发（模拟 gate-intercept 后续调用放行）
  3. 拒绝提示文案不再包含误导性的"走快速通道"建议
  4. 端到端：check_intercept + _deny_response 包装后的最终 systemMessage
     也不含误导建议；同时确认非 pending-init 场景的 systemMessage 仍保留
     快速通道提示（不能一刀切删掉，回归 test_gate_intercept.py 的既有约定）
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import paths as paths_mod  # noqa: E402
from lib.gate_intercept import (  # noqa: E402
    _check_pending_init_intercept,
    _deny_response,
    _is_ae_sdd_self_repo,
    check_intercept,
)
from lib.prompt_inject import inject  # noqa: E402


class TestPendingInitDeadlockFix:

    def test_disengage_marker_clears_pending_init_without_ae_sdd_dir(self, tmp_path):
        """未初始化项目（无 .ae-sdd/）：触发词写标记后，disengage 词应能清除它。"""
        marker = paths_mod.pending_init_marker(tmp_path)
        assert not marker.exists()

        # 触发词 → 写入标记（模拟用户之前调过 /ae-sdd 但项目未 init）
        inject(project_dir=tmp_path, user_prompt="/ae-sdd 开始处理")
        assert marker.is_file(), "触发词应写入 pending-init 标记"

        # disengage 词 → 应能清除标记，即使 .ae-sdd/ 从未存在
        inject(project_dir=tmp_path, user_prompt="退出 ae-sdd")
        assert not marker.exists(), "disengage 词应能清除 pending-init 标记，不依赖 .ae-sdd/ 是否存在"

    def test_disengage_marker_alternate_phrase_also_clears(self, tmp_path):
        """disengage 词的另一种说法（'不锁了'）同样生效。"""
        marker = paths_mod.pending_init_marker(tmp_path)
        inject(project_dir=tmp_path, user_prompt="/ae-sdd 开始处理")
        assert marker.is_file()

        inject(project_dir=tmp_path, user_prompt="不锁了")
        assert not marker.exists()

    def test_pending_init_intercept_blocks_write_before_disengage(self, tmp_path):
        """标记存在时，Write 工具应被拦截（回归：确认拦截本身仍生效）。"""
        marker = paths_mod.pending_init_marker(tmp_path)
        marker.write_text("ae-sdd pending init", encoding="utf-8")

        allowed, reason = _check_pending_init_intercept(
            "Write", bash_command=None, file_path="src/Foo.java", allow_readonly=True,
        )
        assert allowed is False
        assert "尚未初始化" in reason

    def test_pending_init_intercept_deny_reason_points_to_working_escape(self, tmp_path):
        """拒绝提示不应再建议'走快速通道'（对这个场景是死路），应指向 disengage 词。"""
        allowed, reason = _check_pending_init_intercept(
            "Write", bash_command=None, file_path="src/Foo.java", allow_readonly=True,
        )
        assert allowed is False
        assert "走快速通道" not in reason, "快速通道机制要求先定位到 .ae-sdd/，对本场景无效，不应误导"
        assert "退出 ae-sdd" in reason or "不锁了" in reason

    def test_write_allowed_again_after_disengage(self, tmp_path):
        """完整闭环：写标记 → 拦截生效 → disengage 清除标记 → 后续判定不再命中该拦截分支。"""
        marker = paths_mod.pending_init_marker(tmp_path)
        inject(project_dir=tmp_path, user_prompt="/ae-sdd 开始处理")
        assert marker.is_file()

        # 清除前：拦截生效
        allowed, _ = _check_pending_init_intercept(
            "Write", bash_command=None, file_path="src/Foo.java", allow_readonly=True,
        )
        assert allowed is False

        # disengage 清除标记
        inject(project_dir=tmp_path, user_prompt="退出 ae-sdd")
        assert not marker.exists()

        # 标记已清除后，check_intercept 的上游判空逻辑（pending.exists()）不会再
        # 路由进 _check_pending_init_intercept；本层直接验证标记状态即代表死锁已解。

    def test_deny_response_wrapping_does_not_reintroduce_quick_channel_hint(self, tmp_path):
        """🆕 补漏：check_intercept + _deny_response 包装后的最终 systemMessage
        也不应含误导性的"走快速通道"建议——这是用户实际看到的完整文本。"""
        marker = paths_mod.pending_init_marker(tmp_path)
        marker.write_text("ae-sdd pending init", encoding="utf-8")

        allowed, reason = check_intercept(
            "Write", file_path="src/Foo.java", project_dir=tmp_path,
        )
        assert allowed is False
        wrapped = _deny_response("Write", reason)
        message = wrapped["systemMessage"]
        assert "走快速通道" not in message, (
            "_deny_response 不应对 pending-init reason 追加通用的快速通道尾巴，"
            "该建议对本场景无效（.ae-sdd/ 还不存在，快速通道机制读不到标记）"
        )
        assert "退出 ae-sdd" in message or "不锁了" in message

    def test_deny_response_keeps_quick_channel_hint_for_other_reasons(self):
        """回归：非 pending-init 场景的 systemMessage 仍应保留快速通道提示
        （对应 test_gate_intercept.py::TestDenyMessage 的既有约定，不能一刀切删）。"""
        allowed, reason = check_intercept("Write", forced_phase="completed")
        assert allowed is False
        wrapped = _deny_response("Write", reason)
        assert "快速通道" in wrapped["systemMessage"]


class TestSelfRepoExemption:
    """🆕 v3.10.4：ae-sdd 工具自身仓库豁免测试。

    背景：ae-sdd 工具本体源码仓永远不会有 .ae-sdd/，但用户在此仓内发 /ae-sdd
    触发词会写 .ae-sdd-pending-init 标记，随后 gate_intercept 无差别拦截所有
    Write/Edit/Bash，连"修 hook 自己"都拦，形成死锁。修复从根上识别工具仓
    并放行，不进 pending-init 拦截分支。
    """

    @staticmethod
    def _make_self_repo_layout(root: Path) -> Path:
        """在 root 下造出 ae-sdd 工具仓的标志文件结构，返回工作子目录。"""
        (root / "tools" / "bin").mkdir(parents=True)
        (root / "tools" / "lib").mkdir(parents=True)
        (root / "tools" / "bin" / "ae-sdd").write_text("#!/bin/sh\n", encoding="utf-8")
        (root / "tools" / "lib" / "gate_intercept.py").write_text("# stub\n", encoding="utf-8")
        # 工作子目录（模拟在工具仓内的子路径干活）
        work_dir = root / "tools" / "lib"
        return work_dir

    def test_is_ae_sdd_self_repo_detects_tool_layout(self, tmp_path):
        """含 tools/bin/ae-sdd + tools/lib/gate_intercept.py 双文件的目录应被识别为工具仓。"""
        work_dir = self._make_self_repo_layout(tmp_path)
        assert _is_ae_sdd_self_repo(work_dir) is True
        # 仓根本身也应命中
        assert _is_ae_sdd_self_repo(tmp_path) is True

    def test_is_ae_sdd_self_repo_rejects_plain_project(self, tmp_path):
        """普通项目目录（无标志文件）不应被误判为工具仓。"""
        (tmp_path / "src").mkdir()
        (tmp_path / "src" / "App.java").write_text("class App {}", encoding="utf-8")
        assert _is_ae_sdd_self_repo(tmp_path) is False
        assert _is_ae_sdd_self_repo(None) is False

    def test_is_ae_sdd_self_repo_needs_both_marker_files(self, tmp_path):
        """只有单个标志文件（如只有 tools/bin/ae-sdd）不应判定为工具仓。"""
        (tmp_path / "tools" / "bin").mkdir(parents=True)
        (tmp_path / "tools" / "bin" / "ae-sdd").write_text("#!/bin/sh\n", encoding="utf-8")
        # 缺 tools/lib/gate_intercept.py
        assert _is_ae_sdd_self_repo(tmp_path) is False

    def test_self_repo_exempt_even_with_pending_init_marker(self, tmp_path):
        """核心：工具仓内即便存在 .ae-sdd-pending-init 标记，Write 也不应被拦截。"""
        work_dir = self._make_self_repo_layout(tmp_path)
        # 模拟用户在工具仓内发过 /ae-sdd 触发词，标记已写入
        marker = paths_mod.pending_init_marker(work_dir)
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text("ae-sdd pending init", encoding="utf-8")
        assert marker.is_file(), "前置：标记应已存在"

        allowed, reason = check_intercept(
            "Write", file_path="tools/lib/gate_intercept.py", project_dir=work_dir,
        )
        assert allowed is True, "ae-sdd 工具自身仓库应豁免 pending-init 拦截，不形成死锁"
        assert reason == ""

    def test_self_repo_exempt_for_bash_and_edit_too(self, tmp_path):
        """豁免覆盖 Write/Edit/MultiEdit/Bash 全部受拦工具，不留死角。"""
        work_dir = self._make_self_repo_layout(tmp_path)
        marker = paths_mod.pending_init_marker(work_dir)
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text("ae-sdd pending init", encoding="utf-8")

        for tool, extra in [
            ("Edit", {"file_path": "tools/lib/gate_intercept.py"}),
            ("MultiEdit", {"file_path": "tools/lib/gate_intercept.py"}),
            ("Bash", {"bash_command": "python -m pytest tools/tests/"}),
        ]:
            allowed, _ = check_intercept(tool, project_dir=work_dir, **extra)
            assert allowed is True, f"{tool} 在工具仓内应被豁免"

    def test_non_self_repo_still_blocked_with_marker(self, tmp_path):
        """回归：普通项目（无标志文件）有标记时仍应被拦截，豁免不能误放。"""
        marker = paths_mod.pending_init_marker(tmp_path)
        marker.write_text("ae-sdd pending init", encoding="utf-8")
        allowed, reason = check_intercept(
            "Write", file_path="src/Foo.java", project_dir=tmp_path,
        )
        assert allowed is False, "普通项目不应被豁免"
        assert "尚未初始化" in reason


if __name__ == "__main__":
    import unittest
    unittest.main(verbosity=2)
