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
MASTER_VERSION = "3.5.2"


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
    """Find the project asset file under .ae-sdd/assets/."""
    assets = assets_dir(ade_sdd)
    if not assets.is_dir():
        return None
    cand = assets / f"{project_key}.assets.md"
    return cand if cand.is_file() else None


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


def find_module_asset_files(ade_sdd: Path, project_key: str) -> list:
    """🆕 v4.0：发现工程级子文件（总览 + 各工程细节）。

    返回 [Path, ...]，按"总览在前、子文件在后"排序。
    - 总览：.ae-sdd/assets/{projectKey}.assets.md（find_asset_file）
    - 子文件：docWorkspacePath/assets/{projectKey}/{module}/{module}.assets.md
      （就近存放，A6 规则；兼容旧扁平 .ae-sdd/assets/{projectKey}.*.assets.md）

    总览不存在时返回空列表（调用方按缺失处理）。
    """
    result = []
    overview = find_asset_file(ade_sdd, project_key)
    if overview:
        result.append(overview)

    # 子文件发现：新位置（docWorkspace 下按 module 分目录）
    doc_ws = resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws:
        new_loc = doc_ws / "assets" / project_key
        if new_loc.is_dir():
            for module_dir in sorted(new_loc.iterdir()):
                if module_dir.is_dir():
                    cand = module_dir / f"{module_dir.name}.assets.md"
                    if cand.is_file():
                        result.append(cand)

    # 兼容旧扁平位置：.ae-sdd/assets/{projectKey}.*.assets.md（排除总览本体）
    assets = assets_dir(ade_sdd)
    if assets.is_dir():
        for f in sorted(assets.iterdir()):
            if (f.name.startswith(f"{project_key}.") and f.name.endswith(".assets.md")
                    and f.name != f"{project_key}.assets.md"):
                result.append(f)

    return result


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
