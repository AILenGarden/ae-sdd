"""
test_l2_inject.py - l2_inject.py 单元测试（🆕 v3.10.8）。

覆盖：
  - SSOT 读取与语言切片（zh/en）
  - 锚点区间替换（锚点外零改动断言）
  - diff-aware skip（hash 一致不重写）
  - bootstrap 段落边界识别（三家各自的标题模式）
  - 备份 + 回滚
  - mavis/hermes 跳过（无 l2_global_file）
  - Claude 红线条款 11 补丁识别
"""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent / "scripts"))

from tools.lib import distributor_registry as dr  # noqa: E402
import l2_inject  # noqa: E402


# ─── 测试用 SSOT 母版 ─────────────────────────────────────────────────────────

_FAKE_SSOT = """<!-- ae-sdd L2 会话级纪律 SSOT -->

<!-- ════════════════════════════════════════════════════════════════════════════ -->
<!-- SECTION:zh -- 中文版（ZCode / Claude 注入用）                                -->
<!-- ════════════════════════════════════════════════════════════════════════════ -->

## ae-sdd 工作流强制调用（git 仓库及工程类目录）

凡请求可能产生代码改动，在制定实现计划或首次写入前加载并调用 `ae-sdd`。

- **任务大小不构成豁免理由**

<!-- /SECTION:zh -->

<!-- ════════════════════════════════════════════════════════════════════════════ -->
<!-- SECTION:en -- 英文版（Codex 注入用）                                          -->
<!-- ════════════════════════════════════════════════════════════════════════════ -->

## Mandatory ae-sdd Coding Workflow

For every task that may create code, load and invoke `ae-sdd` before the first write.

- **Task size is not an exemption**

<!-- /SECTION:en -->

<!-- redline11:zh -->
| 11 | 文档承载 changelog | 主题连续性破坏 |

> 条款 11 与 ae-sdd `source/L2-DISCIPLINE.md` 同源。
<!-- /redline11:zh -->

<!-- redline11:en -->
| 11 | Documents carrying changelog | Topic continuity broken |

> Clause 11 is co-sourced with ae-sdd `source/L2-DISCIPLINE.md`.
<!-- /redline11:en -->
"""


def _write_ssot(tmp: Path) -> Path:
    """在临时目录写一份 SSOT 母版，返回路径。"""
    ssot = tmp / "L2-DISCIPLINE.md"
    ssot.write_text(_FAKE_SSOT, encoding="utf-8")
    return ssot


# ─── SSOT 读取与语言切片 ─────────────────────────────────────────────────────

class TestSSOTSlicing(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ssot = _write_ssot(self.tmp)

    def test_slice_zh_contains_chinese(self):
        body = l2_inject._slice_section(self.ssot.read_text(encoding="utf-8"), "zh")
        self.assertIn("工作流强制调用", body)
        self.assertIn("任务大小不构成豁免理由", body)
        self.assertNotIn("Mandatory ae-sdd", body)

    def test_slice_en_contains_english(self):
        body = l2_inject._slice_section(self.ssot.read_text(encoding="utf-8"), "en")
        self.assertIn("Mandatory ae-sdd", body)
        self.assertIn("Task size is not an exemption", body)
        self.assertNotIn("工作流强制调用", body)

    def test_slice_invalid_lang_raises(self):
        with self.assertRaises(ValueError):
            l2_inject._slice_section(self.ssot.read_text(encoding="utf-8"), "fr")

    def test_redline11_zh(self):
        rl = l2_inject._slice_redline11(self.ssot.read_text(encoding="utf-8"), "zh")
        self.assertIn("文档承载 changelog", rl)
        self.assertIn("L2-DISCIPLINE.md", rl)

    def test_redline11_en(self):
        rl = l2_inject._slice_redline11(self.ssot.read_text(encoding="utf-8"), "en")
        self.assertIn("Documents carrying changelog", rl)


# ─── 锚点区间替换（核心安全：锚点外零改动）──────────────────────────────────

class TestAnchoredInjection(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ssot = _write_ssot(self.tmp)
        # 模拟已 bootstrap 的全局文件（有锚点）
        self.target = self.tmp / "AGENTS.md"
        self.before_anchor = "# My Agent\n\n## Some Rule\nDo stuff.\n\n---\n\n"
        self.after_anchor = "\n---\n\n## Another Section\nKeep me.\n"
        self.old_inner = "## OLD ae-sdd content\nOld stuff.\n"
        self._write_target_with_anchor(self.old_inner)

    def _write_target_with_anchor(self, inner: str):
        content = (
            self.before_anchor
            + f"<!-- BEGIN {l2_inject.ANCHOR_BEGIN} @ test @ 20260101T000000Z -->\n"
            + inner
            + f"<!-- END {l2_inject.ANCHOR_BEGIN} -->\n"
            + self.after_anchor
        )
        self.target.write_text(content, encoding="utf-8")

    @patch.object(l2_inject, "SSOT_PATH", None)
    def test_inject_replaces_only_anchor_region(self):
        """注入后锚点外区域与原始完全一致。"""
        l2_inject.SSOT_PATH = self.ssot
        original = self.target.read_text(encoding="utf-8")
        original_lines = original.splitlines(keepends=True)
        span = l2_inject._find_anchor(original_lines)
        self.assertIsNotNone(span, "setUp 应已写入锚点")
        original_outside = l2_inject._extract_outside_anchor(original_lines, span)

        res = l2_inject._inject_anchored(self.target, "zh", quiet=True)
        self.assertEqual(res.status, "ok")

        new_lines = self.target.read_text(encoding="utf-8").splitlines(keepends=True)
        new_span = l2_inject._find_anchor(new_lines)
        self.assertIsNotNone(new_span)
        new_outside = l2_inject._extract_outside_anchor(new_lines, new_span)

        # 核心断言：锚点外零改动
        self.assertEqual(original_outside, new_outside,
                         "锚点外区域被改动！违反安全约束。")

    @patch.object(l2_inject, "SSOT_PATH", None)
    def test_inject_updates_inner_content(self):
        """注入后锚点区间内容已更新为 SSOT 切片。"""
        l2_inject.SSOT_PATH = self.ssot
        l2_inject._inject_anchored(self.target, "zh", quiet=True)
        content = self.target.read_text(encoding="utf-8")
        self.assertIn("工作流强制调用", content)
        self.assertNotIn("OLD ae-sdd content", content)

    @patch.object(l2_inject, "SSOT_PATH", None)
    def test_diff_aware_skip(self):
        """hash 一致时第二次注入应 skip。"""
        l2_inject.SSOT_PATH = self.ssot
        res1 = l2_inject._inject_anchored(self.target, "zh", quiet=True)
        self.assertEqual(res1.status, "ok")
        res2 = l2_inject._inject_anchored(self.target, "zh", quiet=True)
        self.assertEqual(res2.status, "skip")

    @patch.object(l2_inject, "SSOT_PATH", None)
    def test_skip_when_no_anchor(self):
        """无锚点时返回 skip_no_anchor，不自动 bootstrap。"""
        l2_inject.SSOT_PATH = self.ssot
        no_anchor = self.tmp / "no_anchor.md"
        no_anchor.write_text("# No anchor here\nJust text.\n", encoding="utf-8")
        res = l2_inject._inject_anchored(no_anchor, "zh", quiet=True)
        self.assertEqual(res.status, "skip_no_anchor")


# ─── Bootstrap 段落边界识别 ──────────────────────────────────────────────────

class TestBootstrapSpanDetection(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ssot = _write_ssot(self.tmp)
        l2_inject.SSOT_PATH = self.ssot

    def test_zcode_span_detection(self):
        """ZCode: 识别 '## ae-sdd 工作流强制调用' 到下一个非 ae-sdd 的 ## 标题。"""
        content = (
            "# AILen Coder\n\n## 黄金法则\nDo good.\n\n---\n\n"
            "## ae-sdd 工作流强制调用（git 仓库下）\n首轮必须 /ae-sdd。\n\n---\n\n"
            "## ae-sdd Coding 执行纪律\nSDD+TDD。\n\n---\n\n"
            "## 设计原则\nSOLID.\n"
        )
        lines = content.splitlines(keepends=True)
        span = l2_inject._find_bootstrap_span(lines, "zcode")
        self.assertIsNotNone(span)
        start, end = span
        replaced = "".join(lines[start:end])
        self.assertIn("工作流强制调用", replaced)
        self.assertIn("Coding 执行纪律", replaced)
        # 不应包含"设计原则"
        self.assertNotIn("设计原则", replaced)
        # 不应包含"黄金法则"
        self.assertNotIn("黄金法则", replaced)

    def test_codex_span_detection(self):
        """Codex: 识别 '## Mandatory ae-sdd Coding Workflow' 到下一个 ## 标题。"""
        content = (
            "# Codex Global\n\n## Thinking Engine\nThink.\n\n"
            "## Mandatory ae-sdd Coding Workflow\nLoad ae-sdd.\n- one-line not exempt\n\n"
            "## Skill Source\npath here.\n"
        )
        lines = content.splitlines(keepends=True)
        span = l2_inject._find_bootstrap_span(lines, "codex")
        self.assertIsNotNone(span)
        start, end = span
        replaced = "".join(lines[start:end])
        self.assertIn("Mandatory ae-sdd", replaced)
        self.assertIn("one-line not exempt", replaced)
        self.assertNotIn("Skill Source", replaced)
        self.assertNotIn("Thinking Engine", replaced)

    def test_claude_span_detection(self):
        """Claude: 识别 '## ae-sdd 工作流调用' 到下一个非 ae-sdd ## 标题。"""
        content = (
            "# AILen Coder\n\n## 强制红线（10条）\n| 1 | rule | bad |\n\n---\n\n"
            "## ae-sdd 工作流调用（git 仓库下）\n只需触发一次。\n\n"
            "### ae-sdd 流程中的编码行为约束\n表格内容。\n\n---\n\n"
            "## 设计原则\nSOLID.\n"
        )
        lines = content.splitlines(keepends=True)
        span = l2_inject._find_bootstrap_span(lines, "claude")
        self.assertIsNotNone(span)
        start, end = span
        replaced = "".join(lines[start:end])
        self.assertIn("工作流调用", replaced)
        self.assertIn("编码行为约束", replaced)
        self.assertNotIn("设计原则", replaced)

    def test_claude_redline11_gap_detection(self):
        """Claude 红线表条款 10 后是条款 11 的插入点。"""
        content = (
            "## 强制红线（10条）\n\n"
            "| # | 规则 | 后果 |\n|---|------|------|\n"
            "| 9 | 循环内调用外部接口 | 性能灾难 |\n"
            "| 10 | 重复代码 | 技术债务 |\n\n"
            "---\n"
        )
        lines = content.splitlines(keepends=True)
        gap = l2_inject._find_claude_redline11_gap(lines)
        self.assertIsNotNone(gap)
        # gap 应指向条款 10 行的下一行
        self.assertTrue(lines[gap - 1].strip().startswith("| 10 |"))


# ─── 备份与回滚 ───────────────────────────────────────────────────────────────

class TestBackupRollback(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.target = self.tmp / "AGENTS.md"
        self.target.write_text("original content\n", encoding="utf-8")

    def test_backup_creates_bak_file(self):
        bak = l2_inject._backup_file(self.target)
        self.assertIsNotNone(bak)
        self.assertTrue(bak.exists())
        self.assertEqual(bak.read_text(encoding="utf-8"), "original content\n")

    def test_rollback_restores_latest(self):
        l2_inject._backup_file(self.target)
        self.target.write_text("modified\n", encoding="utf-8")
        ok = l2_inject._rollback(self.target)
        self.assertTrue(ok)
        self.assertEqual(self.target.read_text(encoding="utf-8"), "original content\n")

    def test_rollback_no_backup_returns_false(self):
        ok = l2_inject._rollback(self.target)
        self.assertFalse(ok)


# ─── mavis/hermes 跳过 ────────────────────────────────────────────────────────

class TestAgentFiltering(unittest.TestCase):

    def test_mavis_hermes_have_no_l2_config(self):
        """mavis/hermes 的 l2_global_file 应为 None。"""
        for name, cfg in dr._KNOWN_AGENTS.items():
            if name in ("mavis", "hermes"):
                self.assertIsNone(cfg["l2_global_file"],
                                  f"{name} 不应有 l2_global_file")
                self.assertIsNone(cfg["l2_language"])

    def test_claude_codex_zcode_have_l2_config(self):
        """claude/codex/zcode 应有 l2 配置。"""
        for name, cfg in dr._KNOWN_AGENTS.items():
            if name in ("claude", "zcode"):
                self.assertIsNotNone(cfg["l2_global_file"])
                self.assertEqual(cfg["l2_language"], "zh")
            elif name == "codex":
                self.assertIsNotNone(cfg["l2_global_file"])
                self.assertEqual(cfg["l2_language"], "en")


# ─── Bootstrap apply 集成 ─────────────────────────────────────────────────────

class TestBootstrapApply(unittest.TestCase):

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ssot = _write_ssot(self.tmp)
        l2_inject.SSOT_PATH = self.ssot

    def test_bootstrap_apply_creates_anchor(self):
        """bootstrap apply 后文件应包含锚点标记。"""
        target = self.tmp / "AGENTS.md"
        content = (
            "# Agent\n\n## Some Rule\nKeep.\n\n---\n\n"
            "## ae-sdd 工作流强制调用（git 仓库下）\nOld content.\n\n---\n\n"
            "## Design\nPrinciples.\n"
        )
        target.write_text(content, encoding="utf-8")

        res = l2_inject._bootstrap_apply(target, "zcode", "zh", quiet=True)
        self.assertEqual(res.status, "ok")

        result = target.read_text(encoding="utf-8")
        self.assertIn(f"<!-- BEGIN {l2_inject.ANCHOR_BEGIN}", result)
        self.assertIn(f"<!-- END {l2_inject.ANCHOR_BEGIN}", result)
        self.assertIn("工作流强制调用", result)
        self.assertIn("Some Rule", result)  # 锚点外内容保留
        self.assertIn("Design", result)     # 锚点外内容保留
        self.assertNotIn("Old content.", result)  # 旧段落已替换

    def test_bootstrap_preview_found(self):
        """bootstrap dry-run 预览应识别到段落。"""
        target = self.tmp / "AGENTS.md"
        content = (
            "# Agent\n\n## Mandatory ae-sdd Coding Workflow\nOld.\n\n## Next\nstuff\n"
        )
        target.write_text(content, encoding="utf-8")
        preview = l2_inject._bootstrap_preview(target, "codex", "en")
        self.assertTrue(preview["found"])
        self.assertIn("replace_range", preview)

    def test_bootstrap_preview_not_found(self):
        """无 ae-sdd 段落时预览应报 found=False。"""
        target = self.tmp / "AGENTS.md"
        target.write_text("# Agent\nNo ae-sdd here.\n", encoding="utf-8")
        preview = l2_inject._bootstrap_preview(target, "codex", "en")
        self.assertFalse(preview["found"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
