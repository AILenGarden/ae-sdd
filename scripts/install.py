#!/usr/bin/env python3
"""
install.py — ae-sdd SKILL 安装脚本（跨平台）

🆕 v3.0.1 跨平台化（2026-06-18）：用 Python 替代 bash，零外部依赖（仅标准库）。
🆕 v3.0 双目录分层：从 dist/ae-sdd/ 实例化分发包安装（不再是 plugins/ae-sdd/）。

四种运行模式（自动检测）：
    1) 本地仓库（已在 ae-sdd 仓库根目录）
    2) 本地 dist（dist/ae-sdd/ 已构建）
    3) 远程 git clone
    4) 远程 zip 下载

安装目标:
  - ~/.claude/skills/ae-sdd/
  - ~/.codex/skills/ae-sdd/（当目录已存在或检测到 codex CLI 时自动同步）

用法:
    python scripts/install.py                    # 自动检测模式 + 自动安装到可用 Agent
    python scripts/install.py --from-build       # 强制本地 build + install
    python scripts/install.py --target codex     # 只安装到 Codex skills
    python scripts/install.py --uninstall        # 卸载本地安装
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import urllib.request
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Optional


# ─── 颜色（ANSI） ────────────────────────────────────────────────────────────
def _supports_color() -> bool:
    return sys.stdout.isatty()


if _supports_color():
    C_GREEN  = "\033[0;32m"
    C_YELLOW = "\033[1;33m"
    C_RED    = "\033[0;31m"
    C_RESET  = "\033[0m"
else:
    C_GREEN = C_YELLOW = C_RED = C_RESET = ""


def info(msg: str)    -> None: print(f"{C_GREEN}[ae-sdd]{C_RESET} {msg}")
def warn(msg: str)    -> None: print(f"{C_YELLOW}[ae-sdd] ⚠{C_RESET}  {msg}")
def error(msg: str)   -> None: print(f"{C_RED}[ae-sdd] ✗{C_RESET}  {msg}", file=sys.stderr)
def success(msg: str) -> None: print(f"{C_GREEN}[ae-sdd] ✅{C_RESET} {msg}")


# ─── 常量 ────────────────────────────────────────────────────────────────────
REPO_URL     = "https://github.com/AILenGarden/ae-sdd"
RELEASE_URL  = "https://github.com/AILenGarden/ae-sdd/archive/refs/heads/main.zip"
SKILL_NAME   = "ae-sdd"
CLAUDE_DST   = Path.home() / ".claude" / "skills" / SKILL_NAME
CODEX_DST    = Path.home() / ".codex" / "skills" / SKILL_NAME
DST          = CLAUDE_DST  # 向后兼容：历史代码/文档默认指 Claude 目标
BAK_KEEP_DEFAULT = 2  # 每次 install 后保留最近 N 个 .bak 备份（防止 .bak 累积噪音）


# ─── 子流程：调 build_dist.py ────────────────────────────────────────────────
def run_build(repo_root: Path) -> None:
    """调 build_dist.py 构建 dist/ae-sdd/"""
    script = repo_root / "scripts" / "build_dist.py"
    if not script.is_file():
        error(f"未找到 {script}")
        sys.exit(1)
    info("运行 build_dist.py 构建实例化分发包...")
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=str(repo_root),
    )
    if result.returncode != 0:
        error(f"build_dist.py 执行失败 (exit {result.returncode})")
        sys.exit(1)


# ─── 模式 1：本地仓库 ────────────────────────────────────────────────────────
def resolve_local_repo() -> Optional[Path]:
    """如果当前目录是 ae-sdd 仓库根（含 source/），返回仓库根；否则 None"""
    # 当前工作目录
    cwd = Path.cwd()
    if (cwd / "source").is_dir():
        return cwd
    # 脚本所在目录的父目录
    script_dir = Path(__file__).resolve().parent
    repo_root = script_dir.parent
    if (repo_root / "source").is_dir():
        return repo_root
    return None


# ─── 模式 2：远程 git clone ───────────────────────────────────────────────────
def _has_git() -> bool:
    return shutil.which("git") is not None


def _has_unzip() -> bool:
    return shutil.which("unzip") is not None


def fetch_remote() -> Path:
    """下载 ae-sdd 仓库到临时目录，返回仓库根路径"""
    tmp = Path.home() / ".cache" / "ae-sdd-install" / datetime.now().strftime("%Y%m%d%H%M%S")
    tmp.mkdir(parents=True, exist_ok=True)

    if _has_git():
        info(f"远程模式：正在 clone 仓库...")
        result = subprocess.run(
            ["git", "clone", "--depth=1", REPO_URL, str(tmp / "ae-sdd")],
            cwd=str(tmp),
        )
        if result.returncode != 0:
            error(f"git clone 失败 (exit {result.returncode})")
            sys.exit(1)
        return tmp / "ae-sdd"

    # 没 git — 走 zip
    info(f"远程模式：未找到 git，下载 zip...")
    zip_path = tmp / "ae-sdd.zip"
    try:
        urllib.request.urlretrieve(RELEASE_URL, str(zip_path))
    except Exception as e:
        error(f"下载失败: {e}")
        sys.exit(1)

    info("解压中...")
    with zipfile.ZipFile(zip_path, "r") as zf:
        zf.extractall(tmp)
    zip_path.unlink()

    # GitHub zip 解压后子目录形如 ae-sdd-main
    extracted = next((d for d in tmp.iterdir() if d.is_dir() and d.name.startswith("ae-sdd-")), None)
    if not extracted:
        error("解压目录结构异常，未找到 ae-sdd-* 子目录")
        sys.exit(1)
    return extracted


# ─── 备份 + 安装 + 验证 ──────────────────────────────────────────────────────
def _target_paths(selection: str) -> list[Path]:
    """解析安装目标。auto 保持 Claude 兼容，同时同步已有/可用 Codex。"""
    if selection == "claude":
        return [CLAUDE_DST]
    if selection == "codex":
        return [CODEX_DST]
    if selection == "all":
        return [CLAUDE_DST, CODEX_DST]

    # auto：永远保持原 Claude 目标；Codex 已安装或 codex CLI 存在时追加。
    targets = [CLAUDE_DST]
    if CODEX_DST.exists() or shutil.which("codex") or shutil.which("codex.exe"):
        targets.append(CODEX_DST)
    return targets


def backup_existing(dst: Path) -> None:
    """备份已有安装到 .bak.<时间戳>"""
    if dst.exists():
        ts = datetime.now().strftime("%Y%m%d%H%M%S")
        bak = dst.with_name(f"{dst.name}.bak.{ts}")
        warn(f"检测到已有安装版本，备份到：")
        warn(f"  {bak}")
        dst.rename(bak)


def cleanup_old_backups(skills_dir: Path, skill_name: str, keep: int = BAK_KEEP_DEFAULT) -> None:
    """清理 skills_dir 下 {skill_name}.bak.* 旧备份，保留最近 keep 个。

    排序：按目录名（`.bak.YYYYMMDDHHMMSS`）字典序降序，新→旧。
    只清理 .bak.*，不动 .uninstalled.*（用户主动卸载的产物另算）。
    keep=0 表示全部清理；keep<0 表示不清理。
    """
    if keep < 0:
        return
    pattern = f"{skill_name}.bak.*"
    baks = sorted(
        (p for p in skills_dir.glob(pattern) if p.is_dir()),
        key=lambda p: p.name,
        reverse=True,
    )
    if len(baks) <= keep:
        return
    to_remove = baks[keep:]
    removed = 0
    for old in to_remove:
        try:
            shutil.rmtree(old)
            warn(f"清理旧备份: {old.name}")
            removed += 1
        except OSError as e:
            warn(f"清理失败 {old.name}: {e}")
    if removed:
        info(f"已清理 {removed} 个旧备份（保留最近 {keep} 个）")


def install_from_dist(dist_src: Path, dst: Path) -> None:
    """从 dist/ae-sdd/ 复制到指定 Agent skills 目录。"""
    if not dist_src.is_dir():
        error(f"未找到 {dist_src}，仓库结构异常")
        sys.exit(1)
    skill_md = dist_src / "SKILL.md"
    if not skill_md.is_file():
        error(f"未找到 {skill_md}，请先跑 python scripts/build_dist.py")
        sys.exit(1)

    if dst.exists():
        shutil.rmtree(dst)
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(dist_src, dst)
    info(f"文件已复制到 {dst}")


def verify(dst: Path) -> None:
    """验证安装：SKILL.md 存在 + VERSION 可读"""
    skill_md = dst / "SKILL.md"
    if not skill_md.is_file():
        error(f"安装验证失败：{skill_md} 不存在")
        sys.exit(1)
    version_file = dst / "VERSION"
    if version_file.is_file():
        ver = version_file.read_text(encoding="utf-8").split("\n")[0]
        info(f"安装版本: {ver} ({dst})")


# ─── 卸载 ────────────────────────────────────────────────────────────────────
def uninstall(targets: list[Path]) -> None:
    any_removed = False
    for dst in targets:
        if not dst.exists():
            info(f"未找到 {dst}，无需卸载")
            continue
        ts = datetime.now().strftime("%Y%m%d%H%M%S")
        bak = dst.with_name(f"{dst.name}.uninstalled.{ts}")
        warn(f"卸载本地安装: {dst}")
        warn(f"备份到: {bak}")
        dst.rename(bak)
        success(f"已卸载（备份在 {bak}）")
        any_removed = True
    if not any_removed:
        info("没有可卸载的 ae-sdd 安装")
        return


# ─── 打印使用提示 ────────────────────────────────────────────────────────────
def _detect_agents() -> dict:
    """检测可用的 Agent CLI（Claude Code / Codex / Mavis 等）。"""
    agents = {}
    # Claude Code: `claude --version` 或 `which claude`
    if shutil.which("claude") or shutil.which("claude.exe"):
        agents["claude"] = "Claude Code"
    # Codex CLI
    if shutil.which("codex") or shutil.which("codex.exe"):
        agents["codex"] = "Codex CLI"
    # Mavis daemon
    if shutil.which("mavis") or shutil.which("mavis.exe"):
        agents["mavis"] = "Mavis daemon"
    return agents


def print_usage(targets: list[Path]) -> None:
    print()
    success("ae-sdd SKILL 安装成功！")
    print()
    print("  安装路径：")
    for dst in targets:
        print(f"    - {dst}")
        version_file = dst / "VERSION"
        if version_file.is_file():
            ver = version_file.read_text(encoding="utf-8").split("\n")[0]
            print(f"      版本: {ver}")
    print()

    # 🆕 v3.1.2：智能引导 — 检测 Agent CLI 存在则给启动命令
    agents = _detect_agents()
    print("  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    if agents:
        print(f"  ✅ 检测到以下 Agent CLI 可用：")
        for cmd, name in agents.items():
            print(f"     • {name}（命令: {cmd}）")
        print()
        print(f"  → 下一步（任选其一）：")
        print(f"    1. 启动 Claude Code：claude")
        print(f"    2. 启动后输入 /ae-sdd 启动自动化工程助手")
        print(f"    3. 或输入\"装 ae-sdd\"让 ae-sdd-install-skill 引导后续配置")
    else:
        print(f"  ⚠️ 未检测到 Claude Code / Codex / Mavis 等 Agent CLI")
        print(f"  → 推荐安装 Claude Code：")
        print(f"     https://docs.claude.com/en/docs/claude-code/installation")
        print()
        print(f"  → 装好 Agent 后，输入 /ae-sdd 即可启动自动化工程助手")
    print()
    print("  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print()
    print(f"  更多信息：{REPO_URL}")
    print()


# ─── 主流程 ──────────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="install: 把 ae-sdd 实例化分发包装到本地 Claude skills",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--from-build", action="store_true",
                        help="强制本地 build + install（先跑 build_dist.py）")
    parser.add_argument("--uninstall", action="store_true",
                        help="卸载本地安装")
    parser.add_argument("--target", choices=["auto", "claude", "codex", "all"],
                        default="auto",
                        help="安装目标：auto=Claude + 已存在/可用 Codex；all=两者都装")
    parser.add_argument("--keep-bak", type=int, default=BAK_KEEP_DEFAULT,
                        help=f"每个目标保留的 .bak 备份数（默认 {BAK_KEEP_DEFAULT}；0=全清；负数=不清理）")
    args = parser.parse_args()
    targets = _target_paths(args.target)

    if args.uninstall:
        uninstall(targets)
        return 0

    print()
    info("开始安装 ae-sdd SKILL...")
    print()

    if args.from_build:
        # 强制本地 build + install
        repo_root = resolve_local_repo()
        if not repo_root:
            error("--from-build 模式必须在 ae-sdd 仓库根目录运行")
            return 1
        info("本地 build + install 模式 (--from-build)")
        run_build(repo_root)
        dist_src = repo_root / "dist" / "ae-sdd"
    else:
        # 自动检测
        repo_root = resolve_local_repo()
        if repo_root:
            info("本地仓库模式")
            dist_src = repo_root / "dist" / "ae-sdd"
            if not dist_src.is_dir() or not (dist_src / "SKILL.md").is_file():
                warn("本地 dist/ae-sdd/ 不存在或主入口缺失，先跑 build...")
                run_build(repo_root)
        else:
            info("远程模式")
            repo_root = fetch_remote()
            run_build(repo_root)
            dist_src = repo_root / "dist" / "ae-sdd"

    for dst in targets:
        backup_existing(dst)
        cleanup_old_backups(dst.parent, SKILL_NAME, args.keep_bak)
    for dst in targets:
        install_from_dist(dist_src, dst)
        verify(dst)
    print_usage(targets)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        error("用户中断")
        sys.exit(130)
