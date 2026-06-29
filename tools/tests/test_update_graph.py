"""
test_update_graph.py — update_graph.py 单元测试（UC-01~UC-07 + AA 注入 UC-08~13）

覆盖每项检查的核心场景：通过、失败、反例。

注：import alignment_audit 会触发其 register_to_update_graph()（import-time 副作用，
见 alignment_audit.py:666），把 UC-08~13 注入 ug.CHECK_FUNCS。本模块显式 import 它，
使 check_all 在任何测试执行顺序下都确定返回 13（原生 7 + AA 注入 6）。
"""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import update_graph as ug  # noqa: E402
import lib.alignment_audit  # noqa: E402,F401  — 触发 AA 注册，使 check_all 确定返回 13


REPO_ROOT = Path(__file__).resolve().parent.parent.parent


def _setup_repo(files: dict) -> Path:
    """构造临时仓库目录。files 是 {relpath: content}。"""
    tmp = Path(tempfile.mkdtemp())
    for rel, content in files.items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    return tmp


# ─── UC-01 版本号一致性 ──────────────────────────────────────────────────────
class TestUC01(unittest.TestCase):

    def test_real_repo_passes(self):
        # 真实仓库当前版本号一致（3.2.1）
        r = ug.check_uc01_version(REPO_ROOT)
        self.assertTrue(r.pass_, r.message)

    def test_version_drift_blocks(self):
        tmp = _setup_repo({
            "source/SKILL.md": "---\nversion: 3.2.1\n---\n# ae-sdd\n",
            "tools/lib/paths.py": 'MASTER_VERSION = "3.2.0"\n',
            "README.md": "> **版本：** v3.2.1（...）\n",
        })
        r = ug.check_uc01_version(tmp)
        self.assertFalse(r.pass_)
        self.assertIn("漂移", r.message)

    def test_version_aligned_passes(self):
        tmp = _setup_repo({
            "source/SKILL.md": "---\nversion: 3.2.1\n---\n# ae-sdd\n",
            "tools/lib/paths.py": 'MASTER_VERSION = "3.2.1"\n',
            "README.md": "> **版本：** v3.2.1（最新变更）\n",
        })
        r = ug.check_uc01_version(tmp)
        self.assertTrue(r.pass_)

    def test_missing_version_blocks(self):
        tmp = _setup_repo({
            "source/SKILL.md": "# ae-sdd no frontmatter\n",
            "tools/lib/paths.py": 'MASTER_VERSION = "3.2.1"\n',
            "README.md": "> **版本：** v3.2.1\n",
        })
        r = ug.check_uc01_version(tmp)
        self.assertFalse(r.pass_)


# ─── UC-02 门禁注册一致性 ────────────────────────────────────────────────────
class TestUC02(unittest.TestCase):

    def test_real_repo_passes(self):
        r = ug.check_uc02_gates_registry(REPO_ROOT)
        self.assertTrue(r.pass_, r.message)
        self.assertGreater(r.details.get("n_gates", 0), 14)


# ─── UC-03 命令契约闭环 ──────────────────────────────────────────────────────
class TestUC03(unittest.TestCase):

    def test_real_repo_passes(self):
        # 真实仓库：本次命令全闭环，历史遗留 warn
        r = ug.check_uc03_command_contract(REPO_ROOT)
        self.assertTrue(r.pass_)  # warn 也算 pass
        # 历史遗留应被识别
        self.assertTrue(r.details.get("historical"))

    def test_init_no_longer_historical(self):
        # v3.2.5：init 已挂 CLI，不应再出现在历史遗留集合
        self.assertNotIn("init", ug.HISTORICAL_UNIMPLEMENTED)
        r = ug.check_uc03_command_contract(REPO_ROOT)
        historical = r.details.get("historical", [])
        self.assertNotIn("init", historical, "init 已挂 CLI，不应是历史遗留")
        # fork/run/skill/sync-tools 仍为未来命令
        self.assertIn("fork", ug.HISTORICAL_UNIMPLEMENTED)

    def test_new_command_missing_blocks(self):
        # SKILL.md 引用一个本次新增命令但 CLI 没实现
        tmp = _setup_repo({
            "source/SKILL.md": "---\nversion: 3.2.1\n---\n# ae-sdd\n跑 `ae-sdd newcmd --x`\n",
            "tools/bin/ae-sdd": '# cli\nsub.add_parser("gates")\n',
        })
        r = ug.check_uc03_command_contract(tmp)
        self.assertFalse(r.pass_)
        self.assertIn("newcmd", r.details.get("new_missing", []))

    def test_frontmatter_not_matched_as_command(self):
        # description: 字段不应被当成命令
        tmp = _setup_repo({
            "source/SKILL.md": "---\nname: ae-sdd\ndescription: 测试\nversion: 3.2.1\n---\n# ae-sdd\n正文\n",
            "tools/bin/ae-sdd": '# cli\n',
        })
        referenced = ug._extract_skill_referenced_commands(tmp / "source" / "SKILL.md")
        self.assertNotIn("description", referenced)


# ─── UC-04 扫描器分发一致性 ──────────────────────────────────────────────────
class TestUC04(unittest.TestCase):

    def test_real_repo_passes(self):
        r = ug.check_uc04_scanner_distribution(REPO_ROOT)
        self.assertTrue(r.pass_, r.message)

    def test_scanner_not_in_whitelist_blocks(self):
        tmp = _setup_repo({
            "scripts/new_scan.py": "# scanner\n",
            "scripts/build_dist.py": 'runtime_scripts = ["test_authenticity_scan.py"]\n',
        })
        r = ug.check_uc04_scanner_distribution(tmp)
        self.assertFalse(r.pass_)
        self.assertIn("new_scan.py", r.details.get("missing", []))


# ─── UC-05 健康度清单覆盖 ────────────────────────────────────────────────────
class TestUC05(unittest.TestCase):

    def test_real_repo(self):
        # 真实仓库：update_graph 项可能缺失（本次要补）
        r = ug.check_uc05_health_checklist(REPO_ROOT)
        # warn 项 pass_=True，只验证不崩
        self.assertIsNotNone(r.pass_)

    def test_missing_component_warns(self):
        tmp = _setup_repo({
            "source/skills/orchestration/ae-sdd-update-skill.md": "# 健康度\n- [ ] story-review\n",
        })
        r = ug.check_uc05_health_checklist(tmp)
        self.assertTrue(r.pass_)  # warn 算 pass
        self.assertGreater(len(r.details.get("missing", [])), 0)


# ─── UC-06 文档-实现一致性（🆕 v3.4.0）────────────────────────────────────────
class TestUC06(unittest.TestCase):

    def test_real_repo(self):
        r = ug.check_uc06_doc_impl_consistency(REPO_ROOT)
        # 真实仓库应通过（warn 或 pass，不应 error）
        self.assertTrue(r.pass_, f"UC-06 真实仓库应通过：{r.message}")

    def test_subskill_missing_command_blocks(self):
        # 子 SKILL 引用一个未实现命令（实际命令调用形式）→ error
        tmp = _setup_repo({
            "source/skills/phase1-design/x-skill.md": "# x\n跑 `ae-sdd newcmd --x`\n",
            "tools/bin/ae-sdd": '# cli\nsub.add_parser("gates")\n',
        })
        r = ug.check_uc06_doc_impl_consistency(tmp)
        self.assertFalse(r.pass_)
        self.assertTrue(any("newcmd" in i for i in r.details.get("issues", [])))

    def test_subskill_prose_not_matched(self):
        # 正文 "ae-sdd 生成的文档" 不应被误匹配为命令
        tmp = _setup_repo({
            "source/skills/phase1-design/x-skill.md": "# x\nae-sdd 生成的文档应存档\n",
            "tools/bin/ae-sdd": '# cli\nsub.add_parser("gates")\n',
        })
        r = ug.check_uc06_doc_impl_consistency(tmp)
        self.assertTrue(r.pass_, f"正文提及不应误报：{r.message}")

    def test_harness_hs_no_impl_warns(self):
        # HARNESS.md 声明 HS-99 但无映射 → warn（不 error）
        tmp = _setup_repo({
            "source/HARNESS.md": "# HARNESS\n- **HS-99** 某规则\n",
            "tools/bin/ae-sdd": '# cli\nsub.add_parser("gates")\n',
        })
        r = ug.check_uc06_doc_impl_consistency(tmp)
        # HS-99 无映射 → 进 warnings，pass=True
        self.assertTrue(r.pass_)


# ─── bump_version 版本号同步（v3.2.5 UC-01 操作侧）──────────────────────────
class TestBumpVersion(unittest.TestCase):

    def _setup_versioned_repo(self, old: str = "3.2.4") -> Path:
        """构造三处版本号一致的临时仓库。"""
        return _setup_repo({
            "source/SKILL.md": f"---\nname: ae-sdd\ndescription: test\nversion: {old}\n---\n# ae-sdd\n",
            "tools/lib/paths.py": f'MASTER_VERSION = "{old}"\n',
            "README.md": f"> **版本：** v{old}（最新变更）\n",
        })

    def test_bump_syncs_three_places(self):
        tmp = self._setup_versioned_repo("3.2.4")
        result = ug.bump_version(tmp, "3.2.5")
        self.assertTrue(result["verified"])
        self.assertEqual(result["old"], "3.2.4")
        self.assertEqual(result["new"], "3.2.5")
        self.assertEqual(len(result["written"]), 3)
        # 验证三处实际写入
        self.assertEqual(ug._extract_skill_version(tmp / "source" / "SKILL.md"), "3.2.5")
        self.assertEqual(ug._extract_paths_master_version(tmp / "tools" / "lib" / "paths.py"), "3.2.5")
        self.assertEqual(ug._extract_readme_version(tmp / "README.md"), "3.2.5")

    def test_bump_same_version_skips(self):
        tmp = self._setup_versioned_repo("3.2.4")
        result = ug.bump_version(tmp, "3.2.4")
        self.assertTrue(result["verified"])
        self.assertIn("skipped", result)
        self.assertEqual(result["written"], [])

    def test_bump_invalid_format_raises(self):
        tmp = self._setup_versioned_repo("3.2.4")
        with self.assertRaises(ValueError) as ctx:
            ug.bump_version(tmp, "3.2")
        self.assertIn("格式非法", str(ctx.exception))
        with self.assertRaises(ValueError):
            ug.bump_version(tmp, "v3.2.5")  # 带 v 前缀非法
        with self.assertRaises(ValueError):
            ug.bump_version(tmp, "3.2.5.1")  # 四段非法

    def test_bump_preserves_readme_paren_note(self):
        # README 括号说明应保留，只换版本号
        tmp = _setup_repo({
            "source/SKILL.md": "---\nversion: 3.2.4\n---\n# ae-sdd\n",
            "tools/lib/paths.py": 'MASTER_VERSION = "3.2.4"\n',
            "README.md": "> **版本：** v3.2.4（🆕 2026-06-24：某变更；v3.2.3：另一变更）\n",
        })
        ug.bump_version(tmp, "3.2.5")
        readme_text = (tmp / "README.md").read_text(encoding="utf-8")
        self.assertIn("v3.2.5", readme_text)
        self.assertIn("🆕 2026-06-24：某变更", readme_text)  # 括号说明保留
        self.assertNotIn("v3.2.4", readme_text)  # 旧版本号已替换

    def test_bump_verify_failure_raises(self):
        # 写入后 UC-01 校验失败的场景：SKILL.md 无 version 字段
        tmp = _setup_repo({
            "source/SKILL.md": "# ae-sdd no version field\n",
            "tools/lib/paths.py": 'MASTER_VERSION = "3.2.4"\n',
            "README.md": "> **版本：** v3.2.4\n",
        })
        with self.assertRaises(ValueError):
            ug.bump_version(tmp, "3.2.5")


# ─── check_all / summarize ───────────────────────────────────────────────────
class TestCheckAll(unittest.TestCase):

    def test_check_all_returns_13(self):
        results = ug.check_all(REPO_ROOT)
        # UC-01~07（update_graph 原生 7 项）+ UC-08~13（alignment_audit AA 注入 6 项）= 13。
        # AA 的 register_to_update_graph() 在 import alignment_audit 时自动把 UC-08~13
        # 注入共享的 ug.CHECK_FUNCS（见 alignment_audit.py:666），故全量 pytest 收集
        # test_alignment_audit.py 后本测试拿到 13 而非原生 7。这是已知的 import-time
        # 副作用耦合；若未来把 AA 注册改为显式调用，需同步回退此断言到 7。
        self.assertEqual(len(results), 13)

    def test_check_all_only_filter(self):
        results = ug.check_all(REPO_ROOT, only="UC-01")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].check_id, "UC-01")

    def test_check_all_only_uc06(self):
        results = ug.check_all(REPO_ROOT, only="UC-06")
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].check_id, "UC-06")

    def test_check_all_unknown(self):
        results = ug.check_all(REPO_ROOT, only="UC-99")
        self.assertEqual(len(results), 1)
        self.assertFalse(results[0].pass_)

    def test_summarize(self):
        results = ug.check_all(REPO_ROOT)
        s = ug.summarize(results)
        # 13 = UC-01~07 原生 + UC-08~13 AA 注入（见 test_check_all_returns_13 注释）
        self.assertEqual(s["total"], 13)
        self.assertEqual(s["passed"] + s["failed"], 13)
        self.assertIn("checks", s)


# ─── query_affected 查询 API（v3.2 Agent 可读）───────────────────────────────
class TestQueryAffected(unittest.TestCase):

    def setUp(self):
        ug.reload_graph(REPO_ROOT)

    def test_load_graph_returns_rules(self):
        graph = ug.load_graph(REPO_ROOT)
        self.assertIn("rules", graph)
        self.assertGreaterEqual(len(graph["rules"]), 8)

    def test_query_gates_py_hits_ug02(self):
        # 改 gates.py → 命中 UG-02 + UG-08
        qr = ug.query_affected(["tools/lib/gates.py"], REPO_ROOT)
        rule_ids = [r["id"] for r in qr.matched_rules]
        self.assertIn("UG-02", rule_ids)
        self.assertIn("UC-02", qr.checks_to_run)

    def test_query_new_scanner_hits_ug04(self):
        # 新增扫描器 → 命中 UG-04（含 build_dist 白名单连带项）
        qr = ug.query_affected(["scripts/ra_authenticity_scan.py"], REPO_ROOT)
        rule_ids = [r["id"] for r in qr.matched_rules]
        self.assertIn("UG-04", rule_ids)
        # 应提示 build_dist.py 白名单
        paths = [a["path"] for a in qr.affected_items]
        self.assertIn("scripts/build_dist.py", paths)

    def test_query_skill_md_hits_version_rule(self):
        # 改 SKILL.md → 命中 UG-01（版本号连带项）
        qr = ug.query_affected(["source/SKILL.md"], REPO_ROOT)
        rule_ids = [r["id"] for r in qr.matched_rules]
        self.assertIn("UG-01", rule_ids)
        paths = [a["path"] for a in qr.affected_items]
        self.assertIn("tools/lib/paths.py", paths)
        self.assertIn("README.md", paths)

    def test_query_glob_pattern_subskill(self):
        # 改子 SKILL（glob source/skills/**/*.md）→ 命中 UG-05
        qr = ug.query_affected(["source/skills/phase1-design/story-generate-skill.md"], REPO_ROOT)
        rule_ids = [r["id"] for r in qr.matched_rules]
        self.assertIn("UG-05", rule_ids)

    def test_query_dedup_affected_items(self):
        # 多个文件命中同一连带项 → 按 (path, action) 去重
        qr = ug.query_affected(["tools/lib/gates.py", "source/SKILL.md"], REPO_ROOT)
        # gates.py 的 UG-02 "GATE_REGISTRY 一致" 与 UG-04 "新增 _locate_scanner" action 不同，
        # 各算一条；但同一 (path, action) 不应重复
        keys = [(a["path"], a["action"]) for a in qr.affected_items]
        self.assertEqual(len(keys), len(set(keys)), "连带项应按 (path,action) 去重")

    def test_query_no_match_returns_empty(self):
        # 改一个不存在的文件 → 无命中
        qr = ug.query_affected(["nonexistent/file.txt"], REPO_ROOT)
        self.assertEqual(len(qr.matched_rules), 0)
        self.assertEqual(len(qr.affected_items), 0)
        self.assertEqual(len(qr.checks_to_run), 0)

    def test_query_windows_path_normalized(self):
        # Windows 反斜杠路径应被归一化
        qr = ug.query_affected(["tools\\lib\\gates.py"], REPO_ROOT)
        self.assertEqual(qr.changed_files, ["tools/lib/gates.py"])
        self.assertGreater(len(qr.matched_rules), 0)

    def test_query_checks_to_run_deduped(self):
        # 多规则指向同一 UC → 去重
        qr = ug.query_affected(["tools/lib/gates.py", "tools/bin/ae-sdd"], REPO_ROOT)
        # UC-03 被多条规则引用，应只出现一次
        self.assertEqual(qr.checks_to_run.count("UC-03"), 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
