#!/usr/bin/env python3
"""
dev_sync.py — ae-sdd 开发者工具

🆕 v3.0.1 跨平台化（2026-06-18）：用 Python 替代 bash，零外部依赖（仅标准库）。

默认：build + install 一步到位。

用法:
    python scripts/dev_sync.py                # 单次 build + install
    python scripts/dev_sync.py --build-only   # 只 build
    python scripts/dev_sync.py --install-only # 只 install（假设 dist/ 已存在）
    python scripts/dev_sync.py --watch        # 监听 source/、tools/、scripts/ 变化自动 build + install
    python scripts/dev_sync.py --uninstall    # 卸载本地安装
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Callable


# ─── 颜色（ANSI） ────────────────────────────────────────────────────────────
def _supports_color() -> bool:
    return sys.stdout.isatty()


if _supports_color():
    C_BLUE   = "\033[0;34m"
    C_GREEN  = "\033[0;32m"
    C_YELLOW = "\033[0;33m"
    C_RED    = "\033[0;31m"
    C_RESET  = "\033[0m"
else:
    C_BLUE = C_GREEN = C_YELLOW = C_RED = C_RESET = ""


def info(msg: str) -> None: print(f"{C_BLUE}ℹ️  {msg}{C_RESET}")
def ok(msg: str)   -> None: print(f"{C_GREEN}✅ {msg}{C_RESET}")
def warn(msg: str) -> None: print(f"{C_YELLOW}⚠  {msg}{C_RESET}")
def err(msg: str)  -> None: print(f"{C_RED}❌ {msg}{C_RESET}", file=sys.stderr)
def step(msg: str) -> None: print(f"\n{C_BLUE}== {msg} =={C_RESET}")


# ─── 仓库根残留清理 ────────────────────────────────────────────────────────────
# 清理 gitignore 的开发期产物，避免仓库根堆积噪音：
#   - `nul` 空文件：Windows `> nul` 误用产物（gitignore 已忽略但物理文件残留）
#   - `plugins/ae-sdd.bak.*` 旧备份目录：build/install 旁路产物，保留最近 N 个
# 注：只动 git 未跟踪的产物（git ls-files 验证过），不动 git 跟踪文件。
REPO_BAK_KEEP_DEFAULT = 1  # 仓库内 plugins/ae-sdd.bak.* 保留最近 N 个


def _clean_stray_nul(repo_root: Path) -> bool:
    """删除仓库根的 `nul` 空文件（Windows `> nul` 误用产物）。

    Windows 下 `nul` 是保留设备名，Path.exists()/is_file() 会把它当虚拟设备返回 True，
    但物理上并无文件。用 os.path.isfile() 判定真实文件（设备名返回 False），
    只清理真实存在的 nul 文件（跨平台场景，如 git-bash 在非 Windows 产生的残留）。
    返回是否无需清理或清理成功。
    """
    nul = repo_root / "nul"
    # isfile 对 Windows 设备名 nul 返回 False（无真实文件）；真实文件返回 True
    if not os.path.isfile(nul):
        return True  # 无真实 nul 文件，无需清理

    # 跳过 git 跟踪文件（防御性，正常 nul 不会被跟踪）
    try:
        tracked = subprocess.run(
            ["git", "-C", str(repo_root), "ls-files", "nul"],
            capture_output=True, text=True, encoding="utf-8",
        ).stdout.strip()
        if tracked:
            warn(f"nul 被 git 跟踪，跳过清理：{tracked}")
            return False
    except Exception:
        pass  # git 不可用则继续尝试删除

    try:
        os.unlink(nul)
        ok("已清理仓库根残留: nul")
        return True
    except OSError as e:
        # Windows 设备名场景兜底：cmd /c del 带 \\?\ 长路径前缀绕过设备名解析
        long_path = "\\\\?\\" + str(nul.resolve())
        r = subprocess.run(["cmd", "/c", "del", "/f", "/q", long_path],
                           capture_output=True, text=True)
        if not os.path.isfile(nul):
            ok("已清理仓库根残留: nul (via cmd del)")
            return True
        warn(f"清理 nul 失败（可能为 Windows 设备名，非真实文件）: {e}")
        return False


def _clean_repo_baks(repo_root: Path, keep: int = REPO_BAK_KEEP_DEFAULT) -> dict:
    """清理仓库内 plugins/ae-sdd.bak.* 旧备份目录，保留最近 keep 个。

    排序按目录名（.bak.YYYYMMDDHHMMSS）字典序降序，新→旧。
    keep<0 表示不清理；keep=0 表示全清。
    返回 {removed: [...], kept: [...]}。
    """
    if keep < 0:
        return {"removed": [], "kept": []}
    plugins = repo_root / "plugins"
    if not plugins.is_dir():
        return {"removed": [], "kept": []}
    baks = sorted(
        (p for p in plugins.glob("ae-sdd.bak.*") if p.is_dir()),
        key=lambda p: p.name,
        reverse=True,
    )
    if len(baks) <= keep:
        return {"removed": [], "kept": [b.name for b in baks]}

    to_remove = baks[keep:]
    removed = []
    for old in to_remove:
        try:
            shutil.rmtree(old)
            warn(f"清理旧备份: plugins/{old.name}")
            removed.append(old.name)
        except OSError as e:
            warn(f"清理失败 plugins/{old.name}: {e}")
    if removed:
        info(f"已清理 {len(removed)} 个仓库内旧备份（保留最近 {keep} 个）")
    return {"removed": removed, "kept": [b.name for b in baks[:keep]]}


def clean_repo_root_strays(repo_root: Path, keep_bak: int = REPO_BAK_KEEP_DEFAULT) -> dict:
    """清理仓库根残留：nul 文件 + plugins/ae-sdd.bak.* 旧备份。

    build 前调用，保证分发包干净。返回 {nul_cleaned, baks}。
    """
    step("清理仓库根残留")
    nul_ok = _clean_stray_nul(repo_root)
    baks = _clean_repo_baks(repo_root, keep=keep_bak)
    return {"nul_cleaned": nul_ok, "baks": baks}


# ─── 工具函数 ────────────────────────────────────────────────────────────────
def _max_mtime(root: Path) -> float:
    """返回 root 下所有文件的最大 mtime（秒）"""
    if not root.is_dir():
        return 0.0
    return max(
        (p.stat().st_mtime for p in root.rglob("*") if p.is_file()),
        default=0.0,
    )


def _watched_mtime(roots: list[Path]) -> float:
    """返回多个监听根的最大 mtime。"""
    return max((_max_mtime(root) for root in roots), default=0.0)


def run_script(script: Path, *args: str) -> int:
    """调 Python 脚本"""
    cmd = [sys.executable, str(script), *args]
    result = subprocess.run(cmd)
    return result.returncode


# ─── 同步函数 ────────────────────────────────────────────────────────────────
def sync_once(repo_root: Path, do_build: bool, do_install: bool, do_clean: bool = True) -> bool:
    """执行一次 build + install；返回是否成功"""
    if do_clean:
        clean_repo_root_strays(repo_root)

    if do_build:
        step("Build: source/ → dist/ae-sdd/")
        rc = run_script(repo_root / "scripts" / "build_dist.py")
        if rc != 0:
            err("build_dist.py 执行失败")
            return False

    if do_install:
        step("Install: dist/ae-sdd/ → 本地 Agent skills")
        rc = run_script(repo_root / "scripts" / "install.py")
        if rc != 0:
            err("install.py 执行失败")
            return False

    ok("dev-sync 完成")
    return True


def watch_mode(repo_root: Path, do_build: bool, do_install: bool, interval: int = 2) -> None:
    """Polling 监听母版与运行时工具变化（每 N 秒），变化则触发 sync。"""
    watched_roots = [repo_root / "source", repo_root / "tools", repo_root / "scripts"]
    watched_desc = ", ".join(str(root) for root in watched_roots if root.exists())
    info(f"监听模式: 关注 {watched_desc} 变化（每 {interval} 秒检查，按 Ctrl+C 停止）")
    if not sync_once(repo_root, do_build, do_install):
        sys.exit(1)

    last_mtime = _watched_mtime(watched_roots)
    try:
        while True:
            time.sleep(interval)
            current = _watched_mtime(watched_roots)
            if current > last_mtime:
                info(f"检测到母版/工具变化（mtime {last_mtime:.1f} → {current:.1f}）")
                last_mtime = current
                if not sync_once(repo_root, do_build, do_install):
                    err("sync 失败，但继续监听")
    except KeyboardInterrupt:
        info("已停止监听")


# ─── 主流程 ──────────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="dev_sync: ae-sdd 开发者工具（build + install 组合）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--build-only",   action="store_true", help="只 build，不 install")
    parser.add_argument("--install-only", action="store_true", help="只 install，不 build")
    parser.add_argument("--watch",        action="store_true", help="监听 source/ 变化自动 sync")
    parser.add_argument("--uninstall",    action="store_true", help="卸载本地安装")
    parser.add_argument("--interval",     type=int, default=2,  help="--watch 模式检查间隔（秒）")
    parser.add_argument("--clean-only",   action="store_true", help="只清理仓库根残留（nul + plugins 旧备份），不 build/install")
    parser.add_argument("--no-clean",     action="store_true", help="跳过仓库根清理（默认 sync 前会清理）")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent

    if args.uninstall:
        rc = run_script(repo_root / "scripts" / "install.py", "--uninstall")
        return rc

    if args.clean_only:
        clean_repo_root_strays(repo_root)
        return 0

    do_build   = not args.install_only
    do_install = not args.build_only
    do_clean   = not args.no_clean

    if args.watch:
        watch_mode(repo_root, do_build, do_install, args.interval)
        return 0
    else:
        ok_run = sync_once(repo_root, do_build, do_install, do_clean=do_clean)
        return 0 if ok_run else 1


if __name__ == "__main__":
    sys.exit(main())
