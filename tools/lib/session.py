"""
session.py — ae-sdd 入口凭证（entry token）管理（🆕 v3.9.3 — 走 R6 顶层命名空间）

🆕 v3.9.3 改造要点（v3.4.0 关卡1 凭证机制升级）：
  - session.json 与 state.json 共用 R6 顶层目录 `{项目根}/.auto-engineering/{R6 顶层名}/`
  - 废除 v3.8.2 双段 `{ID}--{name}` 拼装（旁路 L1 根治）
  - 废除 raw story_id 直接拼路径（避免 `STORY-004-BE/session.json` 命名空间分裂）
  - 入口凭证必须显式传 (top_node, features)，由调用方派生 R6 顶层名

调用契约：
  session.enter(project_key, top_node, features, ade_sdd)
  - top_node: "PRD" / "DR" / "STORY" / "TASK"
  - features: dict（见 paths.work_item_dir_name）
  - ade_sdd: 项目 .ae-sdd 目录

落盘路径：
  {project_root}/.auto-engineering/{R6 顶层名}/session.json

幂等键：
  projectKey + workItemKey（R6 顶层名）
"""
from __future__ import annotations

import json
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from lib import paths as paths_mod


def _now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def session_path(ade_sdd: Optional[Path], top_node: str = "",
                 features: Optional[dict] = None,
                 story_id: str = "") -> Path:
    """🆕 v3.9.3 session.json 与 state.json 共用 R6 顶层目录。

    🆕 v3.9.3 向后兼容：旧调用 session_path(ade_sdd, story_id)（raw story_id）
    自动识别并走 STORY 顶层名 + story_ids=[story_id]。

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        top_node: 顶层节点类型 "PRD" / "DR" / "STORY" / "TASK"；空串走项目级 fallback
        features: 顶层特征字典（见 paths.work_item_dir_name）

    Returns:
        {project_root}/.auto-engineering/{R6 顶层名}/session.json
        或当 top_node 为空时，{ade_sdd}/session.json（项目级）
    """
    if story_id and not top_node:
        top_node = "STORY"
        features = {"story_ids": [story_id]}
    # 🆕 v3.9.3 向后兼容：检测 features 是否实际是旧 raw story_id
    if features is not None and not isinstance(features, dict) and isinstance(features, str):
        # 旧调用：session_path(ade_sdd, story_id) — 第二个位置参数被当作 features
        # 不可能（type error），但兜底
        features = None
    if top_node and not features and isinstance(top_node, str) and not top_node.startswith(("PRD", "DR", "Story", "Task")):
        # 兼容旧调用：session_path(ade_sdd, story_id="STORY-006-BE")
        # 这种情况下 top_node 实际是 story_id
        story_id = top_node
        top_node = "STORY"
        features = {"story_ids": [story_id]}
    if top_node:
        if ade_sdd is None:
            return Path.cwd() / ".auto-engineering" / paths_mod.work_item_dir_name(top_node, features) / "session.json"
        return paths_mod.work_items_dir(ade_sdd) / paths_mod.work_item_dir_name(top_node, features) / "session.json"
    # 无 top_node：项目级 enter（向后兼容）
    if ade_sdd is None:
        return Path.cwd() / ".ae-sdd" / "session.json"
    return ade_sdd / "session.json"


def _story_ids_from_scope(top_node: str = "", features: Optional[dict] = None) -> list[str]:
    """Return story IDs implied by a session scope, including legacy raw Story calls."""
    story_ids: list[str] = []
    scope = (top_node or "").strip()
    data = features or {}
    if scope.upper() == "STORY" or scope.startswith("Story-"):
        raw_ids = data.get("story_ids") if isinstance(data, dict) else None
        if isinstance(raw_ids, list):
            story_ids.extend(str(sid).strip() for sid in raw_ids if str(sid).strip())
    if scope.upper().startswith("STORY-"):
        story_ids.append(scope.upper())
    normalized: list[str] = []
    for story_id in story_ids:
        normalized.append(story_id)
        if "--" in story_id:
            normalized.append(story_id.split("--", 1)[0])
    return list(dict.fromkeys(normalized))


def _expected_work_item_key(top_node: str = "", features: Optional[dict] = None) -> str:
    try:
        return paths_mod.work_item_dir_name(top_node, features) if top_node else ""
    except ValueError:
        return ""


def _session_matches_scope(session_data: dict, *, work_item_key: str, story_ids: list[str]) -> bool:
    """Best-effort match for v3.9 R6 sessions and v3.8 legacy Story sessions."""
    if work_item_key and session_data.get("workItemKey") == work_item_key:
        return True
    session_story = (session_data.get("storyId") or session_data.get("currentStory") or "").strip()
    if session_story and session_story in story_ids:
        return True
    features = session_data.get("features") or {}
    if isinstance(features, dict):
        recorded_story_ids = features.get("story_ids") or []
        if any(str(sid).strip() in story_ids for sid in recorded_story_ids):
            return True
    return False


def find_session_path(ade_sdd: Optional[Path], top_node: str = "",
                      features: Optional[dict] = None,
                      story_id: str = "") -> Optional[Path]:
    """Find an existing session.json across R6 and legacy work-item layouts.

    v3.9.3 writes `{project}/.auto-engineering/{R6}/session.json`, but real
    projects may already contain tokens in legacy Story directories such as
    `.auto-engineering/STORY-004-BE/session.json`. Gate checks must read those
    tokens instead of silently treating the flow as unentered.
    """
    if story_id and not top_node:
        top_node = "STORY"
        features = {"story_ids": [story_id]}
    primary = session_path(ade_sdd, top_node, features)
    if primary.is_file():
        return primary
    if ade_sdd is None:
        return None

    base = paths_mod.work_items_dir(ade_sdd)
    story_ids = _story_ids_from_scope(top_node, features)
    work_item_key = _expected_work_item_key(top_node, features)
    candidates: list[Path] = []

    if work_item_key:
        candidates.append(base / work_item_key / "session.json")
    for story_id in story_ids:
        candidates.append(base / story_id / "session.json")
        state_path = paths_mod.find_work_item_state_path(ade_sdd, story_id)
        if state_path is not None:
            candidates.append(state_path.parent / "session.json")

    for candidate in candidates:
        if candidate.is_file():
            return candidate

    if not base.is_dir() or not (story_ids or work_item_key):
        return None
    for candidate in sorted(base.glob("*/session.json")):
        try:
            data = json.loads(candidate.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        if _session_matches_scope(data, work_item_key=work_item_key, story_ids=story_ids):
            return candidate
    return None


def enter(project_key: str, top_node: str = "",
          features: Optional[dict] = None,
          ade_sdd: Optional[Path] = None,
          story_id: str = "") -> dict:
    """🆕 v3.9.3 领取 entry token，写入 {R6 顶层名}/session.json。

    Args:
        project_key: 项目 key
        top_node: 顶层节点类型（PRD/DR/STORY/TASK）
        features: 顶层特征字典
        ade_sdd: 项目 .ae-sdd 目录

    Returns:
        写入的 session dict。幂等：同 projectKey + workItemKey 一致则返回现有 token。
    """
    if story_id and not top_node:
        top_node = "STORY"
        features = {"story_ids": [story_id]}
    sp = session_path(ade_sdd, top_node, features)
    work_item_key = paths_mod.work_item_dir_name(top_node, features) if top_node else ""

    # 幂等：已存在且一致 → 返回现有
    if sp.is_file():
        try:
            existing = json.loads(sp.read_text(encoding="utf-8"))
            if (existing.get("projectKey") == project_key
                    and existing.get("workItemKey", "") == work_item_key):
                return existing
        except (json.JSONDecodeError, OSError):
            pass  # 损坏则覆写

    session = {
        "sessionId": str(uuid.uuid4()),
        "projectKey": project_key,
        "workItemKey": work_item_key,
        "topNode": top_node,
        "features": features or {},
        "entryPhase": "initialized",
        "enteredAt": _now_iso(),
        "userConfirmedPhases": [],
    }
    sp.parent.mkdir(parents=True, exist_ok=True)
    sp.write_text(json.dumps(session, ensure_ascii=False, indent=2), encoding="utf-8")
    return session


def read_session(ade_sdd: Optional[Path], top_node: str = "",
                 features: Optional[dict] = None,
                 story_id: str = "") -> Optional[dict]:
    """🆕 v3.9.3 读取 session.json；不存在返回 None。"""
    if story_id and not top_node:
        top_node = "STORY"
        features = {"story_ids": [story_id]}
    sp = find_session_path(ade_sdd, top_node, features) or session_path(ade_sdd, top_node, features)
    if not sp.is_file():
        return None
    try:
        return json.loads(sp.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def has_valid_entry_token(ade_sdd: Optional[Path], top_node: str = "",
                          features: Optional[dict] = None,
                          story_id: str = "") -> bool:
    """🆕 v3.9.3 校验当前是否有有效 entry token（session.json 存在 + sessionId 非空）。"""
    s = read_session(ade_sdd, top_node, features, story_id=story_id)
    return bool(s and s.get("sessionId"))


def confirm_phase(ade_sdd: Optional[Path], phase: str, confirmed_by: str = "user",
                  top_node: str = "",
                  features: Optional[dict] = None,
                  story_id: str = "") -> dict:
    """🆕 v3.9.3 记录审核点 token（阶段3 用）：在 userConfirmedPhases 追加一条确认记录。

    幂等：同一 phase 已确认则不重复追加。
    """
    if story_id and not top_node:
        top_node = "STORY"
        features = {"story_ids": [story_id]}
    sp = find_session_path(ade_sdd, top_node, features) or session_path(ade_sdd, top_node, features)
    work_item_key = paths_mod.work_item_dir_name(top_node, features) if top_node else ""
    s = read_session(ade_sdd, top_node, features) or {
        "sessionId": str(uuid.uuid4()),
        "projectKey": "",
        "workItemKey": work_item_key,
        "topNode": top_node,
        "features": features or {},
        "entryPhase": "initialized",
        "enteredAt": _now_iso(),
        "userConfirmedPhases": [],
    }
    confirms = s.setdefault("userConfirmedPhases", [])
    # 幂等：同 phase 已确认 → 跳过
    if any(c.get("phase") == phase for c in confirms):
        return s
    confirms.append({"phase": phase, "confirmedAt": _now_iso(), "confirmedBy": confirmed_by})
    sp.parent.mkdir(parents=True, exist_ok=True)
    sp.write_text(json.dumps(s, ensure_ascii=False, indent=2), encoding="utf-8")
    return s


def is_phase_confirmed(ade_sdd: Optional[Path], phase: str,
                       top_node: str = "",
                       features: Optional[dict] = None,
                       story_id: str = "") -> bool:
    """🆕 v3.9.3 校验某审核点是否已获用户确认 token。"""
    s = read_session(ade_sdd, top_node, features, story_id=story_id)
    if not s:
        return False
    return any(c.get("phase") == phase for c in s.get("userConfirmedPhases", []))
