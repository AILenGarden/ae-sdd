#!/usr/bin/env python3
"""
build_dist.py — ae-sdd 母版 → 实例化分发包 构建脚本

🆕 v3.0.1 跨平台化（2026-06-18）：用 Python 替代 bash，零外部依赖（仅标准库）。
🆕 v3.0 双目录分层：source/（SSOT） → dist/ae-sdd/（构建产物）。
🆕 v3.1 Harness 层（2026-06-22）：HARNESS.md + harness/ 已纳入分发包（默认包含）。
🆕 v3.2 RA 门禁层（2026-06-24）：运行时真实性扫描器已纳入分发包。
🆕 v3.2.1 Coding 门禁层（2026-06-24）：Coding 真实性扫描器已纳入分发包。

⚠️ v3.0.1 Windows 兼容：用 `git archive` 从 commit 读取 source/（不经过 working tree），
   避免 Windows 的 core.autocrlf 把 LF 转 CRLF。

分发包包含（source/ 下，排除项外）：
  SKILL.md          — 主入口
  HARNESS.md        — Agent Harness（v3.1 新增）
  harness/          — Harness 相关目录（v3.1 新增）
  templates/        — 模板
  standards/        — 约束规则
  skills/           — SKILL 节点
  assets/           — 项目资产模板
  docs/ae-sdd-conventions.md — 约定文档（docs/ 整体被排除，但 ae-sdd-conventions.md 单独保留）
  scripts/test_authenticity_scan.py — 测试真实性扫描器（G-09 运行时依赖）
  scripts/ra_authenticity_scan.py — RA 真实性扫描器（G-RA-4 运行时依赖）
  scripts/coding_authenticity_scan.py — Coding 真实性扫描器（G-CODE-1 运行时依赖）

分发包排除（母版专有，不发给用户）：
  CHANGELOG/        — 开发记录
  docs/plans/       — 设计方案
  .idea/            — IDE 配置
  .claude-plugin/marketplace.json — 内部注册

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


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")


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
#
# 注意：docs/ 整体排除，但 docs/ae-sdd-conventions.md 是面向用户的约定文档，
# 在 _extract_tar_to 里做特例保留（DOCS_KEEP 白名单）。
#
EXCLUDE_DIRS = {"CHANGELOG", "docs", ".idea"}
EXCLUDE_FILES = [".claude-plugin/marketplace.json"]

# docs/ 目录内例外保留的文件（相对 source/ 的路径）
DOCS_KEEP: frozenset[str] = frozenset({
    "docs/ae-sdd-conventions.md",
})


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

    特例：DOCS_KEEP 中的文件即使父目录在 ignore_dirs 也保留。
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

            # 排除规则（白名单例外：DOCS_KEEP 中的文件跳过排除）
            rel_posix = rel_in_src.as_posix()
            if rel_posix not in DOCS_KEEP:
                parts = rel_in_src.parts
                if any(p in ignore_dirs for p in parts):
                    continue
            if rel_posix in ignore_files_set:
                continue

            target = dst / rel_in_src
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                f = tar.extractfile(member)
                if f is not None:
                    target.write_bytes(f.read())


def _copy_tools_to_dist(repo_root: Path, dst: Path) -> None:
    """把 tools/ 复制到 dist，排除 tools/tests/"""
    tools_src = repo_root / "tools"
    tools_dst = dst / "tools"
    if not tools_src.is_dir():
        warn(f"tools/ 目录不存在: {tools_src}，跳过")
        return
    if tools_dst.exists():
        shutil.rmtree(tools_dst)

    def _ignore_tests(_src_dir: str, names: list[str]) -> list[str]:
        return [n for n in names if n == "tests"]

    shutil.copytree(tools_src, tools_dst, ignore=_ignore_tests, copy_function=shutil.copyfile)
    ok("tools/ 已复制到 dist（排除 tools/tests/）")


def _copy_runtime_scripts_to_dist(repo_root: Path, dst: Path) -> None:
    """复制门禁运行时依赖脚本到 dist/scripts/。"""
    scripts_dst = dst / "scripts"
    scripts_dst.mkdir(parents=True, exist_ok=True)

    runtime_scripts = [
        "test_authenticity_scan.py",
        "ra_authenticity_scan.py",
        "coding_authenticity_scan.py",
        "plugin_content_scan.py",  # 🆕 B4 增强：外挂内容安全扫描器
        "flow_violation_scan.py",  # 🆕 2026-06-27 RA 流程违规审计扫描器（G-RA-FLOW-VIOLATION 运行时依赖）
        "ra_depth_scan.py",  # 🆕 v3.5.9 RA 机械派生深度扫描器（G-RA-5 运行时依赖，防「形式通过、内容空转」）
        "ra_implementation_scan.py",  # 🆕 v3.5.18 RA 实现视角完整性扫描器（G-RA-6 运行时依赖）
    ]

    copied = []
    for name in runtime_scripts:
        src_file = repo_root / "scripts" / name
        if not src_file.is_file():
            warn(f"运行时脚本不存在: {src_file}，跳过")
            continue
        shutil.copyfile(src_file, scripts_dst / name)
        copied.append(name)

    if copied:
        ok(f"运行时脚本已复制到 dist/scripts/: {', '.join(copied)}")
    else:
        warn("没有运行时脚本被复制")


def _compile_runtime_to_dist(repo_root: Path, src: Path, dst: Path, build_date: str) -> bool:
    """Generate compact runtime files and replace dist/SKILL.md with bootloader."""
    try:
        from compile_skill_runtime import compile_runtime_package
        manifest = compile_runtime_package(repo_root, src, dst, build_date=build_date)
    except Exception as exc:
        err(f"Runtime 编译失败: {exc}")
        return False

    runtime_manifest = dst / "runtime" / "manifest.json"
    runtime_boot = dst / "runtime" / "boot.compact.md"
    if not runtime_manifest.is_file() or not runtime_boot.is_file():
        err("Runtime 编译产物不完整：缺少 runtime/manifest.json 或 runtime/boot.compact.md")
        return False

    extracts = manifest.get("extracts", {})
    flow_scales = ",".join(extracts.get("flow_scales", []))
    ok(
        "Runtime compact 已生成 "
        f"(gates={extracts.get('gate_count')}, "
        f"scales={flow_scales}, "
        f"subskills={extracts.get('subskill_count', 0)})"
    )
    return True


def _patch_new_source_files(
    src: Path, dst: Path, ignore_dirs: set, ignore_files: list
) -> None:
    """
    把 source/ 里存在于 working tree 但不在 dist 里的文件补充进去。

    用途：git archive 只读已 commit 的文件，staged/untracked 的新文件会漏掉。
    本函数把这些遗漏文件从 working tree 直接复制（补丁模式，只补不覆盖）。
    """
    ignore_files_posix = {Path(f).as_posix() for f in ignore_files}
    patched = []

    for src_file in src.rglob("*"):
        if not src_file.is_file():
            continue
        rel = src_file.relative_to(src)
        rel_posix = rel.as_posix()

        # 排除规则
        if any(p in rel.parts for p in ignore_dirs) and rel_posix not in DOCS_KEEP:
            continue
        if rel_posix in ignore_files_posix:
            continue

        dst_file = dst / rel
        # Overlay changed working-tree files; skip only when bytes are identical.
        if dst_file.exists() and dst_file.read_bytes() == src_file.read_bytes():
            continue  # 已存在，不覆盖（保留 git archive 版本）

        dst_file.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src_file, dst_file)
        patched.append(rel_posix)

    if patched:
        for p in patched:
            ok(f"补丁（working tree）: {p}")
    else:
        info("working tree 补丁：无新增文件")

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

    # ── 🆕 v3.8.1 S-4：生成规则-工具同步 manifest（health 第 10 项依赖）─────────
    # 在复制 tools/ 之前生成到 repo_root/tools/.sync-manifest.json，随 tools/ 一并分发。
    # health 读此 manifest 比对当前文件 hash，检测"同版本内规则-代码漂移"。
    step("生成规则-工具同步 manifest（S-4）")
    try:
        sys.path.insert(0, str(repo_root / "tools"))
        from lib import update_graph as _ug  # noqa: E402
        manifest_path = _ug.write_sync_manifest(repo_root)
        ok(f"sync manifest 已生成: {manifest_path}")
    except Exception as e:
        warn(f"sync manifest 生成失败（不影响构建）: {e}")

    # ── 复制 tools/（working tree，非 git archive，无 CRLF 问题）────────────
    _copy_tools_to_dist(repo_root, dst)

    # ── 复制门禁运行时脚本（G-09 / G-RA-4 / G-CODE-1 需要）──────────────────
    _copy_runtime_scripts_to_dist(repo_root, dst)

    # ── 补充 source/ 里未 commit 的新文件（working tree 补丁）─────────────────
    # git archive 只读已提交的 HEAD，staged/untracked 的新文件不会被打进 tar。
    # 对于 HARNESS.md / harness/ 这类新增文件，从 working tree 直接复制。
    # 只补充存在于 working tree 但不在 dist 里的文件（不覆盖已存在的）。
    _patch_new_source_files(src, dst, EXCLUDE_DIRS, EXCLUDE_FILES)

    # ── 编译 Runtime compact（source 未编译母版 → dist 编译运行包）────────────
    step("编译 Runtime compact")
    if not _compile_runtime_to_dist(repo_root, src, dst, build_date):
        return 1

    # ── 剥离母版专有产物 ────────────────────────────────────────────────────
    step("剥离母版专有产物")
    for rel in EXCLUDE_FILES:
        target = dst / rel
        if target.exists():
            target.unlink()
    ok("剥离完成（marketplace.json / CHANGELOG / docs（保留 ae-sdd-conventions.md）/ .idea）")

    # ── 注入版本信息 ────────────────────────────────────────────────────────
    step("注入版本信息")
    # ⚠ Windows 兼容：强制 binary 模式 + LF
    version = parse_version_from_bytes((dst / "SKILL.md").read_bytes())
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
    if not (dst / "runtime" / "manifest.json").is_file():
        err("致命：compiled runtime manifest 缺失")
        return 1
    ok("compiled runtime manifest 存在")

    # ── 验证 Harness（v3.1）─────────────────────────────────────────────────
    dst_harness = dst / "HARNESS.md"
    if dst_harness.is_file():
        ok(f"HARNESS.md 已包含 ({dst_harness.stat().st_size} 字节)")
    else:
        warn("HARNESS.md 未找到（可能 source/HARNESS.md 尚未 commit）")

    dst_harness_dir = dst / "harness"
    if dst_harness_dir.is_dir():
        ok("harness/ 目录已包含")
    else:
        warn("harness/ 目录未找到")

    # ── 验证 tools/（v3.1）──────────────────────────────────────────────────
    dst_tools_cli = dst / "tools" / "bin" / "ae-sdd"
    if dst_tools_cli.is_file():
        ok(f"tools/bin/ae-sdd 已包含 ({dst_tools_cli.stat().st_size} 字节)")
    else:
        warn("tools/bin/ae-sdd 未找到（tools/ 复制可能失败）")

    dst_gate_intercept = dst / "tools" / "lib" / "gate_intercept.py"
    if dst_gate_intercept.is_file():
        ok(f"tools/lib/gate_intercept.py 已包含 ({dst_gate_intercept.stat().st_size} 字节)")
    else:
        warn("tools/lib/gate_intercept.py 未找到（tools/ 复制可能失败）")

    dst_ra_scanner = dst / "scripts" / "ra_authenticity_scan.py"
    if dst_ra_scanner.is_file():
        ok(f"scripts/ra_authenticity_scan.py 已包含 ({dst_ra_scanner.stat().st_size} 字节)")
    else:
        warn("scripts/ra_authenticity_scan.py 未找到（G-RA-4 将无法执行扫描）")

    dst_coding_scanner = dst / "scripts" / "coding_authenticity_scan.py"
    if dst_coding_scanner.is_file():
        ok(f"scripts/coding_authenticity_scan.py 已包含 ({dst_coding_scanner.stat().st_size} 字节)")
    else:
        warn("scripts/coding_authenticity_scan.py 未找到（G-CODE-1 将无法执行扫描）")

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

    # 回填 DOCS_KEEP 白名单中的文件（因为 _ignore_func 整体排除了 docs/）
    for rel_posix in DOCS_KEEP:
        src_file = src / rel_posix
        dst_file = dst / rel_posix
        if src_file.is_file():
            dst_file.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src_file, dst_file)

    for rel in EXCLUDE_FILES:
        target = dst / rel
        if target.exists():
            target.unlink()

    # fallback 同样复制 tools/
    _copy_tools_to_dist(src.parent, dst)
    _copy_runtime_scripts_to_dist(src.parent, dst)

    if not _compile_runtime_to_dist(src.parent, src, dst, build_date):
        return 1

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

