#!/usr/bin/env python3
"""
build_dist.py — ae-sdd 母版 → 实例化分发包 构建脚本

🆕 v3.0.1 跨平台化（2026-06-18）：用 Python 替代 bash，零外部依赖（仅标准库）。
🆕 v3.0 双目录分层：source/（SSOT） → dist/ae-sdd/（构建产物）。

⚠️ v3.0.1 Windows 兼容：用 `git archive` 从 commit 读取 source/（不经过 working tree），
   避免 Windows 的 core.autocrlf 把 LF 转 CRLF。

用法:
    python scripts/build_dist.py
    python scripts/build_dist.py --source /path/to/source --dist /path/to/dist
"""
from __future__ import annotations

import argparse
import hashlib
import re
import shutil
import subprocess
import sys
import tarfile
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
def parse_version_from_bytes(skill_md_bytes: bytes) -> str:
    """从 SKILL.md 字节提取 version 字段（找不到则默认 3.0.0）"""
    text = skill_md_bytes.decode("utf-8")
    m = re.search(r"^version:\s*(\S+)", text, re.MULTILINE)
    return m.group(1) if m else "3.0.0"


def _archive_source_via_git(src: Path) -> bytes:
    """
    用 `git archive` 把 source/ 目录的 commit 版本打成 tar 字节流。

    ⚠ Windows 兼容：直接读 working tree 会被 core.autocrlf 转 CRLF，
       跟 commit 不一致。git archive 输出 commit 字节级。
    """
    # src 形如 .../ae-sdd/source，仓库根 = src.parent
    repo_root = src.parent
    result = subprocess.run(
        ["git", "-C", str(repo_root), "archive", "HEAD", "--", "source/"],
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"git archive 失败: {result.stderr.decode('utf-8', errors='replace')}"
        )
    return result.stdout


def _extract_tar_to(tar_bytes: bytes, dst: Path, ignore_dirs: set, ignore_files: list[Path]) -> None:
    """
    从 tar 字节流解压到 dst，应用 ignore 规则。
    替代 shutil.copytree（避免 working tree 转换行尾）。
    """
    import io
    dst.mkdir(parents=True, exist_ok=True)

    ignore_files_set = {Path(rel).as_posix() for rel in ignore_files}

    with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:") as tar:
        for member in tar.getmembers():
            # member.name 形如 "source/SKILL.md"（带 source/ 前缀）
            rel = Path(member.name)
            if len(rel.parts) < 2 or rel.parts[0] != "source":
                continue
            rel_in_src = Path(*rel.parts[1:])  # 去掉 source/ 前缀

            # 排除规则
            parts = rel_in_src.parts
            if any(p in ignore_dirs for p in parts):
                continue
            if rel_in_src.as_posix() in ignore_files_set:
                continue

            target = dst / rel_in_src
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                f = tar.extractfile(member)
                if f is not None:
                    target.write_bytes(f.read())


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

    # ⚠ 关键：用 git archive 读 commit 字节级（不经过 working tree）
    # working tree 在 Windows 下会被 core.autocrlf 转 CRLF，破坏 hash 一致性
    info("从 git commit 读取 source/（git archive，避免 working tree CRLF 转换）")
    try:
        tar_bytes = _archive_source_via_git(src)
    except RuntimeError as e:
        # Fallback: working tree（无 git 仓库环境）
        warn(f"git archive 失败: {e}")
        warn("回退到 working tree 读取（Windows 上可能 hash 不一致）")
        return _fallback_build(src, dst)

    # 从 tar 字节流读取 SKILL.md 提取 version
    import io
    with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:") as tar:
        skill_member = next((m for m in tar.getmembers()
                            if m.name == "source/SKILL.md"), None)
        if skill_member is None:
            err("致命：git archive 中找不到 source/SKILL.md")
            return 1
        f = tar.extractfile(skill_member)
        skill_bytes = f.read() if f else b""

    version = parse_version_from_bytes(skill_bytes)
    build_date = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    info(f"母版根:   {src}")
    info(f"目标:     {dst}")
    info(f"版本:     {version}")
    info(f"构建时间: {build_date}")

    # ── 整树复制（从 tar 流）────────────────────────────────────────────────
    step("构建实例化分发包")
    if dst.exists():
        shutil.rmtree(dst)
    _extract_tar_to(tar_bytes, dst, EXCLUDE_DIRS, EXCLUDE_FILES)
    ok("整树复制完成（git archive → 字节级一致）")

    # ── 剥离母版专有产物 ────────────────────────────────────────────────────
    step("剥离母版专有产物")
    for rel in EXCLUDE_FILES:
        target = dst / rel
        if target.exists():
            target.unlink()
    ok("剥离完成（marketplace.json / CHANGELOG / docs / .idea）")

    # ── 注入版本信息 ────────────────────────────────────────────────────────
    step("注入版本信息")
    # ⚠ Windows 兼容：强制 binary 模式 + LF
    (dst / "VERSION").write_bytes(f"{version}\n{build_date}\n".encode("utf-8"))
    ok(f"VERSION 文件已写入: {version}")

    plugin_dir = dst / ".claude-plugin"
    plugin_dir.mkdir(exist_ok=True)
    (plugin_dir / "plugin.json").write_bytes(
        "{\n"
        f'  "name": "ae-sdd",\n'
        f'  "version": "{version}",\n'
        '  "description": "ae-sdd 端到端自动化工程 SKILL 体系（v3.0 实例化分发包）",\n'
        f'  "buildDate": "{build_date}",\n'
        '  "mainEntry": "SKILL.md"\n'
        "}\n".encode("utf-8"),
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


def _fallback_build(src: Path, dst: Path) -> int:
    """无 git 仓库环境的回退方案（用 working tree）"""
    warn("⚠ 回退模式：使用 working tree（Windows 上 hash 可能不匹配 commit）")

    skill_md = src / "SKILL.md"
    version = parse_version_from_bytes(skill_md.read_bytes())
    build_date = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst, ignore=_ignore_func, copy_function=shutil.copyfile)

    for rel in EXCLUDE_FILES:
        target = dst / rel
        if target.exists():
            target.unlink()

    (dst / "VERSION").write_bytes(f"{version}\n{build_date}\n".encode("utf-8"))
    plugin_dir = dst / ".claude-plugin"
    plugin_dir.mkdir(exist_ok=True)
    (plugin_dir / "plugin.json").write_bytes(
        "{\n"
        f'  "name": "ae-sdd",\n'
        f'  "version": "{version}",\n'
        '  "description": "ae-sdd 端到端自动化工程 SKILL 体系（v3.0 实例化分发包）",\n'
        f'  "buildDate": "{build_date}",\n'
        '  "mainEntry": "SKILL.md"\n'
        "}\n".encode("utf-8"),
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        err("用户中断")
        sys.exit(130)

