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
def sync_once(repo_root: Path, do_build: bool, do_install: bool) -> bool:
    """执行一次 build + install；返回是否成功"""
    if do_build:
        step("Build: source/ → dist/ae-sdd/")
        rc = run_script(repo_root / "scripts" / "build_dist.py")
        if rc != 0:
            err("build_dist.py 执行失败")
            return False

    if do_install:
        step("Install: dist/ae-sdd/ → 本地 Claude skills")
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
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent

    if args.uninstall:
        rc = run_script(repo_root / "scripts" / "install.py", "--uninstall")
        return rc

    do_build   = not args.install_only
    do_install = not args.build_only

    if args.watch:
        watch_mode(repo_root, do_build, do_install, args.interval)
        return 0
    else:
        ok_run = sync_once(repo_root, do_build, do_install)
        return 0 if ok_run else 1


if __name__ == "__main__":
    sys.exit(main())
