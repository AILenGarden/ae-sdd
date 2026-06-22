"""
paths.py — ae-sdd CLI 路径工具

跨平台路径解析：定位母版 source/、项目 .ae-sdd/、state.json、assets/。
"""
from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Optional


# 母版版本（与 source/SKILL.md YAML frontmatter 同步）
MASTER_VERSION = "3.0.0"


def locate_master_source(start: Optional[Path] = None) -> Optional[Path]:
    """
    定位母版 source/ 目录。

    优先级：
    1. 环境变量 AE_SDD_MASTER
    2. 当前工作目录的 ./source（含 SKILL.md）
    3. 工具所在仓库的 ./source（含 SKILL.md，从 tools/bin/ae-sdd 向上两级）
    4. ~/.claude/skills/ae-sdd/source
    """
    candidates: list[Path] = []

    if env := os.environ.get("AE_SDD_MASTER"):
        candidates.append(Path(env))

    cwd = Path.cwd()
    candidates.append(cwd / "source")

    # tools/bin/ae-sdd → tools/bin/ → tools/ → 仓库根
    cli_path = Path(start) if start else Path(__file__).resolve()
    repo_root = cli_path.parent.parent.parent
    candidates.append(repo_root / "source")

    # 全局安装位置
    home = Path.home()
    candidates.append(home / ".claude" / "skills" / "ae-sdd" / "source")

    for cand in candidates:
        if cand.is_dir() and (cand / "SKILL.md").is_file():
            return cand
    return None


def locate_project_ae_sdd(cwd: Optional[Path] = None) -> Optional[Path]:
    """定位当前目录的 .ae-sdd/（向上最多 5 级查找）"""
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
    """
    读项目 .ae-sdd/config.yaml（极简解析，仅 key: value 不依赖 PyYAML）。
    """
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
        # section 头（key: 无值）
        if line.startswith(" ") and current_section:
            # 嵌套（缩进）
            key, _, val = line.strip().partition(":")
            val = val.strip().strip('"').strip("'")
            if val:
                out.setdefault(current_section, {})[key] = val
            continue
        key, _, val = line.partition(":")
        key = key.strip()
        val = val.strip()
        if not val:
            # section 头
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
    """在 .ae-sdd/assets/ 找项目资产文件"""
    assets = assets_dir(ade_sdd)
    if not assets.is_dir():
        return None
    cand = assets / f"{project_key}.assets.md"
    return cand if cand.is_file() else None


# ─── 项目文档目录辅助（G-XX 门禁检查用） ─────────────────────────────────────
def project_root(ade_sdd: Path) -> Path:
    """项目根目录 = .ae-sdd/ 的父目录"""
    return ade_sdd.parent


def project_design_dir(project_root: Path) -> Path:
    """项目设计文档目录（DR / Story / TestCase 存放处）"""
    return project_root / "design"


def project_task_dir(project_root: Path) -> Path:
    """项目 Task 文档目录"""
    return project_root / "task"


def find_doc(project_root: Path, story_id: str, suffix: str) -> Optional[Path]:
    """
    在 project/design/ 和 project/ 两个位置找 {story_id}{suffix} 文档。
    返回第一个存在的；都不存在返回 None。
    """
    candidates = [
        project_design_dir(project_root) / f"{story_id}{suffix}",
        project_root / f"{story_id}{suffix}",
    ]
    for cand in candidates:
        if cand.is_file():
            return cand
    return None


def list_docs(project_root: Path, story_id: str, suffix: str) -> list[Path]:
    """在 project/task/ 下找 {story_id}{suffix} 文档列表（按文件名排序）"""
    task_dir = project_task_dir(project_root)
    if not task_dir.is_dir():
        return []
    return sorted(task_dir.glob(f"{story_id}{suffix}"))
