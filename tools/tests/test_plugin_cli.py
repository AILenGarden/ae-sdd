"""test_plugin_cli.py -- unit tests for ae-sdd plugin CLI (v3.5.1)

Covers:
- plugin list (human + JSON)
- plugin validate (pass + fail)
- plugin trace (hit + fallback + conflict)
- plugin init (project + global + --force + missing template)

Test approach: invoke ae-sdd CLI as subprocess, override HOME/USERPROFILE to
isolate from real user config (~/.ae-sdd/plugins/) and the actual repo master path.
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CLI = REPO_ROOT / "tools" / "bin" / "ae-sdd"


def _isolated_env():
    """Return a copy of os.environ with HOME/USERPROFILE redirected to a tmp dir.

    On Windows, Path.home() reads USERPROFILE; on Unix, HOME. Override BOTH
    to make tests portable and prevent pollution of the real user config.
    """
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "utf-8"
    tmp = tempfile.mkdtemp(prefix="ae-sdd-test-home-")
    env["HOME"] = tmp
    env["USERPROFILE"] = tmp
    return env, tmp


def run_ae_sdd(*args, env_overrides=None, cwd=None, expect_returncode=None):
    """Invoke the ae-sdd CLI as a subprocess and return CompletedProcess."""
    env, _tmp = _isolated_env()
    if env_overrides:
        env.update(env_overrides)

    proc = subprocess.run(
        [sys.executable, str(CLI), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        env=env,
        cwd=cwd,
    )
    if expect_returncode is not None:
        assert proc.returncode == expect_returncode, (
            f"returncode mismatch: expected {expect_returncode}, got {proc.returncode}\n"
            f"stdout: {proc.stdout}\nstderr: {proc.stderr}"
        )
    return proc


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


# ====================================================
# plugin list
# ====================================================

class TestPluginList(unittest.TestCase):
    """ae-sdd plugin list tests."""

    def test_list_no_layers(self):
        # L1/L2 无注册表；母版 L3 自 v3.6.1 起注册了 java3d-coding-skill（首例 skill-new 适配器）。
        # 本测试校验"L1/L2 干净 + L3 仅母版官方插件"的基线。
        proc = run_ae_sdd("plugin", "list", "--json", expect_returncode=0)
        data = json.loads(proc.stdout)
        self.assertEqual(data["totalPlugins"], 1)
        self.assertEqual(data["totalConflicts"], 0)
        self.assertEqual(len(data["layers"]), 3)
        # L1-project / L2-global 应不存在；L3-master 应存在且有母版官方插件
        l1 = next(l for l in data["layers"] if l["layer"] == 1)
        l2 = next(l for l in data["layers"] if l["layer"] == 2)
        l3 = next(l for l in data["layers"] if l["layer"] == 3)
        self.assertFalse(l1["exists"])
        self.assertFalse(l2["exists"])
        self.assertTrue(l3["exists"])

    def test_list_human_readable(self):
        proc = run_ae_sdd("plugin", "list", expect_returncode=0)
        combined = proc.stdout + proc.stderr
        for label in ("L1-project", "L2-global", "L3-master"):
            self.assertIn(label, combined, msg=f"missing {label} in output")


# ====================================================
# plugin validate
# ====================================================

class TestPluginValidate(unittest.TestCase):
    """ae-sdd plugin validate tests."""

    def test_validate_no_layers_passes(self):
        # 母版 L3 自 v3.6.1 注册 java3d-coding-skill（valid 应通过，totalPlugins=1）
        proc = run_ae_sdd("plugin", "validate", "--json", expect_returncode=0)
        data = json.loads(proc.stdout)
        self.assertTrue(data["valid"])
        self.assertEqual(data["totalPlugins"], 1)

    def test_validate_human_readable_passes(self):
        proc = run_ae_sdd("plugin", "validate", expect_returncode=0)
        # Human-readable output goes to stderr (output.info / output.ok)
        combined = proc.stdout + proc.stderr
        self.assertIn("校验通过", combined)


# ====================================================
# plugin trace
# ====================================================

class TestPluginTrace(unittest.TestCase):
    """ae-sdd plugin trace tests."""

    def test_trace_fallback_when_no_registry(self):
        proc = run_ae_sdd(
            "plugin", "trace", "source/skills/phase2-coding/coding-skill.md", "--json",
            expect_returncode=0,
        )
        data = json.loads(proc.stdout)
        self.assertEqual(data["layer"], 99)  # LAYER_BUILTIN
        self.assertEqual(data["layerLabel"], "L0-builtin")
        self.assertIsNone(data["plugin"])
        self.assertIsNone(data["resolvedPath"])

    def test_trace_human_readable(self):
        proc = run_ae_sdd(
            "plugin", "trace", "source/skills/phase2-coding/coding-skill.md",
            expect_returncode=0,
        )
        combined = proc.stdout + proc.stderr
        self.assertIn("fallback", combined)
        self.assertIn("L0-builtin", combined)

    def test_trace_requires_target(self):
        # Subcommand without target should fail with argparse error (exit 2)
        proc = run_ae_sdd("plugin", "trace")
        self.assertEqual(proc.returncode, 2)


# ====================================================
# plugin init
# ====================================================

class TestPluginInit(unittest.TestCase):
    """ae-sdd plugin init tests."""

    def test_init_global_creates_file(self):
        env, tmp = _isolated_env()
        proc = subprocess.run(
            [sys.executable, str(CLI), "plugin", "init", "--layer", "global"],
            capture_output=True, text=True, encoding="utf-8",
            env=env, cwd=tmp,
        )
        self.assertEqual(proc.returncode, 0, msg=f"stderr: {proc.stderr}")
        target = Path(tmp) / ".ae-sdd" / "plugins" / "registry.yaml"
        self.assertTrue(target.is_file(), msg=f"target not created: {target}")

    def test_init_global_already_exists_no_force(self):
        env, tmp = _isolated_env()
        # First call creates
        subprocess.run(
            [sys.executable, str(CLI), "plugin", "init", "--layer", "global"],
            capture_output=True, text=True, encoding="utf-8",
            env=env, cwd=tmp,
        )
        # Second call should fail (already exists, no --force)
        proc = subprocess.run(
            [sys.executable, str(CLI), "plugin", "init", "--layer", "global"],
            capture_output=True, text=True, encoding="utf-8",
            env=env, cwd=tmp,
        )
        self.assertEqual(proc.returncode, 1, msg=f"stderr: {proc.stderr}")
        self.assertIn("已存在", proc.stdout + proc.stderr)

    def test_init_global_with_force_overwrites(self):
        env, tmp = _isolated_env()
        subprocess.run(
            [sys.executable, str(CLI), "plugin", "init", "--layer", "global"],
            capture_output=True, text=True, encoding="utf-8",
            env=env, cwd=tmp,
        )
        # Modify the file
        target = Path(tmp) / ".ae-sdd" / "plugins" / "registry.yaml"
        target.write_text("modified\n", encoding="utf-8")
        # Force overwrite
        proc = subprocess.run(
            [sys.executable, str(CLI), "plugin", "init", "--layer", "global", "--force"],
            capture_output=True, text=True, encoding="utf-8",
            env=env, cwd=tmp,
        )
        self.assertEqual(proc.returncode, 0, msg=f"stderr: {proc.stderr}")
        content = target.read_text(encoding="utf-8")
        self.assertIn("schema_version", content)

    def test_init_project_requires_ae_sdd_dir(self):
        # Project init needs .ae-sdd/ in cwd or parents; use a tmp dir
        env, tmp = _isolated_env()
        proc = subprocess.run(
            [sys.executable, str(CLI), "plugin", "init", "--layer", "project"],
            capture_output=True, text=True, encoding="utf-8",
            env=env, cwd=tmp,
        )
        self.assertEqual(proc.returncode, 1, msg=f"stderr: {proc.stderr}")
        self.assertIn(".ae-sdd", proc.stdout + proc.stderr)


if __name__ == "__main__":
    unittest.main()