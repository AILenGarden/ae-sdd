"""test_cli_doc.py — 端到端验证 `ae-sdd doc save/resolve/finalize` CLI 命令（🆕 v3.7.2）。

验证 document_storage 激活后的真实 CLI 调用链：
  建草稿 → ae-sdd doc save → 验证最终路径 + ChangeLog + STORING + 草稿已删
  ae-sdd doc resolve → 只推路径不写
  ae-sdd doc finalize → 补登记不覆盖

通过 subprocess 调真实 CLI（非 mock），覆盖 LLM 实际执行路径。
"""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CLI = str(Path(__file__).resolve().parent.parent / "bin" / "ae-sdd")


def _setup_project() -> Path:
    """建临时 .ae-sdd 项目根（含 assets.md + config.yaml），返回项目根路径。"""
    tmp = Path(tempfile.mkdtemp())
    (tmp / ".ae-sdd" / "assets").mkdir(parents=True, exist_ok=True)
    (tmp / ".ae-sdd" / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
    (tmp / ".ae-sdd" / "assets" / "test.assets.md").write_text(
        f"# §A §B §C §D §E §F §G\n\n| gitPath | `{tmp}` |\n| docWorkspacePath | `{tmp}` |\n",
        encoding="utf-8",
    )
    return tmp


def _run_cli(cwd: Path, *args: str) -> tuple[int, str, str]:
    """跑 ae-sdd CLI，返回 (exit_code, stdout, stderr)。"""
    env = os.environ.copy()
    env["PYTHONPATH"] = str(Path(__file__).resolve().parent.parent.parent)
    r = subprocess.run(
        [sys.executable, CLI, *args],
        capture_output=True, text=True, cwd=str(cwd), env=env, encoding="utf-8",
    )
    return r.returncode, r.stdout, r.stderr


class TestDocSave(unittest.TestCase):
    """`ae-sdd doc save` 端到端。"""

    def test_save_design_doc_end_to_end(self):
        """建草稿 → doc save STORY → 验证文件落地 + ChangeLog + STORING + 草稿删除。"""
        tmp = _setup_project()
        draft = tmp / ".ae-sdd" / "tmp" / "STORY-001-BE-draft.md"
        draft.parent.mkdir(parents=True, exist_ok=True)
        draft.write_text("# Story 内容", encoding="utf-8")

        code, out, err = _run_cli(tmp, "doc", "save",
                                  "--intent", "STORY",
                                  "--story-id", "STORY-001-BE",
                                  "--content-file", str(draft),
                                  "--changelog-note", "首次创建")
        self.assertEqual(code, 0, msg=f"stderr={err}\nstdout={out}")

        # 最终文件存在
        final = tmp / "ae-sdd-doc" / "Story" / "STORY-001-BE.md"
        self.assertTrue(final.is_file(), f"最终文件不存在：{final}")
        self.assertEqual(final.read_text(encoding="utf-8"), "# Story 内容")

        # ChangeLog 同级生成
        cl = tmp / "ae-sdd-doc" / "Story" / "STORY-001-BE-changelog.md"
        self.assertTrue(cl.is_file())
        self.assertIn("首次创建", cl.read_text(encoding="utf-8"))

        # STORING 更新
        storing = tmp / "ae-sdd-doc" / "STORING.md"
        self.assertTrue(storing.is_file())
        self.assertIn("STORY-001-BE.md", storing.read_text(encoding="utf-8"))

        # 草稿已删
        self.assertFalse(draft.exists(), "草稿应已自动删除")

    def test_save_with_keep_draft(self):
        """--keep-draft 保留草稿文件。"""
        tmp = _setup_project()
        draft = tmp / ".ae-sdd" / "tmp" / "draft.md"
        draft.parent.mkdir(parents=True, exist_ok=True)
        draft.write_text("# 内容", encoding="utf-8")

        code, out, err = _run_cli(tmp, "doc", "save",
                                  "--intent", "STORY",
                                  "--story-id", "STORY-002-BE",
                                  "--content-file", str(draft),
                                  "--keep-draft")
        self.assertEqual(code, 0, msg=f"stderr={err}\nstdout={out}")
        self.assertTrue(draft.exists(), "--keep-draft 应保留草稿")

    def test_save_unknown_intent_returns_e000(self):
        """未知 intent 退出码 1 + 含 E000 提示。"""
        tmp = _setup_project()
        draft = tmp / "draft.md"
        draft.write_text("# x", encoding="utf-8")

        code, out, err = _run_cli(tmp, "doc", "save",
                                  "--intent", "BOGUS",
                                  "--content-file", str(draft))
        self.assertEqual(code, 1)
        combined = out + err
        self.assertIn("E000", combined)

    def test_save_report_version_increment(self):
        """事件类报告（CODING_REPORT）两份 doc save，版本号 r 自增、路径不同。"""
        tmp = _setup_project()
        results = []
        for i in (1, 2):
            draft = tmp / f".ae-sdd/tmp/r{i}.md"
            draft.parent.mkdir(parents=True, exist_ok=True)
            draft.write_text(f"# r{i}", encoding="utf-8")
            code, out, err = _run_cli(tmp, "doc", "save",
                                      "--intent", "CODING_REPORT",
                                      "--story-id", "STORY-001",
                                      "--content-file", str(draft),
                                      "--keep-draft")
            self.assertEqual(code, 0, msg=f"r{i} stderr={err}")
            # output.ok/info 走 stderr（见 output.py 设计约定）
            results.append(out + err)
        # 两份路径不同（r1 vs r2）
        self.assertIn("CodingReport-v1-r1", results[0])
        self.assertIn("CodingReport-v1-r2", results[1])


class TestDocResolve(unittest.TestCase):
    """`ae-sdd doc resolve` 只推路径不写。"""

    def test_resolve_outputs_path_without_writing(self):
        """resolve 输出路径但不创建文件。"""
        tmp = _setup_project()
        code, out, err = _run_cli(tmp, "doc", "resolve",
                                  "--intent", "STORY",
                                  "--story-id", "STORY-099-BE")
        self.assertEqual(code, 0, msg=f"stderr={err}")
        # output.ok/info 走 stderr（不污染 stdout 数据流，见 output.py §设计约定）
        combined = out + err
        self.assertIn("STORY-099-BE", combined)
        # 不应创建文件
        self.assertFalse((tmp / "ae-sdd-doc" / "Story" / "STORY-099-BE.md").exists())

    def test_resolve_json_output(self):
        """--json 输出结构化 JSON。"""
        tmp = _setup_project()
        code, out, err = _run_cli(tmp, "doc", "resolve",
                                  "--intent", "CODING_PLAN",
                                  "--story-id", "STORY-001",
                                  "--json")
        self.assertEqual(code, 0, msg=f"stderr={err}")
        import json
        data = json.loads(out)
        self.assertIn("fullPath", data)
        self.assertIn("CodingPlan", data["fullPath"])


class TestDocFinalize(unittest.TestCase):
    """`ae-sdd doc finalize` 补登记不覆盖内容。"""

    def test_finalize_appends_changelog_without_overwrite(self):
        """finalize 补 ChangeLog/STORING，不覆盖已写内容。"""
        tmp = _setup_project()
        target = tmp / "ae-sdd-doc" / "Story" / "STORY-FINAL-BE.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        original = "# 原始内容不被覆盖"
        target.write_text(original, encoding="utf-8")

        code, out, err = _run_cli(tmp, "doc", "finalize",
                                  "--path", str(target),
                                  "--intent", "STORY",
                                  "--story-id", "STORY-FINAL-BE",
                                  "--changelog-note", "补登记")
        self.assertEqual(code, 0, msg=f"stderr={err}")

        # 内容未被覆盖
        self.assertEqual(target.read_text(encoding="utf-8"), original)
        # ChangeLog 已补
        cl = target.parent / "STORY-FINAL-BE-changelog.md"
        self.assertTrue(cl.is_file())
        self.assertIn("补登记", cl.read_text(encoding="utf-8"))

    def test_finalize_nonexistent_file_exits_1(self):
        """finalize 不存在的文件退出码 1。"""
        tmp = _setup_project()
        code, out, err = _run_cli(tmp, "doc", "finalize",
                                  "--path", str(tmp / "nope.md"),
                                  "--intent", "STORY")
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
