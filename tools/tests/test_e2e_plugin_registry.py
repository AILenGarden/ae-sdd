"""test_e2e_plugin_registry.py -- end-to-end test for v3.5.0/v3.5.1 plugin registry

Simulates a real project owner workflow:
1. Set up a mock project (icec-cloud-boss-like)
2. Test all 7 scenarios:
   - L1 project layer override
   - Multi-layer conflict (L1 + L2)
   - L1 provides new skill
   - L0 fallback (no registration)
   - CLI end-to-end (init + edit + validate + trace)
   - YAML error handling
   - compatibility warning

Each scenario cleans up after itself.
"""
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CLI = REPO_ROOT / "tools" / "bin" / "ae-sdd"
sys.path.insert(0, str(REPO_ROOT / "tools"))
from lib import plugin_loader  # noqa: E402


def _isolated_env():
    """Same as test_plugin_cli.py: redirect HOME/USERPROFILE to tmp."""
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "utf-8"
    tmp = tempfile.mkdtemp(prefix="ae-sdd-e2e-home-")
    env["HOME"] = tmp
    env["USERPROFILE"] = tmp
    return env, tmp


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def run_ae_sdd(*args, env_overrides=None, cwd=None, expect_returncode=None):
    env, _ = _isolated_env()
    if env_overrides:
        env.update(env_overrides)
    proc = subprocess.run(
        [sys.executable, str(CLI), *args],
        capture_output=True, text=True, encoding="utf-8",
        env=env, cwd=cwd,
    )
    if expect_returncode is not None:
        assert proc.returncode == expect_returncode, (
            f"returncode mismatch: expected {expect_returncode}, got {proc.returncode}\n"
            f"stdout: {proc.stdout}\nstderr: {proc.stderr}"
        )
    return proc


# ====================================================
# Scenario 1: L1 project layer override (most basic)
# ====================================================

class TestE2EScenario1(unittest.TestCase):
    """L1 project layer overrides coding-skill.md (most basic)."""

    def test_l1_overrides_coding_skill(self):
        # Build a mock project: <project>/.ae-sdd/config.yaml + .ae-sdd/plugins/registry.yaml
        # The plugin SKILL lives at <project>/.ae-sdd/plugins/boss-coding/SKILL.md
        project = Path(tempfile.mkdtemp(prefix="e2e-s1-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        # config.yaml (minimal)
        write_file(ade_sdd / "config.yaml", "projectKey: e2e-s1\n")
        # external SKILL
        boss_skill = ade_sdd / "plugins" / "boss-coding" / "SKILL.md"
        write_file(boss_skill, textwrap.dedent("""\
            ---
            name: boss-coding
            description: E2E scenario 1: boss project coding style
            ---

            # Boss Coding (E2E)
        """))
        # registry.yaml
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            description: E2E scenario 1
            plugins:
              - name: boss-coding-style
                type: skill-override
                version: 0.1.0
                description: boss TDD + DDD
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./boss-coding/SKILL.md
        """))

        try:
            # Use plugin_loader directly (simulating Agent call)
            master = REPO_ROOT / "source"
            result = plugin_loader.resolve_skill(
                "source/skills/phase2-coding/coding-skill.md",
                ade_sdd=ade_sdd,
                master=master,
            )
            self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
            self.assertEqual(result.layer_label, "L1-project")
            self.assertEqual(result.plugin.name, "boss-coding-style")
            self.assertEqual(result.resolved_path, boss_skill.resolve())

            # Verify the resolved file actually exists and has the expected content
            content = result.resolved_path.read_text(encoding="utf-8")
            self.assertIn("Boss Coding (E2E)", content)
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)


# ====================================================
# Scenario 2: Multi-layer conflict
# ====================================================

class TestE2EScenario2(unittest.TestCase):
    """L1 (project) + L2 (global) both override same target — L1 wins."""

    def test_multi_layer_conflict_l1_wins(self):
        # Build mock project
        project = Path(tempfile.mkdtemp(prefix="e2e-s2-proj-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        write_file(ade_sdd / "config.yaml", "projectKey: e2e-s2\n")

        # L1: project layer boss-coding
        proj_skill = ade_sdd / "plugins" / "boss-coding" / "SKILL.md"
        write_file(proj_skill, "# Boss Coding (L1)\n")
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: boss-coding-style
                type: skill-override
                version: 0.1.0
                description: boss TDD
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./boss-coding/SKILL.md
        """))

        # L2: global layer personal-coding (use AE_SDD_GLOBAL_HOME to redirect)
        tmp_home = Path(tempfile.mkdtemp(prefix="e2e-s2-global-"))
        glob_skill = tmp_home / ".ae-sdd" / "plugins" / "personal-coding" / "SKILL.md"
        write_file(glob_skill, "# Personal Coding (L2)\n")
        write_file(tmp_home / ".ae-sdd" / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: personal-coding-style
                type: skill-override
                version: 0.1.0
                description: personal style
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./personal-coding/SKILL.md
        """))

        # Use AE_SDD_GLOBAL_HOME to redirect L2 path
        old_env = os.environ.get("AE_SDD_GLOBAL_HOME")
        os.environ["AE_SDD_GLOBAL_HOME"] = str(tmp_home)
        try:
            master = REPO_ROOT / "source"
            result = plugin_loader.resolve_skill(
                "source/skills/phase2-coding/coding-skill.md",
                ade_sdd=ade_sdd,
                master=master,
            )
            # L1 should win
            self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
            self.assertEqual(result.plugin.name, "boss-coding-style")
            # Conflict recorded
            self.assertEqual(len(result.conflicts), 1, msg=f"warnings: {result.warnings}")
            self.assertEqual(result.conflicts[0].target,
                             "source/skills/phase2-coding/coding-skill.md")
            self.assertEqual(len(result.conflicts[0].losers), 1)
            self.assertEqual(result.conflicts[0].losers[0].name, "personal-coding-style")
            self.assertEqual(result.conflicts[0].losers[0].layer_label, "L2-global")
            # Warning present
            self.assertTrue(len(result.warnings) > 0)
            warning_text = " ".join(result.warnings)
            self.assertIn("L1", warning_text)
            self.assertIn("胜出", warning_text)
        finally:
            if old_env is None:
                os.environ.pop("AE_SDD_GLOBAL_HOME", None)
            else:
                os.environ["AE_SDD_GLOBAL_HOME"] = old_env
            import shutil
            shutil.rmtree(project, ignore_errors=True)
            shutil.rmtree(tmp_home, ignore_errors=True)


# ====================================================
# Scenario 3: provides new skill (project-only)
# ====================================================

class TestE2EScenario3(unittest.TestCase):
    """L1 provides a new SKILL (e.g. finance-coding-skill)."""

    def test_provides_new_skill(self):
        project = Path(tempfile.mkdtemp(prefix="e2e-s3-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        write_file(ade_sdd / "config.yaml", "projectKey: e2e-s3\n")

        # New SKILL for finance domain
        fin_skill = ade_sdd / "plugins" / "finance-coding" / "SKILL.md"
        write_file(fin_skill, textwrap.dedent("""\
            ---
            name: finance-coding
            description: finance domain coding (precision, rounding, reconciliation)
            ---

            # Finance Coding

            Use BigDecimal for money, never float.
        """))
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            description: project adds finance domain
            plugins:
              - name: finance-coding
                type: skill-new
                version: 0.1.0
                description: finance domain coding
                provides: finance-coding-skill
                path: ./finance-coding/SKILL.md
        """))

        try:
            master = REPO_ROOT / "source"
            # Route via provides key
            result = plugin_loader.resolve_skill(
                "finance-coding-skill",  # the provides key
                ade_sdd=ade_sdd,
                master=master,
            )
            self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
            self.assertEqual(result.plugin.name, "finance-coding")
            self.assertEqual(result.resolved_path, fin_skill.resolve())

            # Read the SKILL content via the resolved path
            content = result.resolved_path.read_text(encoding="utf-8")
            self.assertIn("BigDecimal", content)
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)


# ====================================================
# Scenario 4: Fallback (no registration)
# ====================================================

class TestE2EScenario4(unittest.TestCase):
    """No registry -> fallback to L0-builtin."""

    def test_fallback_when_no_registry(self):
        # No project layer, no global layer
        project = Path(tempfile.mkdtemp(prefix="e2e-s4-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        write_file(ade_sdd / "config.yaml", "projectKey: e2e-s4\n")

        env, tmp_home = _isolated_env()

        try:
            master = REPO_ROOT / "source"
            result = plugin_loader.resolve_skill(
                "source/skills/phase2-coding/coding-skill.md",
                ade_sdd=ade_sdd,
                master=master,
            )
            # Should fallback
            self.assertEqual(result.layer, plugin_loader.LAYER_BUILTIN)
            self.assertIsNone(result.plugin)
            self.assertIsNone(result.resolved_path)
            self.assertIn("fallback", " ".join(result.warnings))
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)
            shutil.rmtree(tmp_home, ignore_errors=True)


# ====================================================
# Scenario 5: CLI end-to-end (init + edit + validate + trace)
# ====================================================

class TestE2EScenario5(unittest.TestCase):
    """End-to-end CLI flow: init -> edit registry -> validate -> trace."""

    def test_cli_init_edit_validate_trace(self):
        # Set up mock project with .ae-sdd/ (init needs this for project layer)
        project = Path(tempfile.mkdtemp(prefix="e2e-s5-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        write_file(ade_sdd / "config.yaml", "projectKey: e2e-s5\n")
        env, tmp_home = _isolated_env()

        try:
            # Step 1: init generates a registry.yaml (we'll use the global layer to avoid needing .ae-sdd cwd)
            proc = subprocess.run(
                [sys.executable, str(CLI), "plugin", "init", "--layer", "global"],
                capture_output=True, text=True, encoding="utf-8",
                env=env, cwd=str(project),
            )
            self.assertEqual(proc.returncode, 0, msg=f"init failed: {proc.stderr}")
            target = Path(tmp_home) / ".ae-sdd" / "plugins" / "registry.yaml"
            self.assertTrue(target.is_file())

            # Step 2: Edit the generated file - add a real plugin entry
            # (and create the SKILL file the plugin points to)
            global_skill = Path(tmp_home) / ".ae-sdd" / "plugins" / "my-coding" / "SKILL.md"
            write_file(global_skill, "# My Coding (from CLI e2e)\n")
            write_file(target, textwrap.dedent("""\
                schema_version: 1
                description: edited via CLI
                plugins:
                  - name: my-coding
                    type: skill-override
                    version: 0.1.0
                    description: e2e my coding
                    replaces: source/skills/phase2-coding/coding-skill.md
                    path: ./my-coding/SKILL.md
            """))

            # Step 3: validate
            proc = subprocess.run(
                [sys.executable, str(CLI), "plugin", "validate"],
                capture_output=True, text=True, encoding="utf-8",
                env=env, cwd=str(project),
            )
            self.assertEqual(proc.returncode, 0, msg=f"validate failed: {proc.stderr}")
            combined = proc.stdout + proc.stderr
            self.assertIn("校验通过", combined)
            # 🆕 v3.5.10：L3 默认注册表（example-coding-style）会随母版加载，
            # 临时 project 的 L1 注册表（my-coding）+ L3 默认 = ≥1 个插件即可
            self.assertIn("个插件", combined)
            # 不再硬断言 "1 个插件"，因 L3 默认注册表会被加载（设计行为）

            # Step 4: trace
            proc = subprocess.run(
                [sys.executable, str(CLI), "plugin",
                 "trace", "source/skills/phase2-coding/coding-skill.md"],
                capture_output=True, text=True, encoding="utf-8",
                env=env, cwd=str(project),
            )
            self.assertEqual(proc.returncode, 0, msg=f"trace failed: {proc.stderr}")
            combined = proc.stdout + proc.stderr
            self.assertIn("L2-global", combined)
            self.assertIn("my-coding", combined)

            # Step 5: list
            proc = subprocess.run(
                [sys.executable, str(CLI), "plugin", "list"],
                capture_output=True, text=True, encoding="utf-8",
                env=env, cwd=str(project),
            )
            self.assertEqual(proc.returncode, 0, msg=f"list failed: {proc.stderr}")
            combined = proc.stdout + proc.stderr
            self.assertIn("my-coding", combined)
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)
            shutil.rmtree(tmp_home, ignore_errors=True)


# ====================================================
# Scenario 6: YAML error handling
# ====================================================

class TestE2EScenario6(unittest.TestCase):
    """YAML error handling: syntax / schema / path missing."""

    def test_yaml_syntax_error_blocks(self):
        project = Path(tempfile.mkdtemp(prefix="e2e-s6-syntax-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        # Invalid YAML: mismatched brackets
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: skill-override
                [invalid yaml
        """))

        try:
            master = REPO_ROOT / "source"
            rl = plugin_loader.load_registry(
                ade_sdd / "plugins" / "registry.yaml",
                layer=plugin_loader.LAYER_PROJECT,
                layer_label="L1-project",
            )
            self.assertTrue(rl.exists)
            # Either parsing error or zero plugins
            self.assertEqual(len(rl.plugins), 0)
            # At least one error reported
            self.assertGreater(len(rl.errors), 0)
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)

    def test_schema_violation_blocks_plugin(self):
        project = Path(tempfile.mkdtemp(prefix="e2e-s6-schema-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        # Valid YAML but invalid type
        skill = ade_sdd / "plugins" / "SKILL.md"
        write_file(skill, "# X\n")
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: invalid-type
                version: 0.1.0
                description: foo
                path: ./SKILL.md
        """))

        try:
            master = REPO_ROOT / "source"
            result = plugin_loader.validate(ade_sdd, master)
            self.assertFalse(result["valid"])
            # error mentions 'type'
            self.assertTrue(any("type" in e for e in result["errors"]))
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)

    def test_missing_path_file_blocks_plugin(self):
        project = Path(tempfile.mkdtemp(prefix="e2e-s6-path-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        # Valid YAML + valid type, but path doesn't exist
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: skill-override
                version: 0.1.0
                description: foo
                replaces: source/skills/foo.md
                path: ./nonexistent-SKILL.md
        """))

        try:
            master = REPO_ROOT / "source"
            result = plugin_loader.validate(ade_sdd, master)
            self.assertFalse(result["valid"])
            # error mentions existence
            self.assertTrue(any("存在" in e or "exist" in e.lower() for e in result["errors"]))
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)


# ====================================================
# Scenario 7: compatibility warning
# ====================================================

class TestE2EScenario7(unittest.TestCase):
    """compatibility.ae_sdd_version satisfied -> silent; unsatisfied -> warn (still loads)."""

    def test_compatibility_satisfied_silent(self):
        project = Path(tempfile.mkdtemp(prefix="e2e-s7-ok-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        skill = ade_sdd / "plugins" / "SKILL.md"
        write_file(skill, "# X\n")
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent(f"""\
            schema_version: 1
            plugins:
              - name: future-coding
                type: skill-override
                version: 0.1.0
                description: requires >=3.5.1
                replaces: source/skills/coding-skill.md
                path: ./SKILL.md
                compatibility:
                  ae_sdd_version: ">=3.5.0"
        """))

        try:
            master = REPO_ROOT / "source"
            result = plugin_loader.resolve_skill(
                "source/skills/coding-skill.md",
                ade_sdd=ade_sdd,
                master=master,
            )
            # Should hit L1, no compatibility warning
            compat_warnings = [w for w in result.warnings if "compatibility" in w.lower() or "version" in w.lower()]
            self.assertEqual(len(compat_warnings), 0, msg=f"unexpected warnings: {result.warnings}")
            self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)

    def test_compatibility_unsatisfied_warns_but_loads(self):
        project = Path(tempfile.mkdtemp(prefix="e2e-s7-warn-"))
        ade_sdd = project / ".ae-sdd"
        ade_sdd.mkdir()
        skill = ade_sdd / "plugins" / "SKILL.md"
        write_file(skill, "# X\n")
        # Requires a future version that doesn't exist
        write_file(ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: future-coding
                type: skill-override
                version: 0.1.0
                description: requires >=99.0.0 (impossible)
                replaces: source/skills/coding-skill.md
                path: ./SKILL.md
                compatibility:
                  ae_sdd_version: ">=99.0.0"
        """))

        try:
            master = REPO_ROOT / "source"
            result = plugin_loader.resolve_skill(
                "source/skills/coding-skill.md",
                ade_sdd=ade_sdd,
                master=master,
            )
            # Still hits L1 (warning does not block)
            self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
            self.assertIsNotNone(result.plugin)
            # But warning emitted
            compat_warnings = [w for w in result.warnings if "99.0" in w or "version" in w.lower()]
            self.assertGreater(len(compat_warnings), 0, msg=f"expected warning, got: {result.warnings}")
        finally:
            import shutil
            shutil.rmtree(project, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()