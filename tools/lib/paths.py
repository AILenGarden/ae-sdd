"""
paths.py - ae-sdd CLI path helpers.

Resolves master source, project .ae-sdd, state.json, assets, and project docs.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Optional


# Keep in sync with source/SKILL.md YAML frontmatter.
MASTER_VERSION = "3.5.13"


def compare_versions(installed: Optional[str], master: str = MASTER_VERSION) -> Optional[str]:
    """🆕 v3.4.0：版本对比工具，返回 None 表示一致或无法判断；返回字符串表示落后。

    用于 gate_intercept / health 子命令探测"已安装 SKILL 是否落后于母版"。
    支持 semver 形式（"3.4.0"）；非 semver 字符串统一按 0.0.0 处理。

    Examples:
        compare_versions("3.4.0", "3.4.0")  -> None
        compare_versions("3.2.3", "3.4.0")  -> "installed 3.2.3 < master 3.4.0"
        compare_versions("4.0.0", "3.4.0")  -> None  (新于母版不告警)
        compare_versions(None,   "3.4.0")  -> "installed unknown < master 3.4.0"
    """
    if not installed:
        return f"installed unknown < master {master}"
    if installed == master:
        return None

    def _parse(v: str) -> tuple[int, ...]:
        try:
            return tuple(int(x) for x in v.split(".")[:3])
        except (ValueError, AttributeError):
            return (0, 0, 0)

    if _parse(installed) < _parse(master):
        return f"installed {installed} < master {master}"
    return None  # 新于母版不告警（可能是开发版）


def locate_master_source(start: Optional[Path] = None) -> Optional[Path]:
    """
    Locate the master source directory.

    Priority:
    1. AE_SDD_MASTER environment variable, pointing to source/ or package root.
    2. Current working directory ./source or current directory itself.
    3. Repository/package root relative to this tool.
    4. Installed ~/.claude/skills/ae-sdd and ~/.codex/skills/ae-sdd directories.
    """
    candidates: list[Path] = []

    if env := os.environ.get("AE_SDD_MASTER"):
        env_path = Path(env)
        candidates.append(env_path)
        candidates.append(env_path / "source")

    cwd = Path.cwd()
    candidates.append(cwd / "source")
    candidates.append(cwd)

    cli_path = Path(start) if start else Path(__file__).resolve()
    repo_root = cli_path.parent.parent.parent
    candidates.append(repo_root / "source")
    candidates.append(repo_root)

    home = Path.home()
    candidates.append(home / ".claude" / "skills" / "ae-sdd" / "source")
    candidates.append(home / ".claude" / "skills" / "ae-sdd")
    candidates.append(home / ".codex" / "skills" / "ae-sdd" / "skills" / "ae-sdd" / "source")
    candidates.append(home / ".codex" / "skills" / "ae-sdd" / "skills" / "ae-sdd")
    candidates.append(home / ".codex" / "skills" / "ae-sdd" / "source")
    candidates.append(home / ".codex" / "skills" / "ae-sdd")

    seen: set[Path] = set()
    for cand in candidates:
        resolved = cand.expanduser()
        if resolved in seen:
            continue
        seen.add(resolved)
        if resolved.is_dir() and (resolved / "SKILL.md").is_file():
            return resolved
    return None


def locate_project_ae_sdd(cwd: Optional[Path] = None) -> Optional[Path]:
    """Locate .ae-sdd/ from cwd upward, up to five parent levels."""
    cur = (cwd or Path.cwd()).resolve()
    for _ in range(5):
        cand = cur / ".ae-sdd"
        if cand.is_dir() and (cand / "config.yaml").is_file():
            return cand
        if cur.parent == cur:
            break
        cur = cur.parent
    return None


def read_config(ade_sdd: Path) -> dict:
    """Read .ae-sdd/config.yaml with a tiny key/value parser."""
    cfg_path = ade_sdd / "config.yaml"
    if not cfg_path.is_file():
        return {}
    text = cfg_path.read_text(encoding="utf-8")

    out: dict = {}
    current_section: Optional[str] = None
    for line in text.splitlines():
        line = line.split("#", 1)[0].rstrip()
        if not line.strip():
            continue
        if line.startswith(" ") and current_section:
            key, _, val = line.strip().partition(":")
            val = val.strip().strip('"').strip("'")
            if val:
                out.setdefault(current_section, {})[key] = val
            continue
        key, _, val = line.partition(":")
        key = key.strip()
        val = val.strip()
        if not val:
            current_section = key
            out.setdefault(key, {})
        else:
            current_section = None
            val = val.strip('"').strip("'")
            out[key] = val
    return out


def state_path(ade_sdd: Path) -> Path:
    return ade_sdd / "state.json"


def assets_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "assets"


def overrides_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "overrides"


def reports_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "reports"


def find_asset_file(ade_sdd: Path, project_key: str) -> Optional[Path]:
    """Find the project asset (overview) file under .ae-sdd/assets/.

    🔧 v4.1：总览位置从旧 `{assets}/{key}.assets.md` 升级为新模型
    `{assets}/{key}/{key}.assets.md`（与 document-storage §2.3 工作区级索引一致）。
    查找顺序：新位置优先 → 旧位置回退（向后兼容）。
    """
    assets = assets_dir(ade_sdd)
    if not assets.is_dir():
        return None
    # v4.1 新位置：{assets}/{key}/{key}.assets.md（工作区级索引，含 line 分组子目录）
    new_loc = assets / project_key / f"{project_key}.assets.md"
    if new_loc.is_file():
        return new_loc
    # 旧位置回退：{assets}/{key}.assets.md
    old_loc = assets / f"{project_key}.assets.md"
    return old_loc if old_loc.is_file() else None


def read_asset_field(ade_sdd: Path, project_key: str, field: str) -> Optional[str]:
    """🆕 v4.0：从资产 md §1 读取字段（gitPath / docWorkspacePath / productLine 等）。

    支持 markdown 表格格式（| field | `value` |）和 JSON 块格式（"field": "value"）。
    找不到返回 None（调用方按缺省处理）。
    """
    asset_file = find_asset_file(ade_sdd, project_key)
    if asset_file is None or not asset_file.is_file():
        return None
    try:
        text = asset_file.read_text(encoding="utf-8")
    except OSError:
        return None
    # markdown 表格格式：| field | `value` | 或 | field | value |
    import re
    m = re.search(rf"\|\s*{re.escape(field)}\s*\|\s*`?([^|`]+)`?\s*\|", text)
    if m:
        val = m.group(1).strip().strip("`").strip()
        return val if val else None
    # JSON 块格式："field": "value"
    m = re.search(rf'"{re.escape(field)}"\s*:\s*"([^"]+)"', text)
    if m:
        return m.group(1).strip()
    return None


def resolve_doc_workspace(ade_sdd: Path, project_key: str) -> Optional[Path]:
    """🆕 v4.0：解析文档工作区根路径（document-storage §0.5.1 第四维）。

    优先级：资产 md §1 docWorkspacePath > 缺省回退 gitPath > None。
    用于工程级子文件的就近存放基线：docWorkspacePath/assets/{key}/{module}/。
    """
    doc_ws = read_asset_field(ade_sdd, project_key, "docWorkspacePath")
    if doc_ws:
        return Path(doc_ws)
    git_path = read_asset_field(ade_sdd, project_key, "gitPath")
    if git_path:
        return Path(git_path)
    return None


def resolve_assets_base(ade_sdd: Path, project_key: str) -> Optional[Path]:
    """🆕 v4.1：统一资产根定位入口（document-storage §0.5.3 / §2.3 资产路径 SSOT）。

    返回工程级子文件就近存放的基线目录（对齐 §2.3 新模型）：
      {docWorkspacePath}/.ae-sdd/assets/{projectKey}/

    优先级：资产 md §1 docWorkspacePath > 缺省回退 gitPath > None。
    供 find_module_asset_files / gates 共用，消除各处硬编码。

    注意：.ae-sdd/ 是 ae-sdd 在项目工作区的统一根（与 state.json、secrets 同根），
    资产放其下 assets/ 子目录。docWorkspacePath 缺省回退 gitPath 时，
    即 {gitPath}/.ae-sdd/assets/{key}/（与 ade_sdd 自身所在路径一致）。
    """
    doc_ws = resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws:
        return doc_ws / ".ae-sdd" / "assets" / project_key
    return None


def discover_line_groups(base: Path) -> dict:
    """🆕 v4.1：自动区分 base 目录下的子目录是「module 目录」还是「line 分组目录」。

    判定规则（含同名 .assets.md 即 module；否则若含孙级 module 目录即 line）：
      - module 目录：{base}/{name}/{name}.assets.md 存在 → 归 module 列表
      - line 目录：  {base}/{line}/{name}/{name}.assets.md 存在 → 归 line 字典

    Args:
        base: resolve_assets_base() 返回的 {docWorkspace}/assets/{key}/ 目录

    Returns:
        {
            "flat_modules": [Path, ...],   # 直接 module 子文件（{base}/{m}/{m}.assets.md）
            "line_groups": {line: [Path, ...]},  # line 分组下的 module 文件
        }
        无 module 文件时对应项为空。
    """
    flat_modules: list = []
    line_groups: dict = {}

    if not base.is_dir():
        return {"flat_modules": flat_modules, "line_groups": line_groups}

    for child in sorted(base.iterdir()):
        if not child.is_dir():
            continue
        # 情况 1：本层就是 module 目录（含同名 .assets.md）
        own = child / f"{child.name}.assets.md"
        if own.is_file():
            flat_modules.append(own)
            continue
        # 情况 2：本层是 line 分组目录（孙级含 module 目录）
        line_files = []
        for sub in sorted(child.iterdir()):
            if not sub.is_dir():
                continue
            sub_own = sub / f"{sub.name}.assets.md"
            if sub_own.is_file():
                line_files.append(sub_own)
        if line_files:
            line_groups[child.name] = line_files

    return {"flat_modules": flat_modules, "line_groups": line_groups}


def find_module_asset_files(ade_sdd: Path, project_key: str) -> list:
    """🆕 v4.0 / 🔧 v4.1：发现工程级子文件（总览 + 各工程细节），支持 line 分组。

    返回 [Path, ...]，按"总览在前、子文件在后"排序。子文件发现走三阶段（共存向后兼容）：

      阶段① line 分组：{docWorkspacePath}/assets/{key}/{line}/{module}/{module}.assets.md
              （多业务线项目，如 life 的 2c/admin/common）
      阶段② 单层 module：{docWorkspacePath}/assets/{key}/{module}/{module}.assets.md
              （v4.0 原就近存放规则，单业务线项目）
      阶段③ 旧扁平兼容：.ae-sdd/assets/{key}.*.assets.md
              （历史扁平格式，paths.find_module_asset_files 一直兼容）

    总览：.ae-sdd/assets/{projectKey}.assets.md（find_asset_file），不存在时返回空列表。
    """
    result = []
    overview = find_asset_file(ade_sdd, project_key)
    if overview:
        result.append(overview)

    # 阶段①②：经 docWorkspace 就近存放发现（line 分组 + 单层 module 共用 discover_line_groups）
    base = resolve_assets_base(ade_sdd, project_key)
    if base and base.is_dir():
        discovered = discover_line_groups(base)
        # 阶段① line 分组（按 line 名排序，保证稳定顺序；多业务线项目优先）
        for line_name in sorted(discovered["line_groups"]):
            result.extend(discovered["line_groups"][line_name])
        # 阶段② 单层 module（单业务线项目 / v4.0 原就近存放规则）
        result.extend(discovered["flat_modules"])

    # 阶段③ 兼容旧扁平位置：.ae-sdd/assets/{projectKey}.*.assets.md（排除总览本体）
    assets = assets_dir(ade_sdd)
    if assets.is_dir():
        for f in sorted(assets.iterdir()):
            if (f.name.startswith(f"{project_key}.") and f.name.endswith(".assets.md")
                    and f.name != f"{project_key}.assets.md"):
                result.append(f)

    return result


# ─── 🆕 v4.1 高频路径函数（消除 gates.py / CLI / 其他模块各自硬编码）─────────────
# 这些是扫描报告 B 类发现的"绕过 paths 自拼"高频点，统一收敛到此处。

def config_path(ade_sdd: Path) -> Path:
    """🆕 v4.1：.ae-sdd/config.yaml 路径（消除 gates.py:126 等处 `ade_sdd / "config.yaml"` 自拼）。"""
    return ade_sdd / "config.yaml"


def secrets_dir(ade_sdd: Path) -> Path:
    """🆕 v4.1：.ae-sdd/secrets/ 路径（消除 db_tool.py:58 等处 `ade_sdd / "secrets"` 自拼）。"""
    return ade_sdd / "secrets"


def scripts_dir(master_source: Path) -> Path:
    """🆕 v4.1：母版 scripts/ 目录（消除 gates.py:449 / coding-skill:780 等处自拼）。

    master_source 通常是 source/，scripts/ 在其父目录（仓库根）。
    优先级：master/scripts → master.parent/scripts → master/source/scripts。
    """
    for cand in (master_source / "scripts", master_source.parent / "scripts",
                 master_source / "source" / "scripts"):
        if cand.is_dir():
            return cand
    return master_source.parent / "scripts"  # 缺省指向仓库根 scripts/


def repo_root_from_file(file_path: Path) -> Path:
    """🆕 v4.1：从 __file__ 推导仓库根（消除 CLI 4 处 + plugin_loader 的 `parent.parent.parent` 重复自拼）。

    约定：tools/ 与 scripts/ 均位于仓库根下一层，故 file_path.parent.parent.parent 即仓库根。
    不假设固定层数则需向上找 .git，但此处用约定层数（3 层）保证确定性。
    """
    return file_path.resolve().parent.parent.parent


def project_root(ade_sdd: Path) -> Path:
    """Project root is the parent directory of .ae-sdd/."""
    return ade_sdd.parent


def project_design_dir(project_root: Path) -> Path:
    """Project design docs directory for DR, Story, and TestCase docs."""
    return project_root / "design"


def project_task_dir(project_root: Path) -> Path:
    """Project Task docs directory."""
    return project_root / "task"


def find_doc(project_root: Path, story_id: str, suffix: str) -> Optional[Path]:
    """Find the first existing {story_id}{suffix} doc in design/ or project root."""
    candidates = [
        project_design_dir(project_root) / f"{story_id}{suffix}",
        project_root / f"{story_id}{suffix}",
    ]
    for cand in candidates:
        if cand.is_file():
            return cand
    return None


def list_docs(project_root: Path, story_id: str, suffix: str) -> list[Path]:
    """List {story_id}{suffix} docs under the project task directory."""
    task_dir = project_task_dir(project_root)
    if not task_dir.is_dir():
        return []
    return sorted(task_dir.glob(f"{story_id}{suffix}"))
