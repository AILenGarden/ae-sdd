#!/usr/bin/env python3
"""
build_harness.py — ae-sdd 母版 → Mavis harness 格式（agent.md）编译脚本

由 convert-ae-sdd-to-harness.ps1 迁移而来（决策3：PS1→Python），逐功能对齐：
  - Get-AeSddVersion 三级 fallback（SKILL.md frontmatter → commit msg vX.Y.Z → git short hash）
  - Parse-SkillFrontmatter（name / version / description block）
  - 多维幂等锁（commit + ae_sdd_version + adapter_version + templateHash）
  - Render-Template（{{VAR}} 替换）
  - 无 BOM UTF-8 写入（mavis frontmatter 正则 ^--- 要求文件首字节为 '-'）
  - mount 失败回滚产物（agent.md + lock）
  - mavis CLI 探测（mavis / mavis.cmd / mavis.bat）
  - -DryRun / -Force / -Unmount / -Clean 等价参数

产物落在 <Source>/.harness/agent.md —— mavis 的 findHarnessDirs 优先扫描
<sourceRoot>/.harness/ 下的 identity 文件。.adapter.lock 同目录保存，用于
adapter 幂等判断，不参与 Mavis identity 解析。

用法:
    python scripts/build_harness.py                          # 默认 Source=脚本父父目录
    python scripts/build_harness.py --source /path/to/ae-sdd
    python scripts/build_harness.py --dry-run                # 只看 diff 不写文件
    python scripts/build_harness.py --force                  # 强制重转（忽略幂等锁）
    python scripts/build_harness.py --unmount                # 反向：mavis harness unmount
    python scripts/build_harness.py --unmount --clean        # 卸载 + 删产物目录
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

ADAPTER_VERSION = "0.3.0"   # 幂等锁比对用；v0.3 起以 source_input_sha256 为主键
LEGACY_HARNESS_DIR = "harness"


# ─── 颜色（ANSI） ────────────────────────────────────────────────────────────
def _supports_color() -> bool:
    return sys.stdout.isatty()


if _supports_color():
    C_CYAN = "\033[0;36m"
    C_GREEN = "\033[0;32m"
    C_YELLOW = "\033[0;33m"
    C_RED = "\033[0;31m"
    C_GRAY = "\033[0;90m"
    C_RESET = "\033[0m"
else:
    C_CYAN = C_GREEN = C_YELLOW = C_RED = C_GRAY = C_RESET = ""


def step(msg: str) -> None: print(f"\n{C_CYAN}==> {msg}{C_RESET}")
def ok(msg: str) -> None:   print(f"  {C_GREEN}[OK]{C_RESET} {msg}")
def warn(msg: str) -> None: print(f"  {C_YELLOW}[WARN]{C_RESET} {msg}")
def err(msg: str) -> None:  print(f"  {C_RED}[ERR]{C_RESET} {msg}", file=sys.stderr)


# ─── 备份轮转（治 K2：agent.md.bak.* 无限累积） ──────────────────────────────
# 每次构建都 shutil.copy2 一个 .bak.<timestamp>，旧版无清理逻辑 → 30+ 个 bak 永久
# 堆积。本函数在备份后调用，按 mtime 降序保留最近 keep 个，删其余。
def cleanup_old_bak(target: Path, keep: int = 3) -> int:
    """删除 target 同目录下旧 .bak.<ts> 文件，保留最近 keep 个。返回删除数。"""
    if keep < 0:
        return 0
    baks = sorted(
        target.parent.glob(f"{target.name}.bak.*"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    removed = 0
    for old in baks[keep:]:
        try:
            old.unlink()
            removed += 1
        except OSError:
            pass  # 并发构建竞态：文件已被删则跳过
    return removed


def mavis_harness_name_for_path(source_path: Path) -> str:
    """Match Mavis HarnessManager.toKebabCase(sourcePath) for local mounts."""
    return re.sub(r"--+", "-", re.sub(r"[^a-z0-9-]+", "-", str(source_path).lower())).strip("-")


def cleanup_legacy_harness_artifacts(src: Path, quiet: bool = False) -> int:
    """Remove generated pre-v3.8 harness/harness artifacts so Mavis does not mount duplicates."""
    legacy_root = src / LEGACY_HARNESS_DIR
    legacy_target = legacy_root / ".harness"
    removed = 0

    for path in [
        legacy_target / "agent.md",
        legacy_target / "README.md",
        legacy_root / ".adapter.lock",
    ]:
        try:
            if path.is_file():
                path.unlink()
                removed += 1
        except OSError:
            pass

    try:
        for bak in legacy_target.glob("agent.md.bak.*"):
            try:
                bak.unlink()
                removed += 1
            except OSError:
                pass
    except OSError:
        pass

    for directory in [legacy_target, legacy_root]:
        try:
            directory.rmdir()
            removed += 1
        except OSError:
            pass

    if removed and not quiet:
        warn(f"removed {removed} legacy harness artifact(s) under {legacy_root}")
    return removed


# ─── 元数据提取（对齐 PS1 Get-AeSddVersion / Get-AeSddCommitHash） ──────────
def get_ae_sdd_version(src: Path) -> str:
    """三级 fallback（迁自 PS1 Get-AeSddVersion）。

    1. source/SKILL.md frontmatter 的 version 字段（最权威，跟 paths.py MASTER_VERSION 一致）
    2. commit message 里的 vX.Y.Z
    3. git short hash → "git-<hash>"
    """
    # 1. SKILL.md frontmatter
    skill_path = src / "source" / "SKILL.md"
    if skill_path.is_file():
        try:
            content = skill_path.read_text(encoding="utf-8")
            m = re.search(r"(?s)^---\s*\r?\n.*?^version:\s*(\d+\.\d+\.\d+).*?^---",
                          content, re.MULTILINE)
            if m:
                return m.group(1)
        except Exception:
            pass

    # 2. commit message vX.Y.Z
    try:
        result = subprocess.run(
            ["git", "-C", str(src), "log", "-1", "--format=%s"],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            m = re.search(r"v(\d+\.\d+\.\d+)", result.stdout)
            if m:
                return m.group(1)
    except Exception:
        pass

    # 3. git short hash
    try:
        result = subprocess.run(
            ["git", "-C", str(src), "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True,
        )
        if result.returncode == 0 and result.stdout.strip():
            return "git-" + result.stdout.strip()
    except Exception:
        pass
    return "unknown"


def get_commit_hash(src: Path) -> str:
    """迁自 PS1 Get-AeSddCommitHash。"""
    try:
        result = subprocess.run(
            ["git", "-C", str(src), "rev-parse", "HEAD"],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except Exception:
        pass
    return "unknown"


def get_tree_hash(commit_hash: str, src: Path) -> Optional[str]:
    """🆕 v3.5.6：取指定 commit 的 tree hash（用于 amend 检测）。

    amend 后 commit hash 会变（新 hash），但 tree hash 不变（同内容），
    借此区分"amend 重转"和"真实内容变更"。
    """
    if commit_hash in ("unknown", ""):
        return None
    try:
        result = subprocess.run(
            ["git", "-C", str(src), "rev-parse", f"{commit_hash}^{{tree}}"],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except Exception:
        pass
    return None


# ─── SKILL frontmatter 解析（对齐 PS1 Parse-SkillFrontmatter） ──────────────
def parse_skill_frontmatter(path: Path) -> dict:
    """解析 SKILL.md frontmatter 的 name/version/description。

    迁自 PS1 Parse-SkillFrontmatter。description 是 `|` 块标量，
    后续缩进行拼接（去首尾空白），遇 `  key:` 形式的下一个键停止。
    """
    if not path.is_file():
        raise FileNotFoundError(f"SKILL.md not found: {path}")
    content = path.read_text(encoding="utf-8")
    m = re.search(r"(?s)^---[ \t]*\r?\n(.*?)\r?\n---", content)
    if not m:
        raise ValueError(f"SKILL.md frontmatter not found in {path}")
    yaml = m.group(1)
    result: dict = {}
    lines = yaml.split("\n")
    i = 0
    while i < len(lines):
        line = lines[i]
        kv = re.match(r"^(name|version):\s*(.+)$", line)
        if kv:
            result[kv.group(1)] = kv.group(2).strip()
        elif re.match(r"^description:\s*\|", line):
            desc_lines: list[str] = []
            i += 1
            while i < len(lines):
                nxt = lines[i]
                # 缩进行且不是下一个键（形如 "  word:"）
                if re.match(r"^\s+(.+)$", nxt) and not re.match(r"^\s+\w+:$", nxt):
                    desc_lines.append(nxt.strip())
                    i += 1
                else:
                    break
            result["description"] = " ".join(desc_lines).strip()
            continue
        i += 1
    return result


# ─── 模板渲染（对齐 PS1 Render-Template） ────────────────────────────────────
def render_template(template_path: Path, vars: dict) -> str:
    """{{VAR}} 占位符替换（迁自 PS1 Render-Template，纯 str.replace）。"""
    content = template_path.read_text(encoding="utf-8")
    for key, value in vars.items():
        content = content.replace("{{" + key + "}}", value)
    return content


# ─── 幂等锁（对齐 PS1 Read-AdapterLock + 多维比对） ──────────────────────────
def read_adapter_lock(lock_path: Path) -> Optional[dict]:
    if not lock_path.is_file():
        return None
    try:
        return json.loads(lock_path.read_text(encoding="utf-8"))
    except Exception:
        return None


def template_hash(template_path: Path) -> str:
    """SHA1（对齐 PS1 Get-FileHash -Algorithm SHA1）。"""
    h = hashlib.sha1()
    h.update(template_path.read_bytes())
    return h.hexdigest().upper()


def source_input_hash(src: Path, template_agent: Path, template_readme: Path) -> str:
    """Hash only the inputs that affect generated Mavis harness bytes."""
    h = hashlib.sha256()
    for label, path in [
        ("adapter_version", None),
        ("source/SKILL.md", src / "source" / "SKILL.md"),
        ("source/HARNESS.md", src / "source" / "HARNESS.md"),
        ("scripts/templates/agent.md.template", template_agent),
        ("scripts/templates/README.md.template", template_readme),
    ]:
        h.update(label.encode("utf-8"))
        h.update(b"\0")
        if path is None:
            data = ADAPTER_VERSION.encode("utf-8")
        elif path.is_file():
            data = path.read_bytes()
        else:
            data = b""
        h.update(data)
        h.update(b"\0")
    return h.hexdigest()


# ─── mavis CLI 探测（对齐 PS1 mount 预检） ───────────────────────────────────
def find_mavis_cmd() -> Optional[list[str]]:
    """探测 mavis 可执行命令（迁自 PS1 mavisCmd 探测 + post-commit MAVIS_RUN）。

    返回命令前缀列表（含可能的 cmd.exe 包装），未找到返回 None。
    """
    # 1. PATH 里的 mavis / mavis.exe
    for name in ("mavis", "mavis.exe"):
        path = shutil.which(name)
        if path:
            return [path]
    # 2. ~/.mavis/bin/mavis.cmd / mavis.bat（Windows，需经 cmd.exe）
    home = Path.home()
    for fname in ("mavis.cmd", "mavis.bat"):
        cand = home / ".mavis" / "bin" / fname
        if cand.is_file():
            return ["cmd.exe", "/c", str(cand)]
    return None


def run_mavis(args: list[str]) -> tuple[int, str]:
    """跑 mavis 子命令，返回 (returncode, combined_output)。"""
    mavis_prefix = find_mavis_cmd()
    if mavis_prefix is None:
        return 127, "mavis not found"
    full = mavis_prefix + args
    result = subprocess.run(full, capture_output=True, text=True)
    out = (result.stdout or "") + (result.stderr or "")
    return result.returncode, out


# ─── 主流程 ──────────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="build_harness: ae-sdd 母版 → Mavis harness (agent.md) 编译脚本",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    # 默认 Source = 脚本父父目录（仓库根）
    default_src = Path(__file__).resolve().parent.parent
    parser.add_argument("--source", type=Path, default=default_src,
                        help=f"ae-sdd 母版根（默认: {default_src}）")
    parser.add_argument("--dry-run", action="store_true",
                        help="只打印 diff，不写文件（默认关闭，对齐 PS1 v3.4.0）")
    parser.add_argument("--force", action="store_true",
                        help="强制重转，忽略幂等锁")
    parser.add_argument("--unmount", action="store_true",
                        help="反向：mavis harness unmount 当前路径名及历史别名")
    parser.add_argument("--clean", action="store_true",
                        help="配合 --unmount：同时删除 harness 产物目录")
    parser.add_argument("--no-mount", action="store_true",
                        help="只写产物不触发 mavis mount（CI/无 mavis 环境用）")
    args = parser.parse_args()

    src = args.source.resolve()
    scripts_dir = Path(__file__).resolve().parent
    template_agent = scripts_dir / "templates" / "agent.md.template"
    template_readme = scripts_dir / "templates" / "README.md.template"

    harness_root = src
    target_root = src / ".harness"
    target_agent = target_root / "agent.md"
    target_readme = target_root / "README.md"
    lock_file = target_root / ".adapter.lock"
    source_skill = src / "source" / "SKILL.md"
    source_harness = src / "source" / "HARNESS.md"

    # ── 反向：unmount 模式 ──────────────────────────────────────────────────
    if args.unmount:
        step("Unmount mode")
        mount_names = [
            mavis_harness_name_for_path(src),
            mavis_harness_name_for_path(src / LEGACY_HARNESS_DIR),
            "ae-sdd",
        ]
        print("  Will run:")
        for name in dict.fromkeys(mount_names):
            print(f"    mavis harness unmount {name}")
        if not args.dry_run:
            for name in dict.fromkeys(mount_names):
                rc, out = run_mavis(["harness", "unmount", name])
                print(out)
        else:
            print("  [DRY-RUN] would run the unmount commands above")
        if args.clean:
            print(f"  Will remove: {target_root}")
            if not args.dry_run and target_root.exists():
                shutil.rmtree(target_root)
        else:
            print("  (use --clean to also remove .harness/ dir)")
        return 0

    # ── 1. 前置校验 ─────────────────────────────────────────────────────────
    step("Pre-flight checks")
    if not src.is_dir():
        err(f"ae-sdd master not found: {src}")
        return 1
    ok(f"Master path: {src}")
    if not source_skill.is_file():
        err(f"Master SKILL.md missing: {source_skill}")
        return 1
    ok("source/SKILL.md: exists")
    if not source_harness.is_file():
        warn(f"Master HARNESS.md missing: {source_harness} (harness wrapper will lack gate/state refs)")

    if not template_agent.is_file():
        err(f"agent.md.template missing: {template_agent}")
        return 1
    if not template_readme.is_file():
        err(f"README.md.template missing: {template_readme}")
        return 1

    # ── 2. 读 ae-sdd 元数据 ────────────────────────────────────────────────
    step("Reading ae-sdd metadata")
    version = get_ae_sdd_version(src)
    commit = get_commit_hash(src)
    frontmatter = parse_skill_frontmatter(source_skill)
    skill_name = frontmatter.get("name", "ae-sdd")
    skill_description = frontmatter.get("description", "(no description in frontmatter)")

    print(f"  version:     {version}")
    print(f"  commit:      {commit}")
    print(f"  skill.name:  {skill_name}")
    desc_short = skill_description[:80]
    print(f"  description: {desc_short}...")

    # ── 3. 幂等检查（多维：commit + version + adapter + templateHash） ──────
    step("Idempotency check")
    lock = read_adapter_lock(lock_file)
    tpl_hash = template_hash(template_agent)
    input_hash = source_input_hash(src, template_agent, template_readme)
    should_convert = True
    reason = ""

    if lock and not args.force and "source_input_sha256" in lock:
        drift = []
        if lock.get("source_input_sha256") != input_hash:
            drift.append("source input hash changed")
        if lock.get("ae_sdd_version") != version:
            drift.append(f"ae_sdd_version {lock.get('ae_sdd_version')}->{version}")
        if lock.get("adapter_version") != ADAPTER_VERSION:
            drift.append(f"adapter {lock.get('adapter_version')}->{ADAPTER_VERSION}")
        if lock.get("templateHash") != tpl_hash:
            drift.append("agent template changed")
        if not drift:
            should_convert = False
            reason = (
                f"source inputs unchanged "
                f"(hash={input_hash[:12]}, version={version}, adapter={ADAPTER_VERSION})"
            )
        else:
            reason = "detected drift: " + "; ".join(drift)
    elif lock and not args.force:
        # 🆕 v3.5.6：tree-hash 一致性提前返回（修 amend 循环）
        # amend 后 commit hash 会变（新 hash），但 tree hash 不变（同内容），
        # 借此区分"amend 重转"和"真实内容变更"。
        lock_commit = lock.get("commit", "")
        if lock_commit and lock_commit != commit:
            head_tree = get_tree_hash(commit, src)
            lock_tree = get_tree_hash(lock_commit, src)
            if head_tree and lock_tree and head_tree == lock_tree:
                should_convert = False
                reason = (f"tree-hash 一致 (lock→HEAD 是 amend/amend-like 操作，"
                          f"无内容变更; commit {lock_commit[:7]}→{commit[:7]}, tree={head_tree[:7]})")
        if should_convert:
            drift = []
            if lock.get("commit") != commit:
                drift.append(f"commit {str(lock.get('commit',''))[:7]}→{commit[:7]}")
            if lock.get("ae_sdd_version") != version:
                drift.append(f"ae_sdd_version {lock.get('ae_sdd_version')}→{version}")
            if lock.get("adapter_version") != ADAPTER_VERSION:
                drift.append(f"adapter {lock.get('adapter_version')}→{ADAPTER_VERSION}")
            if lock.get("templateHash") != tpl_hash:
                drift.append("template 内容变化")
            if not drift:
                should_convert = False
                reason = f"全部一致 (commit={commit[:7]}, version={version}, adapter={ADAPTER_VERSION})"
            else:
                reason = "检测到漂移: " + "; ".join(drift)
    elif args.force:
        reason = "--force forced re-convert"
    else:
        reason = "first conversion (no lock file)"

    if not should_convert:
        print(f"  [SKIP] {reason}")
        print("  (use --force to override)")
        return 0
    ok(f"Will re-convert: {reason}")

    # ── 4. 渲染模板 ────────────────────────────────────────────────────────
    step("Rendering agent.md")
    commit_short = commit[:7]
    input_hash_short = input_hash[:16]
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    vars_map = {
        "AUTO_GEN_HEADER": f"# AUTO-GEN @ ae-sdd-source@{input_hash_short} @ {timestamp}",
        "SKILL_DESCRIPTION": skill_description,
        "AE_SDD_VERSION": version,
        "AE_SDD_COMMIT_HASH": commit_short,
        "AE_SDD_SOURCE_HASH": input_hash,
        "AE_SDD_SOURCE_HASH_SHORT": input_hash_short,
        "ADAPTER_VERSION": ADAPTER_VERSION,
        "TIMESTAMP": timestamp,
    }
    agent_content = render_template(template_agent, vars_map)
    readme_content = render_template(template_readme, vars_map)

    print(f"  agent.md  length: {len(agent_content)} chars")
    print(f"  README.md length: {len(readme_content)} chars")

    # ── 5. Dry-run ─────────────────────────────────────────────────────────
    if args.dry_run:
        step("DRY-RUN mode - no files will be written")
        print()
        print("  Operations that WOULD be performed:")
        print(f"    1. Create dir:        {target_root}")
        print(f"    2. Backup old agent.md (if exists): {target_agent}.bak.<timestamp>")
        print(f"    3. Write:             {target_agent}  ({len(agent_content)} chars)")
        print(f"    4. Write:             {target_readme} ({len(readme_content)} chars)")
        print(f"    5. Write lock:        {lock_file}")
        print(f"    6. Verify mount:      mavis harness mount {harness_root}")
        print(f"    7. Remove legacy generated harness artifacts under: {src / LEGACY_HARNESS_DIR}")
        print()
        print("  agent.md preview (first 25 lines):")
        print("  " + "-" * 40)
        for line in agent_content.split("\n")[:25]:
            print(f"  {line}")
        print("  " + "-" * 40)
        print()
        print("  Remove --dry-run to actually execute.")
        return 0

    # ── 6. 备份 ────────────────────────────────────────────────────────────
    step("Backing up old artifacts")
    if target_root.exists():
        backup_ts = datetime.now().strftime("%Y%m%dT%H%M%S")
        if target_agent.is_file():
            bak = target_agent.with_name(f"{target_agent.name}.bak.{backup_ts}")
            shutil.copy2(target_agent, bak)
            ok(f"backup -> {bak}")
            # 备份轮转：保留最近 3 个，删旧 bak（治 K2 无限累积）
            removed = cleanup_old_bak(target_agent, keep=3)
            if removed:
                ok(f"rotated {removed} old .bak file(s) (kept 3)")
    else:
        target_root.mkdir(parents=True, exist_ok=True)
        ok(f"created dir: {target_root}")

    # ── 7. 写产物（无 BOM UTF-8，对齐 PS1 Write-AllTextNoBom） ─────────────
    step("Writing artifacts")
    # ⚠ Python 写 bytes 默认无 BOM；mavis frontmatter 正则 ^--- 要求首字节为 '-'
    target_agent.write_bytes(agent_content.encode("utf-8"))
    ok(str(target_agent))
    target_readme.write_bytes(readme_content.encode("utf-8"))
    ok(str(target_readme))

    # ── 8. 写幂等锁 ────────────────────────────────────────────────────────
    lock_data = {
        "adapter_version": ADAPTER_VERSION,
        "ae_sdd_version": version,
        "source_input_sha256": input_hash,
        "source_commit": commit,
        "templateHash": tpl_hash,
        "converted_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    lock_file.write_bytes(json.dumps(lock_data, indent=2).encode("utf-8"))
    ok(str(lock_file))
    cleanup_legacy_harness_artifacts(src)

    # ── 9. 验证 mount（对齐 PS1，缺失优雅降级） ────────────────────────────
    if args.no_mount:
        step("Skipping mount (--no-mount)")
        step("DONE")
        print(f"  Harness path:  {target_root}")
        print(f"  Mount command: mavis harness mount {harness_root}")
        return 0

    step("Verifying mount")
    mavis_prefix = find_mavis_cmd()
    if mavis_prefix is None:
        warn(f"mavis 未找到（产物已写到 {target_root}，请手动执行：mavis harness mount {harness_root}）")
        warn("退出码 0 — 产物已落地，仅 mavis mount 未触发")
        return 0

    rc, out = run_mavis(["harness", "mount", str(harness_root)])
    print("  mavis harness mount output:")
    for line in out.splitlines():
        print(f"    {line}")
    if rc == 0:
        ok("mavis harness mounted")
    else:
        err(f"mount failed (rc={rc})")
        # mount 失败回滚产物（避免 commit 不变 SKIP → 永久卡在错的 agent.md）
        if target_agent.exists():
            target_agent.unlink()
            warn(f"已回滚 {target_agent}（mount 失败时不留半成品）")
        if lock_file.exists():
            lock_file.unlink()
            warn(f"已回滚 {lock_file}")
        return 1

    # ── 10. 验证 list ──────────────────────────────────────────────────────
    step("Verifying harness list")
    rc, out = run_mavis(["harness", "list"])
    print(out)
    if rc != 0:
        err("list command failed")
        return 1
    expected_name = mavis_harness_name_for_path(src)
    if "ae-sdd" not in out and expected_name not in out:
        err(f"mavis harness list did not include ae-sdd/{expected_name}")
        return 1

    step("DONE")
    print(f"  Harness path:  {target_root}")
    print(f"  Mount command: mavis harness mount {harness_root}")
    print(f"  Unmount:       mavis harness unmount {mavis_harness_name_for_path(src)}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        err("用户中断")
        sys.exit(130)
