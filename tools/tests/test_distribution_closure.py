"""
test_distribution_closure.py — 🆕 v3.4.0 分发闭环单元测试

覆盖：
1. paths.compare_versions 版本对比
2. update_graph.check_uc07_distribution_closure UC-07 检查
3. install.py --target-path 参数
4. init.py _read_master_version 解析母版 SKILL.md frontmatter
5. prompt_inject._read_project_master_version 解析业务仓 config.yaml

运行：python -m pytest tools/tests/test_distribution_closure.py -v
       python tools/tests/test_distribution_closure.py        # 直接跑
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent


def test_compare_versions_equal():
    """compare_versions 相等版本返回 None"""
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    from paths import compare_versions
    assert compare_versions("3.4.0", "3.4.0") is None
    print("✅ test_compare_versions_equal")


def test_compare_versions_older():
    """compare_versions 旧版本返回漂移文本"""
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    from paths import compare_versions
    result = compare_versions("3.2.3", "3.4.0")
    assert result is not None
    assert "3.2.3" in result and "3.4.0" in result
    print(f"✅ test_compare_versions_older: {result}")


def test_compare_versions_newer_no_warn():
    """compare_versions 新版本不告警（开发版允许）"""
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    from paths import compare_versions
    assert compare_versions("4.0.0", "3.4.0") is None
    print("✅ test_compare_versions_newer_no_warn")


def test_compare_versions_none():
    """compare_versions None 输入"""
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    from paths import compare_versions
    result = compare_versions(None, "3.4.0")
    assert result is not None and "unknown" in result
    print(f"✅ test_compare_versions_none: {result}")


def test_uc07_distribution_closure():
    """UC-07 分发闭环检查：在母版仓库应 PASS"""
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    import update_graph
    result = update_graph.check_uc07_distribution_closure(REPO_ROOT)
    assert result.pass_, f"UC-07 应 PASS，实际：{result.message}"
    print(f"✅ test_uc07_distribution_closure: {result.message}")


def test_install_py_target_path():
    """install.py --target-path 应可解析"""
    result = subprocess.run(
        [sys.executable, str(REPO_ROOT / "scripts" / "install.py"), "--help"],
        capture_output=True, text=True, timeout=10,
    )
    assert "--target-path" in result.stdout, "install.py --help 应含 --target-path"
    assert "--quiet" in result.stdout, "install.py --help 应含 --quiet"
    print("✅ test_install_py_target_path")


def test_install_print_usage_survives_gbk_console():
    """A successful Windows install must not fail while printing status glyphs."""
    code = (
        "import importlib.util; "
        f"p=r'{REPO_ROOT / 'scripts' / 'install.py'}'; "
        "s=importlib.util.spec_from_file_location('ae_sdd_install', p); "
        "m=importlib.util.module_from_spec(s); s.loader.exec_module(m); m.print_usage()"
    )
    env = dict(os.environ)
    env["PYTHONIOENCODING"] = "gbk:strict"
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        check=False,
        env=env,
    )
    assert result.returncode == 0, result.stderr.decode("gbk", errors="replace")


def test_windows_sibling_cli_launcher_is_distributed(tmp_path):
    """The compiled package must carry a full-path-safe Windows launcher."""
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import build_dist

    dist = tmp_path / "dist" / "ae-sdd"
    build_dist._copy_tools_to_dist(REPO_ROOT, dist)

    launcher = dist / "tools" / "bin" / "ae-sdd.cmd"
    assert launcher.is_file(), "dist must include tools/bin/ae-sdd.cmd"
    text = launcher.read_text(encoding="utf-8")
    assert '"%~dp0ae-sdd"' in text
    assert "%*" in text


def test_windows_sibling_cli_launcher_preserves_exit_status(tmp_path):
    """The launcher must return the adjacent Python CLI status unchanged."""
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import build_dist

    dist = tmp_path / "dist" / "ae-sdd"
    build_dist._copy_tools_to_dist(REPO_ROOT, dist)

    launcher = dist / "tools" / "bin" / "ae-sdd.cmd"
    text = launcher.read_text(encoding="utf-8").lower()
    assert "exit /b %errorlevel%" in text


def test_init_read_master_version():
    """init.py _read_master_version 应能解析 source/SKILL.md frontmatter

    版本号不写死——以 paths.MASTER_VERSION（SSOT）为预期值，避免 bump 版本后测试过时。
    """
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import init
    # 预期值取 tools/lib/paths.py 的 MASTER_VERSION（单一真相源，bump 时自动同步）
    sys.path.insert(0, str(REPO_ROOT / "tools" / "lib"))
    import paths as paths_mod
    expected = paths_mod.MASTER_VERSION
    version = init._read_master_version(REPO_ROOT / "source")
    assert version == expected, f"应读到 {expected}，实际 {version}"
    print(f"✅ test_init_read_master_version: {version}")


def test_post_commit_hook_installed():
    """post-commit hook 应存在 + 可执行 + hooksPath 正确"""
    hook = REPO_ROOT / ".githooks" / "post-commit"
    assert hook.is_file(), ".githooks/post-commit 应存在"
    import os
    assert os.access(str(hook), os.X_OK), ".githooks/post-commit 应可执行"
    r = subprocess.run(
        ["git", "config", "--get", "core.hooksPath"],
        cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=5,
    )
    assert r.stdout.strip() == ".githooks", f"core.hooksPath 应=.githooks，实际 {r.stdout!r}"
    print("✅ test_post_commit_hook_installed")


def test_changelog_only_skipped():
    """post-commit hook 跳过滤：CHANGELOG-only 提交应静默退出"""
    # 由于 git 实际操作较重，这里只验证 hook 脚本包含过滤逻辑
    hook = REPO_ROOT / ".githooks" / "post-commit"
    text = hook.read_text(encoding="utf-8")
    assert "source/CHANGELOG/" in text, "hook 应过滤 CHANGELOG-only"
    assert "SKIP_AE_SDD_HOOK" in text, "hook 应支持 SKIP_AE_SDD_HOOK 旁路"
    print("✅ test_changelog_only_skipped")


def main() -> int:
    """直接跑（无 pytest）"""
    tests = [
        test_compare_versions_equal,
        test_compare_versions_older,
        test_compare_versions_newer_no_warn,
        test_compare_versions_none,
        test_uc07_distribution_closure,
        test_install_py_target_path,
        test_init_read_master_version,
        test_post_commit_hook_installed,
        test_changelog_only_skipped,
    ]
    failed = 0
    for t in tests:
        try:
            t()
        except AssertionError as e:
            print(f"❌ {t.__name__}: {e}")
            failed += 1
        except Exception as e:
            print(f"💥 {t.__name__}: {type(e).__name__}: {e}")
            failed += 1
    print()
    print(f"📊 {len(tests) - failed}/{len(tests)} passed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
