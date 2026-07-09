"""
test_iteration_check.py — iteration_check 模块单元测试

覆盖：
- IC-1 过时技术栈/幽灵命令扫描（含 changelog 跳过逻辑）
- IC-2 F-1 交叉验证覆盖面计数
- IC-3 已实现未接入扫描（import 解析 + untracked 检测）
- IC-4 HS 物理实现粗筛（粗筛通过 / 零提及 / 自认降级）
- run_all 报告整合
"""
from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import iteration_check as ic


# ─── IC-1 ────────────────────────────────────────────────────────────────────

class TestIC1ObsoleteTech:
    def test_ghost_command_detected(self, tmp_path):
        skill = tmp_path / "SKILL.md"
        cli = tmp_path / "ae-sdd"
        skill.write_text("Run `ae-sdd assets audit` to verify.\n", encoding="utf-8")
        cli.write_text("", encoding="utf-8")
        findings = ic.check_ic1_obsolete_tech(skill, cli)
        assert any("幽灵命令" in f.item and "assets audit" in f.item for f in findings)

    def test_changelog_line_skipped_for_obsolete_kw(self, tmp_path):
        """含 🆕/v3.x 等历史标记的行不应报过时技术栈"""
        skill = tmp_path / "SKILL.md"
        cli = tmp_path / "ae-sdd"
        skill.write_text("🆕 v3.0 新增 sync-tools 章节。\n", encoding="utf-8")
        cli.write_text("", encoding="utf-8")
        findings = ic.check_ic1_obsolete_tech(skill, cli)
        assert not findings, "历史 changelog 行不应被报为过时技术栈残留"

    def test_mechanism_line_reported(self, tmp_path):
        """非 changelog 行含 rules.yaml 等机制描述应被报"""
        skill = tmp_path / "SKILL.md"
        cli = tmp_path / "ae-sdd"
        skill.write_text("同步 rules.yaml 到 tools/。\n", encoding="utf-8")
        cli.write_text("", encoding="utf-8")
        findings = ic.check_ic1_obsolete_tech(skill, cli)
        assert any("rules.yaml" in f.item for f in findings)

    def test_no_skill_md_returns_empty(self, tmp_path):
        findings = ic.check_ic1_obsolete_tech(tmp_path / "missing.md", tmp_path / "cli")
        assert findings == []


# ─── IC-2 ────────────────────────────────────────────────────────────────────

class TestIC2GateClaimCoverage:
    def test_only_g08_covered_warns(self, tmp_path):
        stop = tmp_path / "stop_check.py"
        gates = tmp_path / "gates.py"
        stop.write_text('_G08_CLEAR_RE = re.compile(r"G-08.*CLEAR")\n', encoding="utf-8")
        gates.write_text('"G-01" "G-02" "G-03" "G-04" "G-05" "G-06" "G-07" "G-08"\n', encoding="utf-8")
        findings = ic.check_ic2_gate_claim_coverage(stop, gates)
        assert any("F-1 交叉验证" in f.item for f in findings)

    def test_multi_gate_covered_no_warn(self, tmp_path):
        stop = tmp_path / "stop_check.py"
        gates = tmp_path / "gates.py"
        stop.write_text(
            '_G08_CLEAR_RE = re.compile(r"G-08")\n'
            '_G09_CLEAR_RE = re.compile(r"G-09")\n'
            '_G14_CLEAR_RE = re.compile(r"G-14")\n',
            encoding="utf-8",
        )
        gates.write_text('"G-08" "G-09" "G-10" "G-11" "G-12" "G-13" "G-14"\n', encoding="utf-8")
        findings = ic.check_ic2_gate_claim_coverage(stop, gates)
        assert not any("F-1 交叉验证" in f.item for f in findings)

    def test_retired_self_report_check_no_warn(self, tmp_path):
        """v3.6 Stop hook 废弃 GATE 自报检测后，不再按旧 _G08_CLEAR_RE 覆盖面报 warn。"""
        stop = tmp_path / "stop_check.py"
        gates = tmp_path / "gates.py"
        stop.write_text(
            "废弃 _verify_gate_claims()（gate 自报交叉验证）\n"
            "流程合规性检测已全部转移到 UserPromptSubmit hook\n",
            encoding="utf-8",
        )
        gates.write_text('"G-01" "G-02" "G-03" "G-04" "G-05" "G-06" "G-07" "G-08"\n', encoding="utf-8")
        findings = ic.check_ic2_gate_claim_coverage(stop, gates)
        assert not any(f.severity == "warn" for f in findings)
        assert any("已废弃" in f.item for f in findings)


# ─── IC-3 ────────────────────────────────────────────────────────────────────

class TestIC3UnimportedModules:
    def test_module_in_aesdd_import_list_not_reported(self, tmp_path):
        """模拟 tools/lib/output.py + tools/bin/ae-sdd 引用 output，output 不应报未接入"""
        tools = tmp_path / "tools"
        lib = tools / "lib"
        lib.mkdir(parents=True)
        bin = tools / "bin"
        bin.mkdir(parents=True)
        (lib / "output.py").write_text("# output\n", encoding="utf-8")
        (lib / "gate.py").write_text("# dummy\n", encoding="utf-8")
        (bin / "ae-sdd").write_text(
            "from lib import output, gate\n", encoding="utf-8"
        )
        findings = ic.check_ic3_unimported_modules(tools)
        assert not any(f.item == "模块 'output' 已实现但全树零 import" for f in findings), (
            f"output 误报: {[f.item for f in findings]}"
        )

    def test_truly_unimported_module_detected(self, tmp_path):
        tools = tmp_path / "tools"
        lib = tools / "lib"
        lib.mkdir(parents=True)
        bin = tools / "bin"
        bin.mkdir(parents=True)
        (lib / "deadcode.py").write_text("# deadcode\n", encoding="utf-8")
        (lib / "output.py").write_text("# output\n", encoding="utf-8")
        (bin / "ae-sdd").write_text("from lib import output\n", encoding="utf-8")
        findings = ic.check_ic3_unimported_modules(tools)
        assert any("deadcode" in f.item for f in findings)

    def test_aesdd_no_py_extension_handled(self, tmp_path):
        """ae-sdd 无 .py 后缀但应被扫描"""
        tools = tmp_path / "tools"
        lib = tools / "lib"
        bin = tools / "bin"
        lib.mkdir(parents=True)
        bin.mkdir(parents=True)
        (lib / "gates.py").write_text("# gates\n", encoding="utf-8")
        (bin / "ae-sdd").write_text("from lib import gates\n", encoding="utf-8")
        findings = ic.check_ic3_unimported_modules(tools)
        assert not any("gates" in f.item and "未接入" in f.item for f in findings)


# ─── IC-4 ────────────────────────────────────────────────────────────────────

class TestIC4HSPhysicalImpl:
    def test_known_hs_with_keyword_passes(self, tmp_path):
        """HS-7 声明物理拦截 + gate_intercept.py 含 prd-complete → info 通过"""
        harness = tmp_path / "HARNESS.md"
        tools = tmp_path / "tools"
        lib = tools / "lib"
        lib.mkdir(parents=True)
        harness.write_text("- **HS-7** prd-complete 物理拦截\n", encoding="utf-8")
        (lib / "gate_intercept.py").write_text("# prd-complete check\n", encoding="utf-8")
        findings = ic.check_ic4_hs_physical_impl(harness, tools)
        hs7 = [f for f in findings if "HS-7" in f.item]
        assert hs7 and hs7[0].severity == "info"

    def test_known_hs_missing_keyword_warns(self, tmp_path):
        """HS-7 声明物理拦截 + gate_intercept.py 缺 prd-complete → warn 撒谎"""
        harness = tmp_path / "HARNESS.md"
        tools = tmp_path / "tools"
        lib = tools / "lib"
        lib.mkdir(parents=True)
        harness.write_text("- **HS-7** prd-complete 物理拦截\n", encoding="utf-8")
        (lib / "gate_intercept.py").write_text("# no relevant keyword\n", encoding="utf-8")
        findings = ic.check_ic4_hs_physical_impl(harness, tools)
        hs7 = [f for f in findings if "HS-7" in f.item]
        assert hs7 and hs7[0].severity == "warn"

    def test_hs_with_self_declared_downgrade_info(self, tmp_path):
        """HS-3 声明'靠 agent 自律'→ info（已诚实自认）"""
        harness = tmp_path / "HARNESS.md"
        tools = tmp_path / "tools"
        lib = tools / "lib"
        lib.mkdir(parents=True)
        harness.write_text(
            "- **HS-3** 模糊回复（🆕 v3.4.0：声明但无物理实现，靠 agent 自律）\n",
            encoding="utf-8",
        )
        (lib / "gate_intercept.py").write_text("# unrelated\n", encoding="utf-8")
        findings = ic.check_ic4_hs_physical_impl(harness, tools)
        hs3 = [f for f in findings if "HS-3" in f.item]
        assert hs3 and hs3[0].severity == "info"

    def test_hs_unmapped_without_downgrade_warns(self, tmp_path):
        """HS-4 无映射且未自认降级 → warn 需人工确认"""
        harness = tmp_path / "HARNESS.md"
        tools = tmp_path / "tools"
        lib = tools / "lib"
        lib.mkdir(parents=True)
        harness.write_text("- **HS-4** 跳过 ⑥bis/⑦bis 一致性核查\n", encoding="utf-8")
        (lib / "gate_intercept.py").write_text("# unrelated\n", encoding="utf-8")
        findings = ic.check_ic4_hs_physical_impl(harness, tools)
        hs4 = [f for f in findings if "HS-4" in f.item]
        assert hs4 and hs4[0].severity == "warn"


# ─── run_all ──────────────────────────────────────────────────────────────────

class TestRunAll:
    def test_run_all_returns_report(self, tmp_path):
        """run_all 在临时仓库上跑通，产出报告"""
        # 构造最小仓库结构
        source = tmp_path / "source"
        tools = tmp_path / "tools"
        lib = tools / "lib"
        bin_ = tools / "bin"
        source.mkdir(); lib.mkdir(parents=True); bin_.mkdir(parents=True)
        (source / "SKILL.md").write_text("# SKILL\n", encoding="utf-8")
        (source / "HARNESS.md").write_text("- **HS-7** prd-complete 物理拦截\n", encoding="utf-8")
        (lib / "gate_intercept.py").write_text("# prd-complete\n", encoding="utf-8")
        (lib / "stop_check.py").write_text('#\n', encoding="utf-8")
        (lib / "gates.py").write_text('"G-08"\n', encoding="utf-8")
        (bin_ / "ae-sdd").write_text(
            "from lib import gate_intercept, stop_check, gates, output\n",
            encoding="utf-8",
        )
        (lib / "output.py").write_text("#\n", encoding="utf-8")

        report = ic.run_all(tmp_path)
        assert "IC-1" in report.checks_run
        assert "IC-2" in report.checks_run
        assert "IC-3" in report.checks_run
        assert "IC-4" in report.checks_run
        d = report.to_dict()
        assert "findings" in d
        assert "n_warn" in d
        assert "n_info" in d
        # output 应被识别为已接入（CLI import 列表里有）
        assert not any("'output'" in f["item"] for f in d["findings"])
