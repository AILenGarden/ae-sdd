#!/usr/bin/env python3
"""
distribute.py — ae-sdd 自动编译分发闭环单入口 orchestrator（决策4）。

串联流程：
  1. build_dist（通用包，所有 copytree 类分发器共用）
  2. 遍历 active 分发器：
       needs_compile=True → distributor.compile() 产出专属产物
       needs_compile=False → 用通用 dist 包
     → distributor.install() → verify() → cleanup()
  3. 汇总报告（每个 Agent: ✅/⚠️/❌/⏭️ + 耗时）

兼容性：
  - post-commit hook 改为只调本脚本（--quiet --from-commit）
  - install.py 内部转调本脚本（保留老命令不破坏）
  - --target-path 兼容旧 post-commit 调用形式

用法:
    python scripts/distribute.py                         # auto：跑所有 detect=True 的分发器
    python scripts/distribute.py --target all            # 强制全跑（含 detect=False）
    python scripts/distribute.py --target mavis          # 只跑 mavis
    python scripts/distribute.py --target-path ~/.claude/skills/ae-sdd  # 旧形式兼容
    python scripts/distribute.py --quiet --from-commit   # post-commit hook 调用
    python scripts/distribute.py --no-build              # 跳过 build_dist（dist 已存在）
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

# 让 `from distributors ...` / `from build_harness ...` 可用
sys.path.insert(0, str(Path(__file__).resolve().parent))

from distributors import DISTRIBUTORS, DistributeContext, InstallResult
from distributors._base import log_info, log_warn, log_success, log_error
from distributors._registry import get_active_distributors


# ─── 颜色 ────────────────────────────────────────────────────────────────────
def _supports_color() -> bool:
    return sys.stdout.isatty()


if _supports_color():
    C_GREEN = "\033[0;32m"
    C_YELLOW = "\033[1;33m"
    C_RED = "\033[0;31m"
    C_BLUE = "\033[0;34m"
    C_RESET = "\033[0m"
else:
    C_GREEN = C_YELLOW = C_RED = C_BLUE = C_RESET = ""


def _resolve_repo_root() -> Path:
    """仓库根 = 脚本父父目录（含 source/）。"""
    return Path(__file__).resolve().parent.parent


def _run_build_dist(repo_root: Path, quiet: bool) -> Path:
    """调 build_dist.py 构建通用包，返回 dist 路径。"""
    build_script = repo_root / "scripts" / "build_dist.py"
    if not build_script.is_file():
        log_error(f"build_dist.py 不存在: {build_script}")
        sys.exit(1)
    if not quiet:
        log_info(None, "运行 build_dist.py 构建实例化分发包...")
    result = subprocess_run([sys.executable, str(build_script)])
    if result.returncode != 0:
        log_error(f"build_dist.py 失败 (exit {result.returncode})")
        sys.exit(1)
    dist_path = repo_root / "dist" / "ae-sdd"
    if not (dist_path / "SKILL.md").is_file():
        log_error(f"build_dist 产出异常：{dist_path}/SKILL.md 不存在")
        sys.exit(1)
    return dist_path


def _verify_compiled_dist(repo_root: Path, dist_path: Path) -> bool:
    """Fail before distribution if dist is not a compiled runtime package."""
    tools_dir = repo_root / "tools"
    inserted = False
    if str(tools_dir) not in sys.path:
        sys.path.insert(0, str(tools_dir))
        inserted = True
    try:
        from lib.runtime_verify import verify_runtime_package  # type: ignore
        result = verify_runtime_package(dist_path)
    except Exception as exc:
        log_error(f"compiled runtime 校验器不可用: {exc}")
        return False
    finally:
        if inserted and str(tools_dir) in sys.path:
            sys.path.remove(str(tools_dir))
    if result.ok:
        return True
    log_error("dist 不是完整 compiled runtime package，拒绝分发:")
    for item in result.issues[:8]:
        log_error(f"  - {item}")
    return False


def subprocess_run(cmd: list[str]):
    """subprocess.run 包装（隔离 import，便于测试 mock）。"""
    import subprocess
    return subprocess.run(cmd)


def _is_path_form(s: str) -> bool:
    """判断字符串是否为路径形式（/xxx POSIX 绝对 或 X:\\ Windows 绝对）。"""
    if not s:
        return False
    if s.startswith("/"):
        return True
    if len(s) >= 2 and s[1] == ":":
        return True
    return False


def _print_summary(results: list[tuple[str, InstallResult, bool]]) -> int:
    """打印汇总报告，返回退出码（有 fail→1，否则 0）。"""
    print()
    print(f"{C_BLUE}━━━ 分发汇总 ━━━{C_RESET}")
    has_fail = False
    for name, res, verified in results:
        if res.status == "ok" and verified:
            mark = f"{C_GREEN}✅{C_RESET}"
        elif res.status == "skip":
            mark = f"{C_YELLOW}⏭️{C_RESET}"
        elif res.status == "warn" or (res.status == "ok" and not verified):
            mark = f"{C_YELLOW}⚠️{C_RESET}"
        else:
            mark = f"{C_RED}❌{C_RESET}"
            has_fail = True
        print(f"  {mark} {name:8s} [{res.status:4s}] {res.message} ({res.duration_sec:.1f}s)")
    print()
    if has_fail:
        print(f"{C_RED}存在失败项，请检查上方日志{C_RESET}")
    else:
        print(f"{C_GREEN}全部分发完成{C_RESET}")
    return 1 if has_fail else 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="distribute: ae-sdd 自动编译分发闭环单入口",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--target", default="auto",
                        help="auto=只跑 detect=True 的；all=强制全跑；<name>=单跑指定；"
                             "<path>=旧 --target-path 兼容（以 / 或盘符开头）")
    parser.add_argument("--target-path", default=None,
                        help="(兼容旧 post-commit) 显式安装目标绝对路径，优先级高于 --target")
    parser.add_argument("--quiet", action="store_true", help="静默模式，只输出关键状态")
    parser.add_argument("--from-commit", action="store_true",
                        help="由 post-commit hook 触发（影响日志详尽度）")
    parser.add_argument("--no-build", action="store_true",
                        help="跳过 build_dist（假设 dist 已存在）")
    parser.add_argument("--no-cleanup-mavis", action="store_true",
                        help="跳过 mavis 端 -N 副本清理")
    parser.add_argument("--keep-bak", type=int, default=2,
                        help="每个目标保留的 .bak 备份数（默认 2；0=全清；负数=不清理）")
    args = parser.parse_args()

    repo_root = _resolve_repo_root()

    # ── target 是 path 形式（/xxx 或 C:\xxx）→ 转成 target_path 统一处理 ──────
    # 旧 post-commit 用 `--target-path X`，新用法也允许 `--target /path/to/skills`
    if args.target and _is_path_form(args.target):
        args.target_path = args.target
        args.target = "auto"

    # ── 旧 --target-path 兼容：直接当 copytree 单目标路径 ───────────────────
    if args.target_path:
        target_path = Path(args.target_path).expanduser().resolve()
        if not args.quiet:
            log_info(None, f"target-path 模式：安装到 {target_path}")
        # 🆕 2026-07-03 注册表模式：直接构造 CopytreeDistributor，不依赖 ClaudeDistributor 子类
        from distributors._base import CopytreeDistributor
        d = CopytreeDistributor(name="target-path", target_path=target_path,
                                detect_fn=lambda: True)
        if args.no_build:
            dist_path = repo_root / "dist" / "ae-sdd"
            if not (dist_path / "SKILL.md").is_file():
                log_error(f"--no-build 但 dist 不存在: {dist_path}")
                return 1
        else:
            dist_path = _run_build_dist(repo_root, args.quiet)
        if not _verify_compiled_dist(repo_root, dist_path):
            return 1
        ctx = DistributeContext(repo_root=repo_root, dist_path=dist_path,
                                keep_bak=args.keep_bak, quiet=args.quiet,
                                from_commit=args.from_commit)
        res = d.install(dist_path, ctx)
        verified = d.verify(ctx) if res.status == "ok" else False
        return _print_summary([(d.name, res, verified)])

    # ── 1. build_dist（除非 --no-build） ────────────────────────────────────
    if args.no_build:
        dist_path = repo_root / "dist" / "ae-sdd"
        if not (dist_path / "SKILL.md").is_file():
            log_error(f"--no-build 但 dist 不存在: {dist_path}")
            return 1
    else:
        dist_path = _run_build_dist(repo_root, args.quiet)
    if not _verify_compiled_dist(repo_root, dist_path):
        return 1

    # ── 2. 收集 active 分发器 ───────────────────────────────────────────────
    ctx = DistributeContext(
        repo_root=repo_root,
        dist_path=dist_path,
        keep_bak=args.keep_bak,
        quiet=args.quiet,
        from_commit=args.from_commit,
    )
    distributors = get_active_distributors(ctx, target_filter=args.target)
    if not distributors:
        log_warn(ctx, f"没有匹配的分发器（target={args.target}）")
        log_info(ctx, f"已注册: {[d.name for d in [c() for c in DISTRIBUTORS]]}")
        return 0

    if not args.quiet:
        log_info(ctx, f"将执行分发器: {[d.name for d in distributors]}")

    # ── 3. 逐个分发器：compile → install → verify → cleanup ─────────────────
    results: list[tuple[str, InstallResult, bool]] = []
    for d in distributors:
        if not args.quiet:
            print(f"\n{C_BLUE}── 分发器: {d.name} ({d.protocol}) ──{C_RESET}")
        try:
            # compile（needs_compile=True 时）
            source = dist_path
            if d.needs_compile:
                if not args.quiet:
                    log_info(ctx, f"{d.name}: 编译专属产物...")
                compiled = d.compile(repo_root)
                if compiled is None:
                    results.append((d.name, InstallResult(d.name, "fail", "compile 失败"), False))
                    continue
                source = compiled

            # install
            res = d.install(source, ctx)
            # verify（仅 install 成功时）
            verified = d.verify(ctx) if res.status == "ok" else False
            # cleanup
            if d.name == "mavis" and args.no_cleanup_mavis:
                pass  # 跳过 mavis 清理
            else:
                try:
                    d.cleanup(ctx)
                except Exception as e:
                    log_warn(ctx, f"{d.name}: cleanup 异常（不影响安装结果）: {e}")
            results.append((d.name, res, verified))
        except Exception as e:
            log_error(f"{d.name}: 未预期异常: {e}")
            results.append((d.name, InstallResult(d.name, "fail", str(e)), False))

    # 🆕 v3.5.10 Gap-010：分发链末尾生成 L2 全局注册表空骨架
    _ensure_l2_global_registry_skeleton(args.quiet)

    # 🆕 v3.10.8：L2 会话级纪律 SSOT 注入（仅已 bootstrap 的 agent 做区间替换）
    try:
        from l2_inject import inject_all
        inject_all(quiet=args.quiet)
    except Exception as e:
        log_warn(ctx, f"L2 注入异常（不影响技能安装结果）: {e}")

    return _print_summary(results)


def _ensure_l2_global_registry_skeleton(quiet: bool) -> None:
    """🆕 v3.5.10 Gap-010：安装完成后在 ~/.ae-sdd/plugins/registry.yaml 生成 L2 全局空骨架。

    背景：plugin_loader.py 的三层注册表设计中，L2 全局层从未被任何 install/install.py
    生成过——导致 `ae-sdd plugin list` 在干净机器上恒报 "⊘ L2-global: 注册表不存在（跳过）"，
    让人误以为 plugin 机制不可用。本函数在分发链末尾补一个空骨架（plugins: []），
    让用户清楚地看到"机制可用，只是没有插件"，而不是"机制本身缺失"。

    幂等：已存在则不动；目录不存在则建。
    """
    import os
    from pathlib import Path
    custom = os.environ.get("AE_SDD_GLOBAL_HOME")
    base = Path(custom) if custom else Path.home()
    registry = base / ".ae-sdd" / "plugins" / "registry.yaml"
    if registry.is_file():
        return  # 已存在，不动用户自定义内容
    try:
        registry.parent.mkdir(parents=True, exist_ok=True)
        registry.write_text(
            "# ae-sdd L2 全局插件注册表（自动生成骨架）\n"
            "# 🆕 v3.5.10 Gap-010：本文件由 distribute.py 自动生成（首次安装时）。\n"
            "# 已存在则不覆盖。请在此追加你个人的全局插件清单。\n"
            "#\n"
            "# Schema 权威文档：source/standards/constraints/plugin-registry-spec.md\n"
            "schema_version: 1\n"
            "description: 用户全局插件注册表（空骨架，按需追加）\n"
            "plugins: []\n",
            encoding="utf-8",
        )
        if not quiet:
            from distributors._base import log_info
            log_info(None, f"已生成 L2 全局注册表骨架：{registry}")
    except OSError:
        # 失败不阻断分发链
        pass


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        log_error("用户中断")
        sys.exit(130)
