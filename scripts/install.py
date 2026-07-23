#!/usr/bin/env python3
"""
install.py — ae-sdd SKILL 安装脚本（兼容入口）

🆕 v3.4.4：本脚本已转为 distribute.py 的薄包装（决策4 单入口 orchestrator）。
   实际编译/分发逻辑全部下沉到 scripts/distribute.py + scripts/distributors/。
   本文件保留是为兼容老命令（python scripts/install.py / dev-sync.sh / post-commit 旧调用），
   以及保留 --uninstall 独立逻辑（distribute.py 不处理卸载）。

四种运行模式（自动检测，由 distribute.py 承接）：
    1) 本地仓库（已在 ae-sdd 仓库根目录）
    2) 本地 dist（dist/ae-sdd/ 已构建）
    3) 远程 git clone
    4) 远程 zip 下载

安装目标（由分发器插件决定协议）:
  - ~/.claude/skills/ae-sdd/        (copytree)
  - ~/.codex/skills/ae-sdd/         (copytree，auto 检测)
  - ~/.zcode/skills/ae-sdd/         (copytree，auto 检测)
  - mavis harness mount             (harness_mount，auto 检测)

用法:
    python scripts/install.py                    # 转调 distribute.py（auto 模式）
    python scripts/install.py --from-build       # 同上（distribute 默认就 build）
    python scripts/install.py --target codex     # 只装 Codex
    python scripts/install.py --target-path X    # 旧 post-commit 兼容
    python scripts/install.py --uninstall        # 卸载本地安装（独立逻辑）
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path


def _configure_console_stream(stream) -> None:
    """Keep Windows legacy consoles from turning a successful install into failure.

    Python may select GBK/CP936 for redirected PowerShell output.  The installer
    uses a few status glyphs that are not representable there, so retain the
    console encoding but replace only unsupported glyphs instead of raising
    ``UnicodeEncodeError`` after distribution has already succeeded.
    """
    reconfigure = getattr(stream, "reconfigure", None)
    if callable(reconfigure):
        try:
            reconfigure(errors="replace")
        except (AttributeError, ValueError, OSError):
            pass


_configure_console_stream(sys.stdout)
_configure_console_stream(sys.stderr)

# 让 distribute.py 可被 import
sys.path.insert(0, str(Path(__file__).resolve().parent))


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


# ─── 常量（卸载逻辑用，迁自旧 install.py） ───────────────────────────────────
REPO_URL   = "https://github.com/AILenGarden/ae-sdd"
SKILL_NAME = "ae-sdd"
CLAUDE_DST = Path.home() / ".claude" / "skills" / SKILL_NAME
CODEX_DST  = Path.home() / ".codex" / "skills" / SKILL_NAME
ZCODE_DST  = Path.home() / ".zcode" / "skills" / SKILL_NAME
HERMES_DST = Path.home() / ".hermes" / "skills" / SKILL_NAME


def _target_paths(selection: str) -> list[Path]:
    """解析卸载目标路径（仅 --uninstall 用，迁自旧 install.py:_target_paths）。"""
    if selection == "claude": return [CLAUDE_DST]
    if selection == "codex":  return [CODEX_DST]
    if selection == "zcode":  return [ZCODE_DST]
    if selection == "hermes": return [HERMES_DST]
    if selection == "all":    return [CLAUDE_DST, CODEX_DST, ZCODE_DST, HERMES_DST]
    # auto
    targets = [CLAUDE_DST]
    if CODEX_DST.parent.is_dir() or CODEX_DST.exists() or shutil.which("codex") or shutil.which("codex.exe"):
        targets.append(CODEX_DST)
    if ZCODE_DST.parent.is_dir() or ZCODE_DST.exists() or shutil.which("zcode") or shutil.which("zcode.exe"):
        targets.append(ZCODE_DST)
    if HERMES_DST.parent.is_dir() or HERMES_DST.exists() or shutil.which("hermes") or shutil.which("hermes.exe"):
        targets.append(HERMES_DST)
    return targets


def uninstall(targets: list[Path]) -> None:
    """卸载本地安装（迁自旧 install.py:uninstall）。

    🆕 根治：卸载备份移到 ~/.<agent>/ae-sdd-backups/（agent 域内、skills 同级），
    不再留在 skills 目录（避免 .uninstalled 备份被加载器误识别为独立技能）。
    """
    any_removed = False
    for dst in targets:
        if not dst.exists():
            info(f"未找到 {dst}，无需卸载")
            continue
        ts = datetime.now().strftime("%Y%m%d%H%M%S")
        # 跟着被卸载的 agent 走：~/.<agent>/skills/ae-sdd → ~/.<agent>/ae-sdd-backups/
        backup_root = dst.parent.parent / "ae-sdd-backups"
        backup_root.mkdir(parents=True, exist_ok=True)
        bak = backup_root / f"{dst.name}.uninstalled.{ts}"
        warn(f"卸载本地安装: {dst}")
        warn(f"备份到: {bak}")
        dst.rename(bak)
        success(f"已卸载（备份在 {bak}）")
        any_removed = True
    if not any_removed:
        info("没有可卸载的 ae-sdd 安装")


def _detect_agents() -> dict:
    """检测可用的 Agent CLI（迁自旧 install.py:_detect_agents）。"""
    agents = {}
    if shutil.which("claude") or shutil.which("claude.exe"):
        agents["claude"] = "Claude Code"
    if shutil.which("codex") or shutil.which("codex.exe"):
        agents["codex"] = "Codex CLI"
    if shutil.which("hermes") or shutil.which("hermes.exe"):
        agents["hermes"] = "Hermes CLI"
    if shutil.which("mavis") or shutil.which("mavis.exe"):
        agents["mavis"] = "Mavis daemon"
    return agents


def print_usage() -> None:
    print()
    success("ae-sdd SKILL 安装成功！")
    print()
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
    else:
        print(f"  ⚠️ 未检测到 Claude Code / Codex / Mavis 等 Agent CLI")
        print(f"  → 推荐安装 Claude Code：https://docs.claude.com/en/docs/claude-code/installation")
    print()
    print(f"  更多信息：{REPO_URL}")
    print()


# ─── 主流程：转调 distribute.py（--uninstall 除外） ──────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="install: 把 ae-sdd 安装到本地 Agent skills（转调 distribute.py）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--from-build", action="store_true",
                        help="(兼容) distribute 默认就 build，此参数无额外效果")
    parser.add_argument("--uninstall", action="store_true",
                        help="卸载本地安装（独立逻辑，不转调 distribute）")
    parser.add_argument("--target", default="auto",
                        help="安装目标：auto=所有 detect=True 的分发器；all=强制全跑；"
                             "claude/codex/zcode/hermes/mavis=单跑指定（透传给 distribute.py）")
    parser.add_argument("--target-path", type=str, default=None,
                        help="(兼容 post-commit) 显式安装目标绝对路径，优先级高于 --target")
    parser.add_argument("--quiet", action="store_true",
                        help="(post-commit 用) 静默模式，只输出关键状态")
    parser.add_argument("--keep-bak", type=int, default=2,
                        help=f"每个目标保留的 .bak 备份数（默认 2；0=全清；负数=不清理）")
    parser.add_argument("--cleanup-mavis", dest="cleanup_mavis", action="store_true",
                        default=True, help="(兼容) 清理 mavis 端 ae-sdd-N 副本（默认开启）")
    parser.add_argument("--no-cleanup-mavis", dest="cleanup_mavis", action="store_false",
                        help="跳过 mavis 端 ae-sdd-N 副本清理")
    parser.add_argument("--mavis-keep", type=int, default=0,
                        help="(兼容) mavis 端 -N 副本保留数（默认 0=全清）")
    args = parser.parse_args()

    # ── --uninstall：独立逻辑（distribute.py 不处理卸载） ───────────────────
    # 注：uninstall 仅支持 copytree 类目标（claude/codex/zcode/hermes/all/auto）。
    # mavis 卸载请用 `mavis harness unmount ae-sdd`（非文件操作）。
    if args.uninstall:
        if args.target_path:
            targets = [Path(args.target_path).expanduser().resolve()]
        elif args.target in ("claude", "codex", "zcode", "hermes", "all", "auto"):
            targets = _target_paths(args.target)
        else:
            error(f"--uninstall 不支持 target='{args.target}'（仅 claude/codex/zcode/hermes/all/auto）")
            error(f"卸载 mavis 请用：mavis harness unmount ae-sdd")
            return 1
        uninstall(targets)
        return 0

    # ── 转调 distribute.py ──────────────────────────────────────────────────
    distribute_cmd = [sys.executable, str(Path(__file__).resolve().parent / "distribute.py")]
    distribute_cmd += ["--target", args.target]
    if args.target_path:
        distribute_cmd += ["--target-path", args.target_path]
    if args.quiet:
        distribute_cmd.append("--quiet")
    if args.keep_bak is not None:
        distribute_cmd += ["--keep-bak", str(args.keep_bak)]
    if not args.cleanup_mavis:
        distribute_cmd.append("--no-cleanup-mavis")

    if not args.quiet:
        info("转调 distribute.py ...")
    result = subprocess.run(distribute_cmd)
    if result.returncode == 0 and not args.quiet and not args.uninstall:
        print_usage()
    return result.returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        error("用户中断")
        sys.exit(130)
