#!/usr/bin/env python3
"""
install_cli.py — 把 ae-sdd CLI 安装到系统 PATH

问题背景：
  hook 配置里写的是 "ae-sdd gate-intercept"，
  但 tools/bin/ae-sdd 不在系统 PATH，导致三个 hook 全部静默失效。

本脚本解决方案（按优先级）：
  1. 在 ~/.local/bin/ae-sdd 创建 shim 脚本（Unix）
  2. 在 ~/AppData/Local/ae-sdd/ae-sdd.cmd 创建 Windows 批处理 shim（Windows）
  3. 创建 Python 包装器（跨平台兜底）

用法：
  python scripts/install_cli.py              # 自动检测平台安装
  python scripts/install_cli.py --check      # 只检查当前状态
  python scripts/install_cli.py --uninstall  # 移除 shim
"""
from __future__ import annotations

import argparse
import os
import platform
import shutil
import stat
import sys
from pathlib import Path


# ─── 颜色 ────────────────────────────────────────────────────────────────────
def _supports_color() -> bool:
    return sys.stdout.isatty()

if _supports_color():
    C_GREEN  = "\033[0;32m"
    C_YELLOW = "\033[1;33m"
    C_RED    = "\033[0;31m"
    C_RESET  = "\033[0m"
else:
    C_GREEN = C_YELLOW = C_RED = C_RESET = ""

def ok(msg: str)   -> None: print(f"{C_GREEN}✅ {msg}{C_RESET}")
def warn(msg: str) -> None: print(f"{C_YELLOW}⚠  {msg}{C_RESET}")
def err(msg: str)  -> None: print(f"{C_RED}❌ {msg}{C_RESET}", file=sys.stderr)
def info(msg: str) -> None: print(f"   {msg}")


def _repo_root() -> Path:
    """定位 ae-sdd 仓库根（包含 tools/bin/ae-sdd 的目录）"""
    script_dir = Path(__file__).resolve().parent
    candidates = [
        script_dir.parent,       # scripts/ 的上级 = 仓库根
        script_dir,
        Path.cwd(),
    ]
    for cand in candidates:
        if (cand / "tools" / "bin" / "ae-sdd").is_file():
            return cand
    return script_dir.parent


def _cli_target() -> Path:
    return _repo_root() / "tools" / "bin" / "ae-sdd"


def _python_exe() -> str:
    """返回当前 Python 解释器路径"""
    return sys.executable


# ─── Unix shim ───────────────────────────────────────────────────────────────
_UNIX_SHIM_DIRS: list[Path] = [
    Path.home() / ".local" / "bin",
    Path("/usr/local/bin"),
]

def _unix_shim_content(cli_path: Path, python_exe: str) -> str:
    return (
        "#!/bin/sh\n"
        f'exec "{python_exe}" "{cli_path}" "$@"\n'
    )


def install_unix(cli_path: Path) -> int:
    python_exe = _python_exe()
    content = _unix_shim_content(cli_path, python_exe)

    for shim_dir in _UNIX_SHIM_DIRS:
        try:
            shim_dir.mkdir(parents=True, exist_ok=True)
            shim = shim_dir / "ae-sdd"
            shim.write_text(content, encoding="utf-8")
            shim.chmod(shim.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
            ok(f"shim 已创建: {shim}")
            # 检查是否在 PATH
            if str(shim_dir) not in os.environ.get("PATH", ""):
                warn(f"{shim_dir} 可能不在 PATH 中")
                warn(f"请在 ~/.bashrc / ~/.zshrc 添加: export PATH=\"{shim_dir}:$PATH\"")
            else:
                ok(f"{shim_dir} 已在 PATH 中")
            return 0
        except PermissionError:
            warn(f"无权写入 {shim_dir}，尝试下一个...")
            continue
    err("所有候选目录均无法写入，请手动配置 PATH")
    return 1


# ─── Windows shim ────────────────────────────────────────────────────────────
def _windows_shim_dir() -> Path:
    """返回 Windows 用户级 bin 目录（确保在 PATH 中）"""
    # 优先用 %LOCALAPPDATA%\Programs
    local_app = os.environ.get("LOCALAPPDATA", "")
    if local_app:
        return Path(local_app) / "Programs" / "ae-sdd"
    return Path.home() / "AppData" / "Local" / "Programs" / "ae-sdd"


def _windows_cmd_content(cli_path: Path, python_exe: str) -> str:
    return (
        "@echo off\n"
        f'"{python_exe}" "{cli_path}" %*\n'
    )


def _windows_ps1_content(cli_path: Path, python_exe: str) -> str:
    return (
        f'& "{python_exe}" "{cli_path}" @args\n'
    )


def install_windows(cli_path: Path) -> int:
    python_exe = _python_exe()
    shim_dir = _windows_shim_dir()

    try:
        shim_dir.mkdir(parents=True, exist_ok=True)
    except OSError as e:
        err(f"无法创建目录 {shim_dir}: {e}")
        return 1

    # .cmd shim
    cmd_shim = shim_dir / "ae-sdd.cmd"
    cmd_shim.write_text(_windows_cmd_content(cli_path, python_exe), encoding="utf-8")
    ok(f"cmd shim 已创建: {cmd_shim}")

    # .ps1 shim（PowerShell）
    ps1_shim = shim_dir / "ae-sdd.ps1"
    ps1_shim.write_text(_windows_ps1_content(cli_path, python_exe), encoding="utf-8")
    ok(f"ps1 shim 已创建: {ps1_shim}")

    # 检查是否在 PATH
    path_env = os.environ.get("PATH", "")
    shim_str = str(shim_dir)
    if shim_str.lower() not in path_env.lower():
        warn(f"{shim_dir} 不在 PATH 中，需要手动添加：")
        warn(f"PowerShell: $env:PATH += ';{shim_dir}'")
        warn(f"永久添加: 系统属性 → 环境变量 → Path → 新建 → {shim_dir}")
        info("")
        info("或者运行以下命令（当前会话有效）：")
        info(f'  $env:PATH += ";{shim_dir}"')
        info("")
        info("永久添加到用户 PATH（PowerShell 管理员）：")
        info(f'  [Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";{shim_dir}", "User")')
        return 0  # 安装成功，只是需要用户配置 PATH
    else:
        ok(f"{shim_dir} 已在 PATH 中")
        return 0


# ─── 检查状态 ─────────────────────────────────────────────────────────────────
def check_status() -> int:
    cli = _cli_target()
    info(f"CLI 路径:   {cli}")
    info(f"CLI 存在:   {'✅' if cli.is_file() else '❌'}")

    in_path = shutil.which("ae-sdd")
    if in_path:
        ok(f"ae-sdd 在 PATH 中: {in_path}")
        # 验证指向的是正确的脚本
        try:
            import subprocess
            r = subprocess.run(
                ["ae-sdd", "version"],
                capture_output=True, text=True, timeout=5
            )
            if r.returncode == 0:
                ok(f"ae-sdd version: {r.stdout.strip()[:60]}")
            else:
                warn(f"ae-sdd version 失败: {r.stderr[:60]}")
        except Exception as e:
            warn(f"无法执行 ae-sdd version: {e}")
        return 0
    else:
        err("ae-sdd 不在 PATH 中")
        info("运行 python scripts/install_cli.py 安装 shim")
        return 1


# ─── 卸载 ────────────────────────────────────────────────────────────────────
def uninstall() -> int:
    removed = []

    # Unix
    for shim_dir in _UNIX_SHIM_DIRS:
        shim = shim_dir / "ae-sdd"
        if shim.is_file():
            shim.unlink()
            removed.append(str(shim))

    # Windows
    shim_dir = _windows_shim_dir()
    for name in ("ae-sdd.cmd", "ae-sdd.ps1"):
        f = shim_dir / name
        if f.is_file():
            f.unlink()
            removed.append(str(f))

    if removed:
        for r in removed:
            ok(f"已移除: {r}")
    else:
        info("未找到已安装的 shim，无需卸载")
    return 0


# ─── 主流程 ───────────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="install_cli: 把 ae-sdd CLI 安装到系统 PATH",
    )
    parser.add_argument("--check", action="store_true", help="只检查当前状态")
    parser.add_argument("--uninstall", action="store_true", help="移除 shim")
    args = parser.parse_args()

    if args.check:
        return check_status()
    if args.uninstall:
        return uninstall()

    cli = _cli_target()
    if not cli.is_file():
        err(f"CLI 文件不存在: {cli}")
        err("请确认在 ae-sdd 仓库根目录下运行本脚本")
        return 1

    print()
    info(f"ae-sdd CLI: {cli}")
    info(f"Python:     {_python_exe()}")
    print()

    is_windows = platform.system() == "Windows"
    if is_windows:
        rc = install_windows(cli)
    else:
        rc = install_unix(cli)

    if rc == 0:
        print()
        ok("安装完成。验证命令：")
        info("  ae-sdd version")
        info("  ae-sdd health")
        print()

    return rc


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        err("用户中断")
        sys.exit(130)
