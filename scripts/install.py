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
import re
import shutil
import sqlite3
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
ZCODE_DST    = Path.home() / ".zcode" / "skills" / SKILL_NAME   # 🆕 v3.4.0：zcode CLI skills 目录
DST          = CLAUDE_DST  # 向后兼容：历史代码/文档默认指 Claude 目标
BAK_KEEP_DEFAULT     = 2  # 每次 install 后保留最近 N 个 .bak 备份（防止 .bak 累积噪音）
MAVIS_KEEP_DEFAULT   = 0  # 清理 mavis 端 ae-sdd-N 副本时保留的数量（0=全清；负数=不清理）
MAVIS_SQLITE_DB      = Path.home() / ".mavis" / "sqlite.db"


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
    """解析安装目标。auto 保持 Claude 兼容，同时同步已有/可用 Codex + zcode。
    🆕 v3.4.0：post-commit 钩子依赖 --target <PATH> 形式，所以支持自定义路径（"path" 模式）。
    """
    if selection == "claude":
        return [CLAUDE_DST]
    if selection == "codex":
        return [CODEX_DST]
    if selection == "zcode":
        return [ZCODE_DST]
    if selection == "all":
        return [CLAUDE_DST, CODEX_DST, ZCODE_DST]

    # auto：永远保持原 Claude 目标；Codex 已安装或 codex CLI 存在时追加；
    # zcode 已安装或 zcode CLI 存在时追加（v3.4.0+：post-commit hook 默认装 zcode）。
    targets = [CLAUDE_DST]
    if CODEX_DST.exists() or shutil.which("codex") or shutil.which("codex.exe"):
        targets.append(CODEX_DST)
    if ZCODE_DST.exists() or shutil.which("zcode") or shutil.which("zcode.exe"):
        targets.append(ZCODE_DST)
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


def cleanup_mavis_duplicates(skill_name: str = SKILL_NAME,
                             mavis_home: Optional[Path] = None,
                             keep: int = MAVIS_KEEP_DEFAULT) -> int:
    """清理 mavis 端 {skill_name}-N 副本（`mavis skill install` 自动加后缀累积的）。

    只删形如 `{skill_name}-\\d+` 的目录（数字后缀），不动：
    - `{skill_name}` 本体（用户可能特意装在 mavis 端）
    - `{skill_name}-harness-adapter` 等带语义后缀的合法 SKILL
    - 其它非 ae-sdd 前缀的 skill

    同步清理 mavis 的 sqlite.db `skills` 表对应记录（带 .bak 备份）。
    keep=0 表示全清 -N 副本；keep<0 表示不清理。
    返回删除的物理目录数。
    """
    if keep < 0:
        return 0
    if mavis_home is None:
        mavis_home = Path.home() / ".mavis"
    skills_dir = mavis_home / "skills"
    if not skills_dir.is_dir():
        return 0

    # 只匹配 数字后缀副本（ae-sdd-2 / ae-sdd-3 ...），不碰 ae-sdd-harness-adapter
    pattern = re.compile(rf"^{re.escape(skill_name)}-\d+$")
    dupes = sorted(
        [p for p in skills_dir.iterdir() if p.is_dir() and pattern.match(p.name)],
        key=lambda p: p.name,  # 数字后缀字典序 = 数字序
    )
    if not dupes:
        return 0

    # keep>0 时保留最近 keep 个（按数字后缀倒序 = 留最大的 N 个）
    if keep > 0 and len(dupes) > keep:
        dupes = dupes[:-keep]

    # 1. 同步 sqlite 记录（带备份）
    db_path = mavis_home / "sqlite.db"
    db_deleted = 0
    if db_path.is_file():
        try:
            db_backup = db_path.with_suffix(
                f".db.bak.{datetime.now().strftime('%Y%m%d%H%M%S')}"
            )
            shutil.copy2(db_path, db_backup)
            conn = sqlite3.connect(str(db_path))
            cur = conn.cursor()
            for d in dupes:
                cur.execute("DELETE FROM skills WHERE name = ?", (d.name,))
                db_deleted += cur.rowcount
            conn.commit()
            conn.close()
            warn(f"已备份 mavis sqlite.db → {db_backup.name}")
        except Exception as e:
            warn(f"同步清理 mavis sqlite 记录失败（物理目录仍会清理）: {e}")
    else:
        warn("未找到 mavis sqlite.db，跳过索引同步（仅清物理目录）")

    # 2. 删物理目录
    removed = 0
    for d in dupes:
        try:
            shutil.rmtree(d)
            warn(f"清理 mavis 端 -N 副本: {d.name}")
            removed += 1
        except OSError as e:
            warn(f"删除 {d.name} 失败: {e}")

    if removed:
        info(f"已清理 mavis 端 {removed} 个 {skill_name}-N 副本（sqlite 同步删 {db_deleted} 条）")
        if db_deleted < removed:
            warn("注意：mavis daemon 内存中的 skill 缓存可能未同步，")
            warn("      如有残留请通过 MiniMax 桌面应用重启 daemon 后再 list 一次。")
    return removed


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
    parser.add_argument("--target", choices=["auto", "claude", "codex", "zcode", "all"],
                        default="auto",
                        help="安装目标：auto=Claude + 已存在/可用 Codex/Zcode；all=三者都装；指定名字=单目标")
    parser.add_argument("--target-path", type=str, default=None,
                        help="(v3.4.0+ post-commit hook 用) 显式指定安装目标绝对路径，优先级高于 --target")
    parser.add_argument("--quiet", action="store_true",
                        help="(v3.4.0+ post-commit hook 用) 静默模式，只输出关键状态")
    parser.add_argument("--keep-bak", type=int, default=BAK_KEEP_DEFAULT,
                        help=f"每个目标保留的 .bak 备份数（默认 {BAK_KEEP_DEFAULT}；0=全清；负数=不清理）")
    parser.add_argument("--cleanup-mavis", dest="cleanup_mavis", action="store_true",
                        default=True,
                        help="清理 mavis 端 ae-sdd-N 副本（默认开启）")
    parser.add_argument("--no-cleanup-mavis", dest="cleanup_mavis", action="store_false",
                        help="跳过 mavis 端 ae-sdd-N 副本清理")
    parser.add_argument("--mavis-keep", type=int, default=MAVIS_KEEP_DEFAULT,
                        help=f"mavis 端 ae-sdd-N 副本保留数（默认 {MAVIS_KEEP_DEFAULT}=全清；负数=不清理）")
    args = parser.parse_args()
    # --target-path 优先级最高（post-commit hook 调用形式）
    if args.target_path:
        targets = [Path(args.target_path).expanduser().resolve()]
    else:
        targets = _target_paths(args.target)

    if args.uninstall:
        uninstall(targets)
        return 0

    if not args.quiet:
        print()
        info("开始安装 ae-sdd SKILL...")
        print()

    if args.from_build:
        # 强制本地 build + install
        repo_root = resolve_local_repo()
        if not repo_root:
            error("--from-build 模式必须在 ae-sdd 仓库根目录运行")
            return 1
        if not args.quiet:
            info("本地 build + install 模式 (--from-build)")
        run_build(repo_root)
        dist_src = repo_root / "dist" / "ae-sdd"
    else:
        # 自动检测
        repo_root = resolve_local_repo()
        if repo_root:
            if not args.quiet:
                info("本地仓库模式")
            dist_src = repo_root / "dist" / "ae-sdd"
            if not dist_src.is_dir() or not (dist_src / "SKILL.md").is_file():
                warn("本地 dist/ae-sdd/ 不存在或主入口缺失，先跑 build...")
                run_build(repo_root)
        else:
            if not args.quiet:
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
    if args.cleanup_mavis:
        cleanup_mavis_duplicates(keep=args.mavis_keep)
    if not args.quiet:
        print_usage(targets)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        error("用户中断")
        sys.exit(130)
