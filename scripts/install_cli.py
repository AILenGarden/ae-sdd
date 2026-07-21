#!/usr/bin/env python3
"""
install_cli.py — 把 ae-sdd CLI 安装到系统 PATH

问题背景：
  hook 配置里写的是 "ae-sdd gate-intercept"，
  但 tools/bin/ae-sdd 不在系统 PATH，导致三个 hook 全部静默失效。

本脚本解决方案（按优先级）：
  1. 在 ~/.local/bin/ae-sdd 创建 shim 脚本（Unix）
  2. 在 ~/AppData/Local/Programs/ae-sdd/ae-sdd.cmd 创建 Windows 批处理 shim（Windows）
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


CLI_VERSION_CHECK_TIMEOUT_SECONDS = 5


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


def _canonical_windows_path(value: str | Path) -> str:
    """返回用于 Windows PATH 去重的大小写不敏感规范值。"""
    expanded = os.path.expandvars(str(value).strip().strip('"'))
    return expanded.replace("/", "\\").rstrip("\\").casefold()


def _split_windows_path(path_value: str) -> list[str]:
    """按 Windows PATH 语义切分，丢弃空项但保留原始条目文本。"""
    return [item.strip() for item in path_value.split(";") if item.strip()]


def _append_windows_path_entry(path_value: str, entry: str | Path) -> tuple[str, bool]:
    """幂等追加 Windows PATH 条目，返回（新值，是否变化）。"""
    target = _canonical_windows_path(entry)
    if any(_canonical_windows_path(item) == target for item in _split_windows_path(path_value)):
        return path_value, False
    return (f"{path_value};{entry}" if path_value else str(entry)), True


def _remove_windows_path_entry(path_value: str, entry: str | Path) -> tuple[str, bool]:
    """仅移除目标 Windows PATH 条目，不影响其他目录。"""
    target = _canonical_windows_path(entry)
    items = path_value.split(";")
    remaining = [
        item for item in items
        if not item.strip() or _canonical_windows_path(item) != target
    ]
    if len(remaining) == len(items):
        return path_value, False
    return ";".join(remaining), True


def _read_windows_user_path() -> tuple[str, int]:
    r"""读取 HKCU\Environment\Path，并保留原注册表值类型。"""
    import winreg

    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment") as key:
            value, value_type = winreg.QueryValueEx(key, "Path")
            return str(value), value_type
    except FileNotFoundError:
        return "", winreg.REG_EXPAND_SZ


def _write_windows_user_path(path_value: str, value_type: int) -> None:
    """写入当前用户 PATH；不要求管理员权限。"""
    import winreg

    with winreg.CreateKeyEx(
        winreg.HKEY_CURRENT_USER,
        "Environment",
        0,
        winreg.KEY_SET_VALUE,
    ) as key:
        winreg.SetValueEx(key, "Path", 0, value_type, path_value)


def _broadcast_environment_change() -> None:
    """通知 Windows 环境变量已更新；当前父进程仍需重启才能继承。"""
    try:
        import ctypes

        result = ctypes.c_ulong()
        ctypes.windll.user32.SendMessageTimeoutW(
            0xFFFF,  # HWND_BROADCAST
            0x001A,  # WM_SETTINGCHANGE
            0,
            "Environment",
            0x0002,  # SMTO_ABORTIFHUNG
            5000,
            ctypes.byref(result),
        )
    except (AttributeError, OSError):
        # PATH 已写入注册表；广播失败只影响尚未重启的 GUI 进程。
        return


def _ensure_windows_user_path(shim_dir: Path) -> bool:
    """确保 shim 目录同时进入用户 PATH 与当前安装进程 PATH。"""
    user_path, value_type = _read_windows_user_path()
    updated_user_path, changed = _append_windows_path_entry(user_path, shim_dir)
    if changed:
        _write_windows_user_path(updated_user_path, value_type)
        _broadcast_environment_change()

    process_path, _ = _append_windows_path_entry(os.environ.get("PATH", ""), shim_dir)
    os.environ["PATH"] = process_path
    return changed


def _remove_windows_user_path(shim_dir: Path) -> bool:
    """从用户 PATH 和当前安装进程 PATH 中仅移除 ae-sdd shim 目录。"""
    user_path, value_type = _read_windows_user_path()
    updated_user_path, changed = _remove_windows_path_entry(user_path, shim_dir)
    if changed:
        _write_windows_user_path(updated_user_path, value_type)
        _broadcast_environment_change()

    process_path, _ = _remove_windows_path_entry(os.environ.get("PATH", ""), shim_dir)
    os.environ["PATH"] = process_path
    return changed


def _windows_cmd_content(cli_path: Path, python_exe: str) -> str:
    return (
        "@echo off\n"
        f'"{python_exe}" "{cli_path}" %*\n'
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

    # 旧版本曾同时创建 .ps1；PowerShell 的命令优先级会让 Start-Process
    # 先解析到 .ps1，再因它不是 Win32 应用而失败。保留单一 .cmd 入口即可同时
    # 覆盖 PowerShell、cmd 和 ShellExecute/Start-Process。
    ps1_shim = shim_dir / "ae-sdd.ps1"
    if ps1_shim.is_file():
        ps1_shim.unlink()
        ok(f"已移除会抢占 Start-Process 解析的旧 ps1 shim: {ps1_shim}")

    try:
        changed = _ensure_windows_user_path(shim_dir)
    except OSError as e:
        err(f"shim 已创建，但无法更新用户 PATH: {e}")
        return 1

    if changed:
        ok(f"已将 {shim_dir} 加入用户 PATH")
        warn("已运行的 Codex/终端不会自动继承新 PATH；请重启应用或终端")
    else:
        ok(f"{shim_dir} 已在用户 PATH 中")
    return 0


# ─── 检查状态 ─────────────────────────────────────────────────────────────────
def check_status() -> int:
    cli = _cli_target()
    info(f"CLI 路径:   {cli}")
    info(f"CLI 存在:   {'✅' if cli.is_file() else '❌'}")
    if not cli.is_file():
        return 1

    lookup_path = os.environ.get("PATH", "")
    windows_user_path_ready = True
    if platform.system() == "Windows":
        shim_dir = _windows_shim_dir()
        try:
            user_path, _ = _read_windows_user_path()
        except OSError as e:
            err(f"无法读取用户 PATH: {e}")
            return 1
        windows_user_path_ready = any(
            _canonical_windows_path(item) == _canonical_windows_path(shim_dir)
            for item in _split_windows_path(user_path)
        )
        if windows_user_path_ready:
            # 已运行的父进程可能尚未继承注册表中的新 PATH；仅在确认持久化后
            # 将目录加入本次探测路径，避免 --check 对临时会话配置误报成功。
            lookup_path, _ = _append_windows_path_entry(lookup_path, shim_dir)

    in_path = shutil.which("ae-sdd", path=lookup_path)
    if in_path:
        if platform.system() == "Windows" and not windows_user_path_ready:
            err(f"{_windows_shim_dir()} 尚未写入用户 PATH")
            info("运行 python scripts/install_cli.py 修复持久化配置")
            return 1
        if platform.system() == "Windows" and (
            Path(in_path).name.casefold() != "ae-sdd.cmd"
            or _canonical_windows_path(Path(in_path).parent)
            != _canonical_windows_path(_windows_shim_dir())
        ):
            err(f"ae-sdd 当前解析到其他入口: {in_path}")
            info(f"预期入口: {_windows_shim_dir() / 'ae-sdd.cmd'}")
            return 1
        ok(f"ae-sdd 在 PATH 中: {in_path}")
        # 验证指向的是正确的脚本
        try:
            import subprocess
            command = [in_path, "version"]
            if platform.system() == "Windows" and Path(in_path).suffix.lower() in {".cmd", ".bat"}:
                command = [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/c", in_path, "version"]
            child_env = os.environ.copy()
            child_env["PATH"] = lookup_path
            r = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=CLI_VERSION_CHECK_TIMEOUT_SECONDS,
                env=child_env,
            )
            if r.returncode == 0:
                ok(f"ae-sdd version: {r.stdout.strip()[:60]}")
                return 0
            else:
                warn(f"ae-sdd version 失败: {r.stderr[:60]}")
                return 1
        except Exception as e:
            warn(f"无法执行 ae-sdd version: {e}")
            return 1
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

    if platform.system() == "Windows":
        try:
            if _remove_windows_user_path(shim_dir):
                ok(f"已从用户 PATH 移除: {shim_dir}")
        except OSError as e:
            warn(f"shim 已移除，但清理用户 PATH 失败: {e}")

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
        info("提示：如果已用 --use-python 模式配置 hook，安装完成后请重新运行：")
        info("  python scripts/install_cli.py --check")
        info("  若 hook 路径仍指向旧路径，重跑：ae-sdd init-hooks --use-python --force")
        print()

    return rc


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        err("用户中断")
        sys.exit(130)
