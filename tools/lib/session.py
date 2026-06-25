"""
session.py — ae-sdd 入口凭证（entry token）管理（🆕 v3.4.0，建议书4 关卡1）

关卡1 入口凭证机制：
  agent 收到 /ae-sdd 触发后，第一动作必须是 `ae-sdd enter <projectKey> [--story <STORY-ID>]`，
  领取 entry token 写入 .auto-engineering/<STORY>/session.json。
  关卡2（产物落地校验）/关卡3（代码改动准入）校验该 token 存在性。

session.json schema：
  {
    "sessionId": "<uuid>",
    "projectKey": "icec-cloud-boss",
    "storyId": "STORY-020-BE",      # 可空（项目级 enter）
    "entryPhase": "initialized",
    "enteredAt": "2026-06-25T...",
    "userConfirmedPhases": [         # 审核点 token（阶段3 用）
      {"phase": "task-reviewed", "confirmedAt": "...", "confirmedBy": "user"}
    ]
  }

跨 hook 持久化：session.json 写入 .auto-engineering/{storyId}/session.json（若 storyId 为空
则写 .ae-sdd/session.json），随对话持久，多轮间不丢失。
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


def session_path(ade_sdd: Optional[Path], story_id: str = "") -> Path:
    """定位 session.json 路径。

    优先：.auto-engineering/{storyId}/session.json（Story 级 enter）
    回退：.ae-sdd/session.json（项目级 enter，无 storyId）
    """
    if story_id:
        # 项目根下的 .auto-engineering/{storyId}/session.json
        project_root = paths_mod.project_root(ade_sdd) if ade_sdd else Path.cwd()
        return project_root / ".auto-engineering" / story_id / "session.json"
    # 无 storyId：写 .ae-sdd/session.json
    if ade_sdd is None:
        return Path.cwd() / ".ae-sdd" / "session.json"
    return ade_sdd / "session.json"


def enter(project_key: str, story_id: str = "", ade_sdd: Optional[Path] = None) -> dict:
    """领取 entry token，写入 session.json。

    返回写入的 session dict。若 session.json 已存在且 projectKey/storyId 一致，
    视为重入，返回现有 token（幂等）。
    """
    sp = session_path(ade_sdd, story_id)

    # 幂等：已存在且一致 → 返回现有
    if sp.is_file():
        try:
            existing = json.loads(sp.read_text(encoding="utf-8"))
            if (existing.get("projectKey") == project_key
                    and existing.get("storyId", "") == story_id):
                return existing
        except (json.JSONDecodeError, OSError):
            pass  # 损坏则覆写

    session = {
        "sessionId": str(uuid.uuid4()),
        "projectKey": project_key,
        "storyId": story_id,
        "entryPhase": "initialized",
        "enteredAt": _now_iso(),
        "userConfirmedPhases": [],
    }
    sp.parent.mkdir(parents=True, exist_ok=True)
    sp.write_text(json.dumps(session, ensure_ascii=False, indent=2), encoding="utf-8")
    return session


def read_session(ade_sdd: Optional[Path], story_id: str = "") -> Optional[dict]:
    """读取 session.json；不存在返回 None。"""
    sp = session_path(ade_sdd, story_id)
    if not sp.is_file():
        return None
    try:
        return json.loads(sp.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def has_valid_entry_token(ade_sdd: Optional[Path], story_id: str = "") -> bool:
    """校验当前是否有有效 entry token（session.json 存在 + sessionId 非空）。"""
    s = read_session(ade_sdd, story_id)
    return bool(s and s.get("sessionId"))


def confirm_phase(ade_sdd: Optional[Path], phase: str, confirmed_by: str = "user",
                  story_id: str = "") -> dict:
    """记录审核点 token（阶段3 用）：在 userConfirmedPhases 追加一条确认记录。

    幂等：同一 phase 已确认则不重复追加。
    """
    sp = session_path(ade_sdd, story_id)
    s = read_session(ade_sdd, story_id) or {
        "sessionId": str(uuid.uuid4()),
        "projectKey": "",
        "storyId": story_id,
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


def is_phase_confirmed(ade_sdd: Optional[Path], phase: str, story_id: str = "") -> bool:
    """校验某审核点是否已获用户确认 token。"""
    s = read_session(ade_sdd, story_id)
    if not s:
        return False
    return any(c.get("phase") == phase for c in s.get("userConfirmedPhases", []))
