"""test_plugin_loader.py -- unit tests for plugin_loader.py (v3.5.0)

Covers:
- _parse_yaml_subset (YAML subset parser: comments/literal block/list of dict/nested dict)
- load_registry (single-layer load + schema validation + name/replaces uniqueness)
- collect_all_layers (three-layer collection)
- detect_conflicts (multi-layer conflict detection)
- resolve_skill (priority synthesis + fallback)
- _version_satisfies (compatibility check)

No external dependencies. Uses tempfile to construct temp dir trees.
"""
import sys
import os
import tempfile
import textwrap
import unittest
from pathlib import Path

# Make 'lib' importable
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import plugin_loader  # noqa: E402


def write_file(path: Path, content: str) -> None:
    """Write file + auto-create parent dirs."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


# ====================================================
# YAML subset parser tests
# ====================================================

class TestParseYamlSubset(unittest.TestCase):
    """_parse_yaml_subset unit tests."""

    def test_simple_scalar(self):
        text = "schema_version: 1"
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d, {"schema_version": 1})

    def test_string_with_spaces(self):
        text = "name: my plugin"
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d["name"], "my plugin")

    def test_quoted_string(self):
        text = 'name: "my plugin"'
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d["name"], "my plugin")

    def test_comment_ignored(self):
        text = textwrap.dedent("""\
            # this is comment
            schema_version: 1  # inline comment
            name: foo
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d, {"schema_version": 1, "name": "foo"})

    def test_literal_block(self):
        text = textwrap.dedent("""\
            description: |
              line 1
              line 2
              line 3
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertIn("description", d)
        self.assertIn("line 1", d["description"])
        self.assertIn("line 2", d["description"])
        self.assertIn("line 3", d["description"])

    def test_list_of_scalar(self):
        text = textwrap.dedent("""\
            tags:
              - foo
              - bar
              - baz
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d["tags"], ["foo", "bar", "baz"])

    def test_list_of_dict(self):
        text = textwrap.dedent("""\
            plugins:
              - name: foo
                type: skill-override
                version: 0.1.0
              - name: bar
                type: skill-new
                version: 0.2.0
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(len(d["plugins"]), 2)
        self.assertEqual(d["plugins"][0]["name"], "foo")
        self.assertEqual(d["plugins"][0]["type"], "skill-override")
        self.assertEqual(d["plugins"][1]["name"], "bar")
        self.assertEqual(d["plugins"][1]["type"], "skill-new")

    def test_nested_dict(self):
        text = textwrap.dedent("""\
            compatibility:
              ae_sdd_version: ">=3.5.0"
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d["compatibility"], {"ae_sdd_version": ">=3.5.0"})

    def test_complex_registry(self):
        text = textwrap.dedent("""\
            schema_version: 1
            description: |
              complex test registry

            plugins:
              - name: my-coding
                type: skill-override
                version: 0.1.0
                author: "EDY"
                description: My coding style
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./my-coding/SKILL.md
                compatibility:
                  ae_sdd_version: ">=3.5.0"
                tags:
                  - tdd
                  - ddd
              - name: finance-coding
                type: skill-new
                version: 0.1.0
                description: Finance coding
                provides: finance-coding-skill
                path: ./finance/SKILL.md
                dependencies:
                  - my-coding
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d["schema_version"], 1)
        self.assertIn("complex test registry", d["description"])
        self.assertEqual(len(d["plugins"]), 2)

        p0 = d["plugins"][0]
        self.assertEqual(p0["name"], "my-coding")
        self.assertEqual(p0["type"], "skill-override")
        self.assertEqual(p0["replaces"], "source/skills/phase2-coding/coding-skill.md")
        self.assertEqual(p0["compatibility"], {"ae_sdd_version": ">=3.5.0"})
        self.assertEqual(p0["tags"], ["tdd", "ddd"])

        p1 = d["plugins"][1]
        self.assertEqual(p1["name"], "finance-coding")
        self.assertEqual(p1["type"], "skill-new")
        self.assertEqual(p1["provides"], "finance-coding-skill")
        self.assertEqual(p1["dependencies"], ["my-coding"])

    def test_doc_separator_ignored(self):
        text = textwrap.dedent("""\
            ---
            schema_version: 1
            name: foo
            ---
        """)
        d = plugin_loader._parse_yaml_subset(text)
        self.assertEqual(d, {"schema_version": 1, "name": "foo"})

    # ─── flow style（🆕 治 E3：plugins: [] 被解析成 str '[]'）─────────────────

    def test_flow_empty_list(self):
        """plugins: [] 应解析为空 list，不是 str '[]'（E3 回归）"""
        d = plugin_loader._parse_yaml_subset("plugins: []\n")
        self.assertEqual(d["plugins"], [])
        self.assertIsInstance(d["plugins"], list)

    def test_flow_list_of_scalars(self):
        """[a, b, c] → list（元素递归 coerce）"""
        d = plugin_loader._parse_yaml_subset("items: [a, b, 3]\n")
        self.assertEqual(d["items"], ["a", "b", 3])

    def test_flow_empty_dict(self):
        """meta: {} → 空 dict，不是 str '{}'"""
        d = plugin_loader._parse_yaml_subset("meta: {}\n")
        self.assertEqual(d["meta"], {})
        self.assertIsInstance(d["meta"], dict)

    def test_flow_dict_inline(self):
        """{k: v} → dict（值递归 coerce）"""
        d = plugin_loader._parse_yaml_subset('compat: {ae_sdd_version: ">=3.5.0"}\n')
        self.assertEqual(d["compat"], {"ae_sdd_version": ">=3.5.0"})


# ====================================================
# Single-layer registry load tests
# ====================================================

class TestLoadRegistry(unittest.TestCase):
    """load_registry unit tests."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_nonexistent_registry(self):
        rl = plugin_loader.load_registry(
            self.tmp / "nonexistent.yaml",
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertFalse(rl.exists)
        self.assertEqual(len(rl.plugins), 0)
        self.assertEqual(len(rl.errors), 0)

    def test_valid_registry_with_plugins(self):
        # Build an external SKILL file so the path validation passes
        skill_file = self.tmp / "plugins" / "my-coding" / "SKILL.md"
        write_file(skill_file, "# My Coding\n")

        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            description: test
            plugins:
              - name: my-coding
                type: skill-override
                version: 0.1.0
                description: My coding
                replaces: source/skills/coding-skill.md
                path: ./my-coding/SKILL.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertTrue(rl.exists)
        self.assertEqual(len(rl.errors), 0, msg=f"errors: {rl.errors}")
        self.assertEqual(len(rl.plugins), 1)
        self.assertEqual(rl.plugins[0].name, "my-coding")
        self.assertEqual(rl.plugins[0].layer, plugin_loader.LAYER_PROJECT)
        self.assertEqual(rl.plugins[0].layer_label, "L1-project")
        self.assertEqual(rl.plugins[0].resolved_path, skill_file.resolve())

    def test_invalid_schema_version(self):
        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, "schema_version: 99\nplugins: []\n")

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertEqual(len(rl.errors), 1)
        self.assertIn("schema_version", rl.errors[0])

    def test_duplicate_name_in_same_layer(self):
        skill_file = self.tmp / "plugins" / "SKILL.md"
        write_file(skill_file, "# X\n")

        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: dup
                type: skill-override
                version: 0.1.0
                description: a
                replaces: source/skills/a.md
                path: ./SKILL.md
              - name: dup
                type: skill-override
                version: 0.1.0
                description: b
                replaces: source/skills/b.md
                path: ./SKILL.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        # The second occurrence of 'dup' should trigger a duplicate-name error
        dup_errors = [e for e in rl.errors if "dup" in e and ("duplicate" in e.lower() or "重复" in e)]
        self.assertGreaterEqual(len(dup_errors), 1, msg=f"errors: {rl.errors}")

    def test_duplicate_replaces_in_same_layer(self):
        skill_file_a = self.tmp / "plugins" / "a.md"
        skill_file_b = self.tmp / "plugins" / "b.md"
        write_file(skill_file_a, "# A\n")
        write_file(skill_file_b, "# B\n")

        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: a-plugin
                type: skill-override
                version: 0.1.0
                description: a
                replaces: source/skills/coding-skill.md
                path: ./a.md
              - name: b-plugin
                type: skill-override
                version: 0.1.0
                description: b
                replaces: source/skills/coding-skill.md
                path: ./b.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        dup_errors = [e for e in rl.errors if "replaces" in e and ("multiple" in e.lower() or "duplic" in e.lower() or "repeated" in e.lower() or "重复" in e or "多次" in e)]
        self.assertGreaterEqual(len(dup_errors), 1, msg=f"errors: {rl.errors}")

    def test_path_with_dotdot_blocked(self):
        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: bad
                type: skill-override
                version: 0.1.0
                description: bad
                replaces: source/skills/coding-skill.md
                path: ../../etc/passwd
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertEqual(len(rl.errors), 1)
        self.assertIn("..", rl.errors[0])

    def test_missing_required_field(self):
        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: skill-override
                # version missing
                description: foo
                replaces: source/skills/foo.md
                path: ./foo.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertGreater(len(rl.errors), 0)
        self.assertTrue(any("version" in e for e in rl.errors))

    def test_path_nonexistent(self):
        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: skill-override
                version: 0.1.0
                description: foo
                replaces: source/skills/foo.md
                path: ./nonexistent.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertEqual(len(rl.errors), 1)
        self.assertTrue("存在" in rl.errors[0] or "exist" in rl.errors[0].lower())

    def test_invalid_type(self):
        skill_file = self.tmp / "plugins" / "SKILL.md"
        write_file(skill_file, "# X\n")

        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: bogus-type
                version: 0.1.0
                description: foo
                path: ./SKILL.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertEqual(len(rl.errors), 1)
        self.assertIn("type", rl.errors[0])

    def test_skill_override_requires_replaces(self):
        skill_file = self.tmp / "plugins" / "SKILL.md"
        write_file(skill_file, "# X\n")

        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: skill-override
                version: 0.1.0
                description: foo
                # replaces missing
                path: ./SKILL.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertGreater(len(rl.errors), 0)
        self.assertTrue(any("replaces" in e for e in rl.errors))

    def test_skill_new_requires_provides(self):
        skill_file = self.tmp / "plugins" / "SKILL.md"
        write_file(skill_file, "# X\n")

        registry = self.tmp / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: skill-new
                version: 0.1.0
                description: foo
                # provides missing
                path: ./SKILL.md
        """))

        rl = plugin_loader.load_registry(
            registry,
            layer=plugin_loader.LAYER_PROJECT,
            layer_label="L1-project",
        )
        self.assertGreater(len(rl.errors), 0)
        self.assertTrue(any("provides" in e for e in rl.errors))


# ====================================================
# Three-layer load + conflict detection
# ====================================================

class TestResolveSkill(unittest.TestCase):
    """resolve_skill unit tests."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        # project layer
        self.ade_sdd = self.tmp / "project" / ".ae-sdd"
        # master layer (repo root)
        self.master = self.tmp / "master"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_no_layers_returns_builtin_fallback(self):
        result = plugin_loader.resolve_skill(
            target="source/skills/phase2-coding/coding-skill.md",
            ade_sdd=self.ade_sdd,
            master=self.master,
        )
        self.assertIsNone(result.resolved_path)
        self.assertEqual(result.layer, plugin_loader.LAYER_BUILTIN)
        self.assertEqual(result.layer_label, "L0-builtin")

    def test_l1_project_layer_hit(self):
        proj_skill = self.ade_sdd / "plugins" / "SKILL.md"
        write_file(proj_skill, "# Project Coding\n")
        write_file(self.ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: project-coding
                type: skill-override
                version: 0.1.0
                description: project coding
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./SKILL.md
        """))

        result = plugin_loader.resolve_skill(
            target="source/skills/phase2-coding/coding-skill.md",
            ade_sdd=self.ade_sdd,
            master=self.master,
        )
        self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
        self.assertEqual(result.plugin.name, "project-coding")
        self.assertEqual(result.resolved_path, proj_skill.resolve())

    def test_multi_layer_project_wins(self):
        # project layer
        proj_skill = self.ade_sdd / "plugins" / "proj.md"
        write_file(proj_skill, "# Project\n")
        write_file(self.ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: project-coding
                type: skill-override
                version: 0.1.0
                description: project
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./proj.md
        """))

        # global layer
        global_registry = plugin_loader.plugin_registry_path_global()
        global_skill = global_registry.parent / "glob.md"
        write_file(global_skill, "# Global\n")
        write_file(global_registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: global-coding
                type: skill-override
                version: 0.1.0
                description: global
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./glob.md
        """))

        try:
            result = plugin_loader.resolve_skill(
                target="source/skills/phase2-coding/coding-skill.md",
                ade_sdd=self.ade_sdd,
                master=self.master,
            )
            self.assertEqual(result.layer, plugin_loader.LAYER_PROJECT)
            self.assertEqual(result.plugin.name, "project-coding")
            self.assertEqual(len(result.conflicts), 1)
            self.assertEqual(result.conflicts[0].target, "source/skills/phase2-coding/coding-skill.md")
            self.assertEqual(len(result.conflicts[0].losers), 1)
            self.assertEqual(result.conflicts[0].losers[0].name, "global-coding")
        finally:
            # Cleanup global layer
            if global_registry.exists():
                global_registry.unlink()
            if global_skill.exists():
                global_skill.unlink()

    def test_global_layer_only(self):
        global_registry = plugin_loader.plugin_registry_path_global()
        global_skill = global_registry.parent / "glob.md"
        write_file(global_skill, "# Global\n")
        write_file(global_registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: global-coding
                type: skill-override
                version: 0.1.0
                description: global
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./glob.md
        """))

        try:
            result = plugin_loader.resolve_skill(
                target="source/skills/phase2-coding/coding-skill.md",
                ade_sdd=self.ade_sdd,
                master=self.master,
            )
            self.assertEqual(result.layer, plugin_loader.LAYER_GLOBAL)
            self.assertEqual(result.plugin.name, "global-coding")
        finally:
            if global_registry.exists():
                global_registry.unlink()
            if global_skill.exists():
                global_skill.unlink()

    def test_provides_new_skill(self):
        proj_skill = self.ade_sdd / "plugins" / "finance.md"
        write_file(proj_skill, "# Finance\n")
        write_file(self.ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: finance-coding
                type: skill-new
                version: 0.1.0
                description: finance
                provides: finance-coding-skill
                path: ./finance.md
        """))

        result = plugin_loader.resolve_skill(
            target="finance-coding-skill",  # provides key
            ade_sdd=self.ade_sdd,
            master=self.master,
        )
        self.assertIsNotNone(result.plugin, msg=f"result: {result.to_dict()}")
        self.assertEqual(result.plugin.name, "finance-coding")
        self.assertEqual(result.resolved_path, proj_skill.resolve())


# ====================================================
# Conflict detection
# ====================================================

class TestDetectConflicts(unittest.TestCase):
    """detect_conflicts unit tests."""

    def test_no_conflicts(self):
        plugins = [
            plugin_loader.Plugin(name="a", type="skill-override", version="0.1.0",
                                 description="", path="a.md", replaces="x.md",
                                 layer=plugin_loader.LAYER_PROJECT, layer_label="L1"),
            plugin_loader.Plugin(name="b", type="skill-override", version="0.1.0",
                                 description="", path="b.md", replaces="y.md",
                                 layer=plugin_loader.LAYER_GLOBAL, layer_label="L2"),
        ]
        conflicts = plugin_loader.detect_conflicts(plugins)
        self.assertEqual(len(conflicts), 0)

    def test_multi_layer_conflict(self):
        plugins = [
            plugin_loader.Plugin(name="a", type="skill-override", version="0.1.0",
                                 description="", path="a.md", replaces="x.md",
                                 layer=plugin_loader.LAYER_PROJECT, layer_label="L1-project"),
            plugin_loader.Plugin(name="b", type="skill-override", version="0.1.0",
                                 description="", path="b.md", replaces="x.md",
                                 layer=plugin_loader.LAYER_GLOBAL, layer_label="L2-global"),
            plugin_loader.Plugin(name="c", type="skill-override", version="0.1.0",
                                 description="", path="c.md", replaces="x.md",
                                 layer=plugin_loader.LAYER_MASTER, layer_label="L3-master"),
        ]
        conflicts = plugin_loader.detect_conflicts(plugins)
        self.assertEqual(len(conflicts), 1)
        self.assertEqual(conflicts[0].winner.name, "a")  # L1 wins
        self.assertEqual(len(conflicts[0].losers), 2)
        self.assertIn("b", [l.name for l in conflicts[0].losers])
        self.assertIn("c", [l.name for l in conflicts[0].losers])


# ====================================================
# version compatibility
# ====================================================

class TestVersionSatisfies(unittest.TestCase):
    """_version_satisfies unit tests."""

    def test_ge_passes(self):
        self.assertTrue(plugin_loader._version_satisfies("3.5.0", ">=3.5.0"))
        self.assertTrue(plugin_loader._version_satisfies("3.6.0", ">=3.5.0"))
        self.assertTrue(plugin_loader._version_satisfies("4.0.0", ">=3.5.0"))

    def test_ge_fails(self):
        self.assertFalse(plugin_loader._version_satisfies("3.4.3", ">=3.5.0"))
        self.assertFalse(plugin_loader._version_satisfies("3.4.99", ">=3.5.0"))

    def test_exact_match(self):
        self.assertTrue(plugin_loader._version_satisfies("3.5.0", "3.5.0"))
        self.assertFalse(plugin_loader._version_satisfies("3.5.1", "3.5.0"))

    def test_unparseable_passes(self):
        # Unparseable -> pass (fault tolerance)
        self.assertTrue(plugin_loader._version_satisfies("invalid", ">=3.5.0"))
        self.assertTrue(plugin_loader._version_satisfies("3.5.0", "invalid"))


# ====================================================
# validate entry
# ====================================================

class TestValidate(unittest.TestCase):
    """validate unit tests."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.ade_sdd = self.tmp / "project" / ".ae-sdd"
        self.master = self.tmp / "master"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_no_layers_valid(self):
        result = plugin_loader.validate(self.ade_sdd, self.master)
        self.assertTrue(result["valid"])
        self.assertEqual(result["totalPlugins"], 0)
        self.assertEqual(result["totalConflicts"], 0)

    def test_with_valid_plugin(self):
        proj_skill = self.ade_sdd / "plugins" / "SKILL.md"
        write_file(proj_skill, "# X\n")
        write_file(self.ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: my-coding
                type: skill-override
                version: 0.1.0
                description: my
                replaces: source/skills/coding-skill.md
                path: ./SKILL.md
        """))

        result = plugin_loader.validate(self.ade_sdd, self.master)
        self.assertTrue(result["valid"], msg=f"errors: {result['errors']}")
        self.assertEqual(result["totalPlugins"], 1)

    def test_with_invalid_plugin(self):
        registry = self.ade_sdd / "plugins" / "registry.yaml"
        write_file(registry, textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: foo
                type: bogus
                version: 0.1.0
                description: foo
                path: ./SKILL.md
        """))

        result = plugin_loader.validate(self.ade_sdd, self.master)
        self.assertFalse(result["valid"])
        self.assertGreater(len(result["errors"]), 0)


# ====================================================
# B4 增强测试：外挂内容安全扫描分层阻断（规则 #16）
# ====================================================

class TestContentScanLayeredBlocking(unittest.TestCase):
    """🆕 B4：验证 load_registry 的规则 #16 分层阻断策略。

    - L2 全局层：BLOCKER 命中 → 阻断（plugin 不入列 + error）
    - L1 项目层：BLOCKER 命中 → 告警（plugin 仍加载 + warning）
    - L3 仓库根：跳过扫描（无 findings）
    """

    def setUp(self):
        import tempfile
        self.tmp = Path(tempfile.mkdtemp(prefix="pcs-loader-"))
        # L2 全局层：用环境变量重定向到临时目录
        self.global_home = self.tmp / "globalhome"
        self.global_home.mkdir()
        self._old_env = os.environ.get("AE_SDD_GLOBAL_HOME")
        os.environ["AE_SDD_GLOBAL_HOME"] = str(self.global_home)

        self.ade_sdd = self.tmp / "project" / ".ae-sdd"
        self.master = self.tmp / "master"

    def tearDown(self):
        import shutil
        if self._old_env is None:
            os.environ.pop("AE_SDD_GLOBAL_HOME", None)
        else:
            os.environ["AE_SDD_GLOBAL_HOME"] = self._old_env
        shutil.rmtree(self.tmp, ignore_errors=True)

    def _write_dangerous_plugin(self, registry_path: Path, skill_path: Path, name: str):
        """构造一个含 BLOCKER 的外挂（rm -rf /）。"""
        write_file(skill_path, "# Dangerous\n\n```\nrm -rf /\n```\n")
        registry_path.parent.mkdir(parents=True, exist_ok=True)
        write_file(registry_path, textwrap.dedent(f"""\
            schema_version: 1
            plugins:
              - name: {name}
                type: skill-override
                version: 0.1.0
                description: dangerous
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./{skill_path.name}
        """))

    def test_l2_global_blocker_blocks_loading(self):
        """L2 全局层 BLOCKER → plugin 被阻断（不入 plugins + errors 非空）。"""
        global_plugins = self.global_home / ".ae-sdd" / "plugins"
        skill = global_plugins / "evil.md"
        registry = global_plugins / "registry.yaml"
        self._write_dangerous_plugin(registry, skill, "evil-global")

        rl = plugin_loader.load_registry(
            plugin_loader.plugin_registry_path_global(),
            plugin_loader.LAYER_GLOBAL,
            plugin_loader.LAYER_NAMES[plugin_loader.LAYER_GLOBAL],
        )
        # BLOCKER → 不入 plugins
        self.assertEqual(len(rl.plugins), 0)
        # 且产生 error
        self.assertTrue(any("PC-001" in e for e in rl.errors),
                        f"L2 BLOCKER 应产生 error，实际 errors: {rl.errors}")

    def test_l1_project_blocker_warns_not_blocks(self):
        """L1 项目层 BLOCKER → plugin 仍加载，仅 warning。"""
        skill = self.ade_sdd / "plugins" / "evil.md"
        registry = self.ade_sdd / "plugins" / "registry.yaml"
        self._write_dangerous_plugin(registry, skill, "evil-project")

        rl = plugin_loader.load_registry(
            plugin_loader.plugin_registry_path_project(self.ade_sdd),
            plugin_loader.LAYER_PROJECT,
            plugin_loader.LAYER_NAMES[plugin_loader.LAYER_PROJECT],
        )
        # L1 → plugin 仍加载（owner 自负）
        self.assertEqual(len(rl.plugins), 1)
        self.assertEqual(rl.plugins[0].name, "evil-project")
        # 但有 warning
        self.assertTrue(any("PC-001" in w for w in rl.warnings),
                        f"L1 BLOCKER 应产生 warning，实际 warnings: {rl.warnings}")

    def test_clean_plugin_loads_everywhere(self):
        """安全外挂在 L1/L2 都正常加载（无 error）。"""
        skill = self.ade_sdd / "plugins" / "safe.md"
        write_file(skill, "# Safe Coding\n正常编码指南\n")
        write_file(self.ade_sdd / "plugins" / "registry.yaml", textwrap.dedent("""\
            schema_version: 1
            plugins:
              - name: safe-coding
                type: skill-override
                version: 0.1.0
                description: safe
                replaces: source/skills/phase2-coding/coding-skill.md
                path: ./safe.md
        """))

        rl = plugin_loader.load_registry(
            plugin_loader.plugin_registry_path_project(self.ade_sdd),
            plugin_loader.LAYER_PROJECT,
            plugin_loader.LAYER_NAMES[plugin_loader.LAYER_PROJECT],
        )
        self.assertEqual(len(rl.plugins), 1)
        # 安全内容不产生 BLOCKER/WARN error（INFO 不入列）
        self.assertEqual(len(rl.errors), 0)


if __name__ == "__main__":
    unittest.main()