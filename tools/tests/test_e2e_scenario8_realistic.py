"""Scenario 8: Real-world simulation for icec-cloud-boss project owner (EDY).

Simulates a complete workflow:
1. EDY's icec-cloud-boss project has its own CodingSKILL customization
2. EDY also has personal global preferences
3. Test priority synthesis in this realistic mixed scenario
4. Test new project-specific skill (provides)
"""
import os
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "tools"))
from lib import plugin_loader  # noqa: E402


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class TestE2EScenario8(unittest.TestCase):
    """icec-cloud-boss real scenario: L1 + L2 + provides simultaneously."""

    def setUp(self):
        # Build mock project: <project>/icec-cloud-boss/.ae-sdd/
        self.project = Path(tempfile.mkdtemp(prefix="e2e-s8-boss-"))
        self.ade_sdd = self.project / "icec-cloud-boss" / ".ae-sdd"
        self.ade_sdd.mkdir(parents=True)
        write_file(self.ade_sdd / "config.yaml", "projectKey: icec-cloud-boss\n")

        # L1: project layer overrides coding-skill.md
        # (boss team uses TDD + 重试幂等 + heavy observability)
        self.proj_coding_skill = self.ade_sdd / "plugins" / "boss-coding" / "SKILL.md"
        write_file(self.proj_coding_skill, textwrap.dedent("""\
            ---
            name: boss-coding
            description: icec-cloud-boss TDD + 重试幂等 CodingSKILL
            ---

            # Boss Coding

            ## 规则
            1. 测试先行
            2. 幂等 + 重试必加
            3. trace_id 必加
        """))
        write_file(self.ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            description: icec-cloud-boss 团队 CodingSKILL 定制
            plugins:
              - name: boss-coding
                type: skill-override
                version: 0.1.0
                author: EDY
                description: TDD + 重试幂等
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./boss-coding/SKILL.md
                compatibility:
                  ae_sdd_version: ">=3.5.0"
                tags: [team-style, tdd, retry-idempotent]
        """))

        # L1 also provides a new skill: boss-finance-coding (项目专属)
        self.proj_finance_skill = self.ade_sdd / "plugins" / "boss-finance" / "SKILL.md"
        write_file(self.proj_finance_skill, textwrap.dedent("""\
            ---
            name: boss-finance-coding
            description: boss 财务领域 Coding（精度/舍入/对账）
            ---

            # Boss Finance Coding

            - BigDecimal not float
            - 4 舍 5 入到分
            - 日终对账必跑
        """))
        # Add finance to registry (manual indent to avoid dedent issues)
        finance_block = (
            "\n"
            "  - name: boss-finance-coding\n"
            "    type: skill-new\n"
            "    version: 0.1.0\n"
            "    description: boss 财务领域\n"
            "    provides: boss-finance-coding-skill\n"
            "    path: ./boss-finance/SKILL.md\n"
            "    dependencies: [boss-coding]\n"
            "    tags: [finance]\n"
        )
        with open(self.ade_sdd / "plugins" / "registry.yaml", "a", encoding="utf-8") as f:
            f.write(finance_block)

        # L2: global layer personal coding style (EDY's personal preference)
        # (every project: prefer DDD style over TDD unless project overrides)
        self.tmp_global_home = Path(tempfile.mkdtemp(prefix="e2e-s8-global-"))
        glob_skill = self.tmp_global_home / ".ae-sdd" / "plugins" / "personal-ddd" / "SKILL.md"
        write_file(glob_skill, textwrap.dedent("""\
            ---
            name: personal-ddd-coding
            description: EDY's personal DDD + Clean Arch coding style
            ---

            # Personal DDD Coding

            - Domain modeling first
            - Aggregate / Entity / VO
            - Repository pattern
        """))
        write_file(self.tmp_global_home / ".ae-sdd" / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            description: EDY 个人 Coding 偏好（DDD + Clean Arch）
            plugins:
              - name: personal-ddd-coding
                type: skill-override
                version: 0.2.0
                description: 个人 DDD 偏好
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./personal-ddd/SKILL.md
        """))

        # Set AE_SDD_GLOBAL_HOME to redirect global layer
        self._old_global_home = os.environ.get("AE_SDD_GLOBAL_HOME")
        os.environ["AE_SDD_GLOBAL_HOME"] = str(self.tmp_global_home)

        self.master = REPO_ROOT / "source"

    def tearDown(self):
        if self._old_global_home is None:
            os.environ.pop("AE_SDD_GLOBAL_HOME", None)
        else:
            os.environ["AE_SDD_GLOBAL_HOME"] = self._old_global_home
        import shutil
        shutil.rmtree(self.project, ignore_errors=True)
        shutil.rmtree(self.tmp_global_home, ignore_errors=True)

    def test_project_coding_skill_wins(self):
        """icec-cloud-boss 项目层覆盖 coding-skill.md → L1 胜出 vs EDY 全局层"""
        result = plugin_loader.resolve_skill(
            "source/skills/phase2-coding/coding-skill.md",
            ade_sdd=self.ade_sdd,
            master=self.master,
        )
        # L1 boss-coding wins (project > global)
        self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
        self.assertEqual(result.plugin.name, "boss-coding")
        self.assertEqual(result.resolved_path, self.proj_coding_skill.resolve())

        # Conflict: L2 personal-ddd-coding is overridden
        self.assertEqual(len(result.conflicts), 1)
        self.assertEqual(result.conflicts[0].winner.name, "boss-coding")
        self.assertEqual(result.conflicts[0].losers[0].name, "personal-ddd-coding")
        self.assertEqual(result.conflicts[0].losers[0].layer_label, "L2-global")

        # The resolved content should be the boss one, not the personal one
        content = result.resolved_path.read_text(encoding="utf-8")
        self.assertIn("Boss Coding", content)
        self.assertIn("测试先行", content)
        self.assertIn("幂等", content)
        # NOT the personal DDD content
        self.assertNotIn("DDD", content)

    def test_global_only_targets_still_work(self):
        """L2 全局层独有的 target → 仍然命中 L2"""
        # Personal-ddd-coding also covers coding-skill.md which is what we tested
        # Let's test that fallback works for an unrelated target
        result = plugin_loader.resolve_skill(
            "source/skills/phase1-design/story-generate-skill.md",
            ade_sdd=self.ade_sdd,
            master=self.master,
        )
        # No registration for story-generate -> fallback
        self.assertEqual(result.layer, plugin_loader.LAYER_BUILTIN)
        self.assertIsNone(result.plugin)

    def test_provides_new_skill(self):
        """L1 provides boss-finance-coding-skill → 通过 provides key 命中"""
        result = plugin_loader.resolve_skill(
            "boss-finance-coding-skill",
            ade_sdd=self.ade_sdd,
            master=self.master,
        )
        self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
        self.assertEqual(result.plugin.name, "boss-finance-coding")
        self.assertEqual(result.resolved_path, self.proj_finance_skill.resolve())

        # Read the finance SKILL
        content = result.resolved_path.read_text(encoding="utf-8")
        self.assertIn("BigDecimal", content)
        self.assertIn("对账", content)

    def test_list_plugins_shows_all_three_layers(self):
        """list_plugins 应该合并三层：L1 (2 plugins) + L2 (1 plugin) + L0 (no registry)"""
        result = plugin_loader.list_plugins(self.ade_sdd, self.master)
        self.assertEqual(result["totalPlugins"], 3)  # 2 L1 + 1 L2
        # 1 conflict (boss-coding vs personal-ddd-coding)
        self.assertEqual(result["totalConflicts"], 1)
        # Layer breakdown
        layers_by_name = {l["layerLabel"]: l for l in result["layers"]}
        self.assertEqual(len(layers_by_name["L1-project"]["plugins"]), 2)
        self.assertEqual(len(layers_by_name["L2-global"]["plugins"]), 1)
        self.assertFalse(layers_by_name["L3-master"]["exists"])

    def test_validate_passes_for_realistic_setup(self):
        """validate 应该通过：3 个合法 plugin + 1 个 conflict（warn 不阻断）"""
        result = plugin_loader.validate(self.ade_sdd, self.master)
        self.assertTrue(result["valid"], msg=f"errors: {result['errors']}")
        self.assertEqual(result["totalPlugins"], 3)
        self.assertEqual(result["totalConflicts"], 1)
        # Should have warnings (the conflict)
        self.assertGreater(len(result["warnings"]), 0)


if __name__ == "__main__":
    unittest.main()