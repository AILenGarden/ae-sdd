#!/usr/bin/env python3
"""
init.py — ae-sdd 项目实例化（Layer 4）

🆕 v3.0 P0 三件套之 init（2026-06-18）：
  给具体项目（如 icec-cloud-boss）创建 .ae-sdd/ 骨架 + 项目资产 + overrides 模板。

🆕 v3.1 Harness 自动注入（2026-06-22）：
  init 完成后自动调用 ae-sdd init-hooks，将 PreToolUse hook 写入项目 .claude/settings.json。
  可用 --no-hooks 跳过（e.g. CI 环境）。

用法:
    python scripts/init.py <project-dir> <project-key> [选项]

示例:
    python scripts/init.py D:/Item/icec-cloud-boss icec-cloud-boss
    python scripts/init.py . icec-cloud-boss --asset-path ./my.assets.md
    python scripts/init.py D:/Item/icec-cloud-boss icec-cloud-boss --dry-run
    python scripts/init.py D:/Item/icec-cloud-boss icec-cloud-boss --force
    python scripts/init.py D:/Item/icec-cloud-boss icec-cloud-boss --no-hooks
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

TOOLS_DIR = Path(__file__).resolve().parent.parent / "tools"
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))
from lib import project_assets  # noqa: E402

for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8")
    except (AttributeError, OSError):
        pass


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


def info(msg: str)    -> None: print(f"{C_BLUE}ℹ️  {msg}{C_RESET}")
def ok(msg: str)   -> None: print(f"{C_GREEN}✅ {msg}{C_RESET}")
def warn(msg: str) -> None: print(f"{C_YELLOW}⚠  {msg}{C_RESET}")
def err(msg: str)  -> None: print(f"{C_RED}❌ {msg}{C_RESET}", file=sys.stderr)
def success(msg: str) -> None: print(f"{C_GREEN}🎉 {msg}{C_RESET}")
def step(msg: str) -> None: print(f"\n{C_BLUE}== {msg} =={C_RESET}")


# ─── 模板 ────────────────────────────────────────────────────────────────────
CONFIG_TEMPLATE = """# ae-sdd 项目实例配置
# 🆕 v3.0 生成时间: {timestamp}
# 不要手工修改此文件（master/source 字段除外）

version: 1
projectKey: {project_key}
gitPath: {git_path}

# 母版引用（指向 ae-sdd 母版仓库）
master:
  # 相对路径：项目内的 ae-sdd 子仓库（推荐，便于开发期同步）
  source: {master_source}
  # 或远程 URL：
  # source: https://github.com/AILenGarden/ae-sdd
  # 🆕 v3.4.0：不再硬编码 3.0.0，由 init 时读母版 source/SKILL.md frontmatter 填充
  version: "{master_version}"
  # 母版根目录：master 的 source/ 子目录（v3.0 双目录分层）
  masterDir: source
  # 🆕 v3.4.0：记录 init 时的母版检查时间 + 分发闭环版本号（PostToolUse hook 检测漂移用）
  lastCheckedAt: "{timestamp}"
  expectedDispChain: build_dist -> install -> harness_adapter -> harness_remount

# 项目资产（必填）
assetPath: assets/{project_key}.assets.md

# Override 目录（项目特化规则覆盖母版）
overrideDir: overrides

# 🆕 v3.8.0 自动化开关（默认关闭；开启后 6 个人工审核点改走 Tier 3 多 reviewer 联审共识）
# 详见 source/SKILL.md §🚀 自动化模式 与 tools/lib/config.py
automation:
  # 总开关：false=现状(每审核点等用户✅) / true=全自动化(审核点走联审共识)
  enabled: false
  # 联审强度：开启后统一强制 Tier 3（业务/架构/第三方视角三审交叉）
  reviewerTier: 3
  # 开工前信息预收集：扫输入材料+资产，列清单让用户一次补齐
  preflightInfoCollection: true
  # 阻断出口：联审 3 轮矫正未决时（pause=state.phase=paused等用户 / fail=标记失败）
  onConsensusStall: pause
  # 审核点白名单（默认全部 6 个走联审；合法值 1/1.5/2/2.5/4/5）
  automatedReviewPoints: [1, 1.5, 2, 2.5, 4, 5]
  # 开启时间戳（审计用，AI 不得自行改；由 ae-sdd automation enable 写入）
  enabledAt: ""

# 上次更新
lastUpdated: {timestamp}
"""

STATE_TEMPLATE = {
    "version": "1",
    "projectKey": None,  # 由 init 填充
    "phase": "initialized",
    "currentStory": None,
    "currentTask": None,
    "history": [
        {
            "phase": "initialized",
            "timestamp": None,  # 由 init 填充
            "by": "ae-sdd init",
        }
    ],
}

OVERRIDES_README = """# 项目覆盖（Overrides）

> **Override 解析规则：** 实例有效规则 = 母版 defaults + overrides/（同名覆盖）

本目录放项目特化规则，**覆盖母版**（`../ae-sdd/source/`）的同名文件。

## 用法

### 1. 约束特化
把母版 `standards/constraints/api.md` 复制到这里，按项目调整字段：
```bash
cp ../ae-sdd/source/standards/constraints/api.md ./api.md
# 编辑 ./api.md 加项目特定内容
```

### 2. 模板特化
把母版 `templates/design/story-template.md` 复制到这里，按项目调整字段（v3.9.3 起 be-story-template.md 已合并入主模板）：
```bash
cp ../ae-sdd/source/templates/design/story-template.md ./story-template.md
# 编辑，加项目元信息字段
```

### 3. 不要复制 SKILL
SKILL 是节点级通用规则，**不实例化**。如有项目特定流程扩展，单独写 `<project>-SKILL.md` 自定义 SKILL，不放在 overrides/。

## 启动时
`ae-sdd` 会先读母版 defaults，然后读 `overrides/` 同名文件覆盖。
"""


# ─── 工具 ────────────────────────────────────────────────────────────────────
def locate_master() -> Optional[Path]:
    """定位母版 source/ 目录（按优先级）：本脚本同仓库 source/ > 上级 ae-sdd/source/ > 失败"""
    script_dir = Path(__file__).resolve().parent
    candidates = [
        script_dir.parent / "source",                      # 同仓库（开发态）
        script_dir.parent.parent / "ae-sdd" / "source",   # 项目内 ae-sdd 子仓库
    ]
    for cand in candidates:
        if cand.is_dir() and (cand / "SKILL.md").is_file():
            return cand
    return None


def locate_master_assets(master_source: Path, project_key: str) -> Optional[Path]:
    """从母版 assets/{projectKey}.assets.md 找项目资产模板"""
    asset = master_source / "assets" / project_key / f"{project_key}.assets.md"
    return asset if asset.is_file() else None


def _read_master_version(master_source: Path) -> str:
    """🆕 v3.4.0：从母版 source/SKILL.md frontmatter 解析 version 字段。

    失败时回退到 paths.MASTER_VERSION（install.py/ae-sdd CLI 一致性来源）。
    返回 "unknown" 表示母版不可读（不影响 init 主流程）。
    """
    skill_md = master_source / "SKILL.md"
    if not skill_md.is_file():
        return "unknown"
    try:
        text = skill_md.read_text(encoding="utf-8")
    except OSError:
        return "unknown"
    # YAML frontmatter 形式：
    # ---
    # version: 3.4.0
    # ---
    import re
    m = re.search(r"^---\s*\n.*?^version:\s*([\d.]+)\s*\n.*?^---", text, re.MULTILINE | re.DOTALL)
    if m:
        return m.group(1)
    # fallback: 读 tools/lib/paths.py MASTER_VERSION 常量
    try:
        import importlib.util
        paths_py = master_source.parent / "tools" / "lib" / "paths.py"
        if paths_py.is_file():
            spec = importlib.util.spec_from_file_location("ae_paths", paths_py)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)  # type: ignore
            return getattr(mod, "MASTER_VERSION", "unknown")
    except Exception:
        pass
    return "unknown"


# ─── 核心流程 ────────────────────────────────────────────────────────────────
def init_project(
    project_dir: Path,
    project_key: str,
    *,
    asset_path: Optional[Path] = None,
    no_asset: bool = False,
    force: bool = False,
    dry_run: bool = False,
    master_source_rel: str = "../ae-sdd",
    no_hooks: bool = False,
) -> int:
    """执行项目实例化；返回 0 成功 / 1 失败"""

    # ── 校验 ────────────────────────────────────────────────────────────────
    project_dir = project_dir.resolve()
    if not project_dir.is_dir():
        err(f"项目目录不存在: {project_dir}")
        return 1

    # projectKey 合法性（kebab-case 字母数字连字符）
    if not project_key.replace("-", "").replace("_", "").isalnum():
        err(f"projectKey 不合法（只允许字母/数字/连字符/下划线）: {project_key}")
        return 1

    target = project_dir / ".ae-sdd"
    if target.exists() and not force:
        err(f"项目目录已存在: {target}")
        err(f"      如需覆盖，加 --force")
        return 1

    # 找母版
    master_source = locate_master()
    if master_source and not dry_run:
        info(f"母版 source/: {master_source}")

    # 找项目资产源
    asset_source: Optional[Path] = None
    if asset_path:
        if not asset_path.is_file():
            err(f"--asset-path 指定的文件不存在: {asset_path}")
            return 1
        asset_source = asset_path
    elif master_source and not no_asset:
        asset_source = locate_master_assets(master_source, project_key)

    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    # ── 打印计划 ────────────────────────────────────────────────────────────
    step("实例化计划")
    print(f"  项目目录:   {project_dir}")
    print(f"  projectKey: {project_key}")
    print(f"  目标:       {target}")
    print(f"  资产来源:   {asset_source if asset_source else '（自动生成 baseline 资产）' if not no_asset else '（不创建）'}")
    print(f"  master 引用: {master_source_rel}")
    if dry_run:
        print(f"  ⚠  dry-run 模式 — 不会真改文件")

    if dry_run:
        return 0

    # ── 创建目录结构 ────────────────────────────────────────────────────────
    step("创建 .ae-sdd/ 目录结构")
    target.mkdir(parents=True, exist_ok=True)
    (target / "assets").mkdir(exist_ok=True)
    (target / "overrides").mkdir(exist_ok=True)
    (target / "reports").mkdir(exist_ok=True)
    ok(f"已创建 {target}/")

    # ── config.yaml ─────────────────────────────────────────────────────────
    step("生成 config.yaml")
    # 🆕 v3.4.0：从母版 SKILL.md frontmatter 读实际 version，硬编码 3.0.0 已废弃
    master_version = _read_master_version(master_source) if master_source else "unknown"
    if master_source:
        info(f"母版 version: {master_version}")
    config_content = CONFIG_TEMPLATE.format(
        timestamp=timestamp,
        project_key=project_key,
        git_path=str(project_dir).replace("\\", "/"),
        master_source=master_source_rel,
        master_version=master_version,
    )
    (target / "config.yaml").write_text(config_content, encoding="utf-8")
    ok(f"已创建 {target/'config.yaml'}")

    # Project-level state is forbidden. A task-scoped state is created later
    # via `ae-sdd state new --id <ID> --entry-node <PRD|DR|STORY|TASK>`.
    step("跳过项目级 state.json")
    info("state 将在 .auto-engineering/{WORKITEM}/state.json 中按任务创建")

    # ── 项目资产 ────────────────────────────────────────────────────────────
    step("项目资产")
    if no_asset:
        warn("跳过项目资产（--no-asset）")
        warn("      后续可跑：ae-sdd assets generate --project " + project_key)
    else:
        result = project_assets.generate_project_assets(
            target,
            project_key,
            project_root=project_dir,
            force=True,
            source_asset=asset_source,
        )
        if result.pass_:
            if asset_source:
                ok(f"已复制并校验 {asset_source} → {result.asset_file}")
            else:
                ok(f"已自动生成 baseline 项目资产 {result.asset_file}")
        else:
            warn(f"项目资产生成后仍缺索引层: {', '.join(result.missing_after)}")

    # ── overrides/README.md ─────────────────────────────────────────────────
    step("生成 overrides/README.md")
    (target / "overrides" / "README.md").write_text(OVERRIDES_README, encoding="utf-8")
    ok(f"已创建 {target/'overrides'/'README.md'}")

    # ── reports/.gitkeep ────────────────────────────────────────────────────
    (target / "reports" / ".gitkeep").touch()
    ok(f"已创建 {target/'reports'/'.gitkeep'}")

    # ── 完成 ────────────────────────────────────────────────────────────────
    step("完成")
    print(f"  目录结构:")
    def _show(p: Path, prefix: str = "    ") -> None:
        print(f"{prefix}{p.name}/")
        for child in sorted(p.iterdir()):
            if child.is_dir():
                _show(child, prefix + "  ")
            else:
                print(f"{prefix}  {child.name}")
    _show(target)

    print()
    success("项目实例化完成！下一步：")
    print(f"  1. 校对 .ae-sdd/config.yaml（master 引用是否正确）")
    print(f"  2. 跑 `ae-sdd assets check --project {project_key}` 校验项目资产")
    print(f"  3. 在 .ae-sdd/overrides/ 添加项目特化规则（可选）")
    print(f"  4. 在 Claude Code 中启动 /ae-sdd 开始第一个 Story")
    print()

    # ── 自动注入 PreToolUse hook（v3.1 Harness 层） ─────────────────────────
    if no_hooks:
        warn("跳过 hook 注入（--no-hooks）")
        warn("      后续可手动跑：ae-sdd init-hooks " + str(project_dir))
    else:
        _run_init_hooks(project_dir)

    return 0


def _run_init_hooks(project_dir: Path) -> None:
    """调用 ae-sdd init-hooks，将 PreToolUse hook 写入项目 .claude/settings.json"""
    step("注入 Harness Hook（PreToolUse）")
    try:
        result = subprocess.run(
            ["ae-sdd", "init-hooks", str(project_dir)],
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if result.stdout:
            print(result.stdout, end="")
        if result.returncode == 0:
            ok("Harness hook 已注入 .claude/settings.json")
            info("  Claude Code 将在每次 Write/Edit/Bash 前执行 ae-sdd gate-intercept")
        else:
            warn(f"ae-sdd init-hooks 退出码 {result.returncode}，请检查输出")
            if result.stderr:
                warn(result.stderr)
            warn("可手动补跑：ae-sdd init-hooks " + str(project_dir))
    except FileNotFoundError:
        warn("ae-sdd 命令未找到（可能未安装到 PATH）")
        warn("请安装后手动跑：ae-sdd init-hooks " + str(project_dir))
        warn("参考：python scripts/install.py 或配置 PATH 指向 tools/bin/")


# ─── CLI 入口 ────────────────────────────────────────────────────────────────
def main() -> int:
    parser = argparse.ArgumentParser(
        description="init: 把 ae-sdd 实例化到具体项目（创建 .ae-sdd/ 骨架）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("project_dir", type=Path,
                        help="项目根目录（将创建 <project_dir>/.ae-sdd/）")
    parser.add_argument("project_key",
                        help="项目标识（kebab-case，对应母版 assets/{projectKey}/）")
    parser.add_argument("--asset-path", type=Path,
                        help="从已有 .assets.md 复制（默认从母版 assets/{projectKey}.assets.md 找）")
    parser.add_argument("--no-asset", action="store_true",
                        help="不创建项目资产（待用户后续生成）")
    parser.add_argument("--master-source", default="../ae-sdd",
                        help="master.source 字段值（默认: ../ae-sdd）")
    parser.add_argument("--no-hooks", action="store_true",
                        help="不自动注入 PreToolUse hook 到 .claude/settings.json")
    parser.add_argument("--force", action="store_true",
                        help="覆盖已有 .ae-sdd/")
    parser.add_argument("--dry-run", action="store_true",
                        help="只打印计划，不真改文件")
    args = parser.parse_args()

    print()
    info("开始项目实例化...")
    print()

    return init_project(
        args.project_dir,
        args.project_key,
        asset_path=args.asset_path,
        no_asset=args.no_asset,
        force=args.force,
        dry_run=args.dry_run,
        master_source_rel=args.master_source,
        no_hooks=args.no_hooks,
    )


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        err("用户中断")
        sys.exit(130)
