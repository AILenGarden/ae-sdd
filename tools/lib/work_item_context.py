"""Work-item state resolution helpers.

This module resolves task-scoped `.auto-engineering/<work-item>/state.json`
files. Project-global `.ae-sdd/state.json` is intentionally not a fallback
because it leaks state across concurrent sessions.
"""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from lib import paths, state as state_mod


@dataclass(frozen=True)
class WorkItemState:
    key: str
    path: Path
    data: dict


@dataclass(frozen=True)
class ResolvedWorkItemState:
    path: Path
    key: str
    data: dict
    source: str


class AmbiguousWorkItemError(RuntimeError):
    """Raised when an implicit state read would consume a stale global mirror."""

    def __init__(self, candidates: list[WorkItemState]):
        self.candidates = candidates
        super().__init__(format_ambiguity_message(candidates))


class NoWorkItemStateError(RuntimeError):
    """Raised when no task-scoped state exists and no explicit work item was given."""

    def __init__(self):
        super().__init__(
            "No ae-sdd work-item state exists. Create or select a task-scoped state first: "
            "ae-sdd state new --id <ID> --entry-node <PRD|DR|STORY> --story-ids <ID>"
        )


_SESSION_FIELD_CANDIDATES = (
    "session_id",
    "sessionId",
    "conversation_id",
    "conversationId",
    "thread_id",
    "threadId",
    "transcript_path",
    "transcriptPath",
)


def state_path_work_item_key(sp: Path, fallback: str = "") -> str:
    try:
        if sp.name == "state.json" and sp.parent.parent.name == ".auto-engineering":
            return sp.parent.name
    except Exception:
        pass
    return fallback


def list_work_item_states(ade_sdd: Path) -> list[WorkItemState]:
    base = paths.work_items_dir(ade_sdd)
    if not base.is_dir():
        return []

    items: list[WorkItemState] = []
    for child in sorted(base.iterdir()):
        if not child.is_dir():
            continue
        sp = child / "state.json"
        if not sp.is_file():
            continue
        try:
            data = state_mod.read_state(sp)
        except Exception:
            data = {}
        items.append(WorkItemState(key=child.name, path=sp, data=data if isinstance(data, dict) else {}))
    return items


def format_ambiguity_message(candidates: list[WorkItemState]) -> str:
    keys = [c.key for c in candidates]
    lines = [
        "Multiple ae-sdd work-item states exist; refusing to infer an active task implicitly.",
        "Specify the target work item explicitly, for example:",
        "  ae-sdd state read --work-item <work-item>",
        "  ae-sdd state write --work-item <work-item> --phase <phase>",
        "Candidates:",
    ]
    for key in keys:
        lines.append(f"  - {key}")
    return "\n".join(lines)


def session_key_from_payload(payload: Optional[dict] = None, explicit: str = "") -> str:
    if explicit:
        return str(explicit).strip()
    payload = payload or {}
    for field in _SESSION_FIELD_CANDIDATES:
        value = payload.get(field)
        if value:
            return str(value).strip()
    return ""


def _session_context_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "session-context"


def _safe_session_file_name(session_key: str) -> str:
    digest = hashlib.sha256(session_key.encode("utf-8", errors="ignore")).hexdigest()[:24]
    prefix = re.sub(r"[^A-Za-z0-9_.-]+", "-", session_key).strip(".-")[:40] or "session"
    return f"{prefix}-{digest}.json"


def _session_context_path(ade_sdd: Path, session_key: str) -> Path:
    return _session_context_dir(ade_sdd) / _safe_session_file_name(session_key)


def _is_under_auto_engineering(ade_sdd: Path, sp: Path) -> bool:
    try:
        resolved = sp.resolve()
        resolved.relative_to(paths.work_items_dir(ade_sdd).resolve())
        return resolved.is_file()
    except Exception:
        return False


def read_session_binding(ade_sdd: Path, session_key: str) -> Optional[ResolvedWorkItemState]:
    if not session_key:
        return None
    p = _session_context_path(ade_sdd, session_key)
    if not p.is_file():
        return None
    try:
        binding = json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return None
    raw_path = str(binding.get("activeStatePath") or "").strip()
    if not raw_path:
        return None
    sp = Path(raw_path)
    if not sp.is_absolute():
        sp = paths.project_root(ade_sdd) / sp
    if not _is_under_auto_engineering(ade_sdd, sp):
        return None
    try:
        data = state_mod.read_state(sp)
    except Exception:
        return None
    key = state_path_work_item_key(sp, str(binding.get("activeWorkItem") or ""))
    return ResolvedWorkItemState(path=sp, key=key, data=data, source="session")


def bind_session_state(
    ade_sdd: Path,
    session_key: str,
    state_path: Path,
    work_item_key: str,
    active_story: str = "",
) -> None:
    if not session_key or not _is_under_auto_engineering(ade_sdd, state_path):
        return
    p = _session_context_path(ade_sdd, session_key)
    payload = {
        "activeWorkItem": work_item_key,
        "activeStatePath": str(state_path),
        "activeStory": active_story,
        "updatedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    try:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    except OSError:
        pass


# ─── 🆕 会话 engage 标记（门禁按需启用）───────────────────────────────────────
# 设计：用户调 /ae-sdd 触发词后，prompt_inject 写入本会话的 engage 标记文件；
# gate_intercept 在 resolve_default_state 前检查该标记——有则启用门禁，无则放行。
# 这样"没调 ae-sdd 的会话/子 Agent 不被锁"，实现"按需启用 hook"语义。
# 标记按 session_key 维度（sha256 hash 文件名），子 Agent 继承父会话 session_id
# 时自动随主会话 engage。

def _engaged_dir(ade_sdd: Path) -> Path:
    """engage 标记目录（.ae-sdd/.session-engaged/）。"""
    return ade_sdd / ".session-engaged"


def is_session_engaged(ade_sdd: Path, session_key: str) -> bool:
    """本会话是否已 engage ae-sdd（用户调过 /ae-sdd 触发词）。

    无 session_key → False（无标识的会话视为未 engage，放行）。
    """
    if not session_key:
        return False
    return _engaged_dir(ade_sdd).joinpath(
        _safe_session_file_name(session_key)
    ).is_file()


def mark_session_engaged(ade_sdd: Path, session_key: str) -> None:
    """记录本会话已 engage。持续态，写入后由 disengage_session 清除。"""
    if not session_key:
        return
    p = _engaged_dir(ade_sdd) / _safe_session_file_name(session_key)
    try:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(
            json.dumps(
                {"engagedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")},
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )
    except OSError:
        pass


def disengage_session(ade_sdd: Path, session_key: str) -> None:
    """退出 engage，清除本会话标记。"""
    if not session_key:
        return
    p = _engaged_dir(ade_sdd) / _safe_session_file_name(session_key)
    try:
        p.unlink(missing_ok=True)
    except OSError:
        pass


def artifact_fingerprint(path: str | Path) -> str:
    """Content fingerprint used by precise artifact invalidation."""
    value = Path(path)
    try:
        digest = hashlib.sha256(value.read_bytes()).hexdigest()
    except OSError:
        digest = "missing"
    return "sha256:" + digest


def make_provenance(artifact: str, path: str | Path, input_artifacts: list[dict] | None = None) -> dict:
    """Create a compact provenance record without relying on Git or mtime."""
    return {
        "artifact": artifact,
        "path": str(path),
        "inputArtifacts": input_artifacts or [],
        "outputFingerprint": artifact_fingerprint(path),
        "generatedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }


def invalidation_plan(changed_artifacts: list[dict], known_artifacts: list[dict]) -> dict:
    """Compute affected outputs from input fingerprint edges."""
    changed = {str(item.get("fingerprint") or item.get("outputFingerprint")) for item in changed_artifacts}
    invalidated, retained = [], []
    for artifact in known_artifacts:
        inputs = artifact.get("inputArtifacts") or []
        affected = any(str(item.get("fingerprint")) in changed for item in inputs)
        (invalidated if affected else retained).append(artifact)
    return {"invalidated": invalidated, "retained": retained,
            "changed": list(changed), "reason": "input fingerprint dependency"}


_STORY_RE = re.compile(
    r"\bSTORY-[A-Za-z0-9][A-Za-z0-9_-]*(?:-[A-Za-z0-9][A-Za-z0-9_-]*)*\b",
    re.IGNORECASE,
)
_WORK_ITEM_RE = re.compile(r"\b(?:PRD|DR|Story|Task)-[A-Za-z0-9][A-Za-z0-9_.-]*\b", re.IGNORECASE)


def _tokens_from_prompt(prompt_text: str) -> list[str]:
    if not prompt_text:
        return []
    tokens: list[str] = []
    tokens.extend(m.group(0) for m in _STORY_RE.finditer(prompt_text))
    tokens.extend(m.group(0) for m in _WORK_ITEM_RE.finditer(prompt_text))
    return list(dict.fromkeys(tokens))


def _state_identifier_values(item: WorkItemState) -> list[str]:
    data = item.data if isinstance(item.data, dict) else {}
    values: list[str] = [item.key]
    for field in (
        "workItemId",
        "workItemKey",
        "stateMachineId",
        "stateMachineName",  # 🆕 v3.10.1 纯业务名（无 UUID 前缀），供按业务名匹配
        "currentWorkItem",
        "activeWorkItem",
        "currentStory",
        "activeStory",
        "storyId",
    ):
        value = data.get(field)
        if value:
            values.append(str(value))
    values.extend(str(v) for v in (data.get("storyIds") or []) if v)
    values.extend(str(v) for v in (data.get("storyStates") or {}).keys() if v)
    for dr_id, dr_state in (data.get("drStates") or {}).items():
        if dr_id:
            values.append(str(dr_id))
        if isinstance(dr_state, dict):
            dr_value = dr_state.get("drId")
            if dr_value:
                values.append(str(dr_value))
            values.extend(str(v) for v in (dr_state.get("storyStates") or {}).keys() if v)
    return list(dict.fromkeys(v.strip() for v in values if v and str(v).strip()))


def _resolve_prompt_identifier_state(ade_sdd: Path, prompt_text: str) -> Optional[ResolvedWorkItemState]:
    """Resolve full work-item/story identifiers mentioned in a prompt.

    Regex extraction alone can reduce project-specific IDs such as
    ``cs-ai-STORY-005-BE`` to ``STORY-005-BE``. Matching against known state
    identifiers first preserves the real work-item key and keeps the binding
    session-scoped.
    """
    if not prompt_text:
        return None
    prompt_lc = prompt_text.casefold()
    hits: list[tuple[int, int, WorkItemState]] = []
    for item in list_work_item_states(ade_sdd):
        positions: list[tuple[int, int]] = []
        for ident in _state_identifier_values(item):
            ident_lc = ident.casefold()
            pos = prompt_lc.find(ident_lc)
            if pos >= 0:
                positions.append((pos, len(ident_lc)))
        if positions:
            pos, length = min(positions, key=lambda p: (p[0], -p[1]))
            hits.append((pos, -length, item))
    if not hits:
        return None
    _, _, item = sorted(hits, key=lambda h: (h[0], h[1], h[2].key))[0]
    return ResolvedWorkItemState(path=item.path, key=item.key, data=item.data, source="prompt-identifier")


def resolve_mentioned_state(ade_sdd: Path, prompt_text: str) -> Optional[ResolvedWorkItemState]:
    direct = _resolve_prompt_identifier_state(ade_sdd, prompt_text)
    if direct is not None:
        return direct

    for token in _tokens_from_prompt(prompt_text):
        hit = None
        if token.upper().startswith("STORY-"):
            found = paths.find_nested_state_by_story_id(ade_sdd, token.upper())
            if found:
                sp, data = found
                hit = ResolvedWorkItemState(
                    path=sp,
                    key=state_path_work_item_key(sp, sp.parent.name),
                    data=data,
                    source="prompt-story",
                )
        if hit is None:
            sp = paths.find_work_item_state_path(ade_sdd, token)
            if sp is not None:
                hit = ResolvedWorkItemState(
                    path=sp,
                    key=state_path_work_item_key(sp, token),
                    data=state_mod.read_state(sp),
                    source="prompt-work-item",
                )
        if hit is not None:
            return hit
    return None


def resolve_default_state(
    ade_sdd: Path,
    *,
    session_key: str = "",
    prompt_text: str = "",
    bind_session: bool = False,
) -> ResolvedWorkItemState:
    mentioned = resolve_mentioned_state(ade_sdd, prompt_text)
    if mentioned is not None:
        if bind_session:
            active_story = state_mod.get_active_story(mentioned.data) or ""
            bind_session_state(ade_sdd, session_key, mentioned.path, mentioned.key, active_story)
        return mentioned

    bound = read_session_binding(ade_sdd, session_key)
    if bound is not None:
        return bound

    candidates = list_work_item_states(ade_sdd)
    # Completed work items must not occupy the implicit-default slot or the
    # ambiguity pool: once a work item finishes, it should neither be picked
    # silently nor keep blocking unrelated work as phantom "multiple states"
    # noise (life project 2026-07-08/09 incident).
    active_candidates = [c for c in candidates if not state_mod.is_work_item_completed(c.data)]
    if len(active_candidates) == 1:
        only = active_candidates[0]
        return ResolvedWorkItemState(path=only.path, key=only.key, data=only.data, source="single-work-item")
    if len(active_candidates) > 1:
        raise AmbiguousWorkItemError(active_candidates)

    raise NoWorkItemStateError()
