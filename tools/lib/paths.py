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
MASTER_VERSION = "3.4.0"


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
