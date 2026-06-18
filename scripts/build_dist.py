#!/usr/bin/env python3
"""
build_dist.py — ae-sdd 母版 → 实例化分发包 构建脚本

🆕 v3.0.1 跨平台化（2026-06-18）：用 Python 替代 bash，零外部依赖（仅标准库）。
🆕 v3.0 双目录分层：source/（SSOT） → dist/ae-sdd/（构建产物）。

用法:
    python scripts/build_dist.py
    python scripts/build_dist.py --source /path/to/source --dist /path/to/dist
"""
from __future__ import annotations

import argparse
import hashlib
import re
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path


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


# ─── 排除规则（母版专有产物，dist 不携带） ────────────────────────────────────
EXCLUDE_DIRS = {"CHANGELOG", "docs", ".idea"}
EXCLUDE_FILES = [".claude-plugin/marketplace.json"]


def _ignore_func(_src_dir: str, names: list[str]) -> list[str]:
    """shutil.copytree 的 ignore 回调：排除母版专有目录"""
    return [n for n in names if n in EXCLUDE_DIRS]


# ─── 工具函数 ────────────────────────────────────────────────────────────────
def parse_version(skill_md: Path) -> str:
    """从 source/SKILL.md YAML frontmatter 提取 version 字段（找不到则默认 3.0.0）"""
    content = skill_md.read_text(encoding="utf-8")
    m = re.search(r"^version:\s*(\S+)", content, re.MULTILINE)
    return m.group(1) if m else "3.0.0"


def dir_size(path: Path) -> int:
    """递归计算目录总大小（字节）"""
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def human_size(n: int) -> str:
    """字节数转人类可读"""
    for unit in ("B", "K", "M", "G", "T"):
        if n < 1024:
            return f"{n:.0f}{unit}" if unit == "B" else f"{n:.1f}{unit}"
        n /= 1024
    return f"{n:.1f}P"


# ─── 主流程 ──────────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="build_dist: 从 ae-sdd 母版构建实例化分发包",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--source", type=Path, help="母版根（默认: <repo>/source）")
    parser.add_argument("--dist",   type=Path, help="目标分发包（默认: <repo>/dist/ae-sdd）")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    src = args.source.resolve() if args.source else (repo_root / "source")
    dst = args.dist.resolve()   if args.dist   else (repo_root / "dist" / "ae-sdd")

    # ── 前置校验 ────────────────────────────────────────────────────────────
    step("前置校验")
    if not src.is_dir():
        err(f"致命：母版目录不存在: {src}")
        err("      请确认仓库结构 — 母版应在 source/ 下")
        return 1

    skill_md = src / "SKILL.md"
    if not skill_md.is_file() or skill_md.stat().st_size == 0:
        err(f"致命：{skill_md} 缺失或为空，主入口未就绪")
        return 1

    version = parse_version(skill_md)
    build_date = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    info(f"母版根:   {src}")
    info(f"目标:     {dst}")
    info(f"版本:     {version}")
    info(f"构建时间: {build_date}")

    # ── 整树复制 ────────────────────────────────────────────────────────────
    step("构建实例化分发包")
    if dst.exists():
        shutil.rmtree(dst)
    # 不预先 mkdir(dst) — 让 shutil.copytree 自己创建
    shutil.copytree(src, dst, ignore=_ignore_func)
    ok("整树复制完成")

    # ── 剥离母版专有产物 ────────────────────────────────────────────────────
    step("剥离母版专有产物")
    for rel in EXCLUDE_FILES:
        target = dst / rel
        if target.exists():
            target.unlink()
    ok("剥离完成（marketplace.json / CHANGELOG / docs / .idea）")

    # ── 注入版本信息 ────────────────────────────────────────────────────────
    step("注入版本信息")
    (dst / "VERSION").write_text(f"{version}\n{build_date}\n", encoding="utf-8")
    ok(f"VERSION 文件已写入: {version}")

    plugin_dir = dst / ".claude-plugin"
    plugin_dir.mkdir(exist_ok=True)
    (plugin_dir / "plugin.json").write_text(
        "{\n"
        f'  "name": "ae-sdd",\n'
        f'  "version": "{version}",\n'
        '  "description": "ae-sdd 端到端自动化工程 SKILL 体系（v3.0 实例化分发包）",\n'
        f'  "buildDate": "{build_date}",\n'
        '  "mainEntry": "SKILL.md"\n'
        "}\n",
        encoding="utf-8",
    )
    ok(f"plugin.json 已生成: {version} @ {build_date}")

    # ── 验证主入口 ──────────────────────────────────────────────────────────
    step("验证主入口")
    dst_skill = dst / "SKILL.md"
    if not dst_skill.is_file() or dst_skill.stat().st_size == 0:
        err(f"致命：{dst_skill} 缺失或为空，构建失败")
        return 1
    ok("主入口 SKILL.md 存在且非空")

    # ── 摘要 ────────────────────────────────────────────────────────────────
    step("构建摘要")
    print(f"  母版大小:    {human_size(dir_size(src))}")
    print(f"  分发包大小:  {human_size(dir_size(dst))}")
    print(f"  分发包文件数: {sum(1 for _ in dst.rglob('*') if _.is_file())}")
    print(f"  版本:        {version}")
    print(f"  构建时间:    {build_date}")
    print(f"  路径:        {dst}")

    ok("构建完成 — 下一步: python scripts/install.py 安装到本地 Claude skills")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        err("用户中断")
        sys.exit(130)
