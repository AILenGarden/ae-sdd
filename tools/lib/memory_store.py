"""
Phase-aware memory store for ae-sdd.

This module is intentionally small and dependency-free. It stores auditable
JSONL records under the current project's .ae-sdd/memory directory and supports
mandatory phase hooks:

  memory enter -> phase work -> memory write -> memory exit

`exit` fails when no write happened after the latest `enter` for the same
phase/story/task scope.
"""
from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional

from lib import paths


PHASE_ALIASES = {
    "requirement-analysis": "ra",
    "requirements": "ra",
    "ra": "ra",
    "design": "design",
    "story": "design",
    "task": "coding-plan",
    "task-generate": "coding-plan",
    "coding-plan": "coding-plan",
    "plan": "coding-plan",
    "coding": "coding",
    "execute": "coding",
    "review": "review",
    "code-review": "review",
    "postmortem": "review",
}

VALID_PHASES = {"ra", "design", "coding-plan", "coding", "review"}
VALID_LAYERS = {"L0", "L1", "L2", "L3", "L4"}


@dataclass
class MemoryScope:
    project_root: Path
    memory_root: Path
    phase: str
    story: Optional[str] = None
    task: Optional[str] = None

    @property
    def scope_key(self) -> str:
        parts = [self.phase]
        if self.story:
            parts.append(self.story)
        if self.task:
            parts.append(self.task)
        return "__".join(_safe_part(p) for p in parts)


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def normalize_phase(phase: str) -> str:
    p = (phase or "").strip().lower()
    p = PHASE_ALIASES.get(p, p)
    if p not in VALID_PHASES:
        raise ValueError(f"unknown memory phase: {phase} (allowed: {sorted(VALID_PHASES)})")
    return p


def normalize_layer(layer: str) -> str:
    l = (layer or "L1").strip().upper()
    if l not in VALID_LAYERS:
        raise ValueError(f"unknown memory layer: {layer} (allowed: {sorted(VALID_LAYERS)})")
    return l


def locate_scope(
    *,
    project: Optional[str] = None,
    phase: str,
    story: Optional[str] = None,
    task: Optional[str] = None,
) -> MemoryScope:
    project_dir = Path(project).resolve() if project else Path.cwd()
    ade_sdd = paths.locate_project_ae_sdd(project_dir)
    if ade_sdd is None:
        # For early project setup and tests, create a local .ae-sdd directory.
        ade_sdd = project_dir / ".ae-sdd"
    project_root = ade_sdd.parent
    memory_root = ade_sdd / "memory"
    return MemoryScope(
        project_root=project_root,
        memory_root=memory_root,
        phase=normalize_phase(phase),
        story=story,
        task=task,
    )


def _safe_part(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in ("-", "_", ".") else "_" for ch in value)


def _jsonl_path(scope: MemoryScope, layer: str) -> Path:
    layer = normalize_layer(layer)
    if layer == "L0":
        return scope.memory_root / "session" / f"{scope.scope_key}.jsonl"
    if layer == "L1":
        if scope.story:
            base = scope.memory_root / "story" / _safe_part(scope.story)
            if scope.task:
                base = base / "task" / _safe_part(scope.task)
            return base / f"{scope.phase}.jsonl"
        return scope.memory_root / "phase" / f"{scope.phase}.jsonl"
    if layer == "L2":
        return scope.memory_root / "project" / f"{scope.phase}.jsonl"
    if layer == "L3":
        return scope.memory_root / "global-patterns" / f"{scope.phase}.jsonl"
    return scope.memory_root / "archive" / f"{scope.phase}.jsonl"


def _stage_path(scope: MemoryScope) -> Path:
    return scope.memory_root / ".stage" / f"{scope.scope_key}.json"


def _append_jsonl(path: Path, record: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")


def _read_json(path: Path) -> dict:
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}


def _write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _iter_jsonl(path: Path) -> Iterable[dict]:
    if not path.is_file():
        return
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            yield {"type": "corrupt", "raw": line, "path": str(path)}


def enter(scope: MemoryScope, *, actor: str = "ae-sdd", note: str = "") -> dict:
    """Record that a phase has loaded memory before work starts."""
    now = utc_now()
    existing = read(scope, include_project=True, limit=50)
    stage = _read_json(_stage_path(scope))
    stage.update({
        "phase": scope.phase,
        "story": scope.story,
        "task": scope.task,
        "last_enter_at": now,
        "last_enter_by": actor,
        "last_enter_note": note,
    })
    _write_json(_stage_path(scope), stage)

    event = {
        "type": "enter",
        "phase": scope.phase,
        "story": scope.story,
        "task": scope.task,
        "timestamp": now,
        "actor": actor,
        "note": note,
        "loaded_entries": len(existing),
    }
    _append_jsonl(_jsonl_path(scope, "L0"), event)
    return {"entered": True, "scope": scope.scope_key, "loaded_entries": len(existing), "entries": existing}


def write(
    scope: MemoryScope,
    *,
    summary: str,
    layer: str = "L1",
    kind: str = "observation",
    evidence: Optional[list[str]] = None,
    actor: str = "ae-sdd",
    tags: Optional[list[str]] = None,
) -> dict:
    if not summary or not summary.strip():
        raise ValueError("memory summary is required")
    layer = normalize_layer(layer)
    now = utc_now()
    record = {
        "type": "memory",
        "phase": scope.phase,
        "story": scope.story,
        "task": scope.task,
        "layer": layer,
        "kind": kind,
        "summary": summary.strip(),
        "evidence": evidence or [],
        "tags": tags or [],
        "timestamp": now,
        "actor": actor,
    }
    target = _jsonl_path(scope, layer)
    _append_jsonl(target, record)

    stage = _read_json(_stage_path(scope))
    stage.update({
        "phase": scope.phase,
        "story": scope.story,
        "task": scope.task,
        "last_write_at": now,
        "last_write_by": actor,
        "last_write_path": str(target),
    })
    _write_json(_stage_path(scope), stage)
    return {"written": True, "path": str(target), "record": record}


def check_exit_ready(scope: MemoryScope, *, allow_empty: bool = False) -> dict:
    """Check whether a phase scope can be left without writing an exit event."""
    stage = _read_json(_stage_path(scope))
    enter_at = stage.get("last_enter_at")
    write_at = stage.get("last_write_at")

    if allow_empty:
        return {
            "pass": True,
            "blocked": False,
            "reason": "",
            "scope": scope.scope_key,
            "stage": stage,
            "allow_empty": True,
        }

    if not enter_at:
        return {
            "pass": False,
            "blocked": True,
            "reason": "memory enter is required before leaving this node",
            "scope": scope.scope_key,
            "stage": stage,
        }
    if not write_at:
        return {
            "pass": False,
            "blocked": True,
            "reason": "memory write is required after the latest memory enter",
            "scope": scope.scope_key,
            "stage": stage,
        }
    if write_at < enter_at:
        return {
            "pass": False,
            "blocked": True,
            "reason": "memory write must happen after the latest memory enter",
            "scope": scope.scope_key,
            "stage": stage,
        }

    return {
        "pass": True,
        "blocked": False,
        "reason": "",
        "scope": scope.scope_key,
        "stage": stage,
    }


def exit_phase(scope: MemoryScope, *, actor: str = "ae-sdd", allow_empty: bool = False) -> dict:
    check = check_exit_ready(scope, allow_empty=allow_empty)
    stage = check["stage"]
    enter_at = stage.get("last_enter_at")
    write_at = stage.get("last_write_at")
    ok = bool(check["pass"])
    now = utc_now()
    event = {
        "type": "exit",
        "phase": scope.phase,
        "story": scope.story,
        "task": scope.task,
        "timestamp": now,
        "actor": actor,
        "pass": ok,
        "last_enter_at": enter_at,
        "last_write_at": write_at,
    }
    _append_jsonl(_jsonl_path(scope, "L0"), event)
    if not ok:
        return {
            "pass": False,
            "blocked": True,
            "reason": check["reason"],
            "stage": stage,
        }
    return {"pass": True, "blocked": False, "stage": stage}


def read(scope: MemoryScope, *, include_project: bool = True, limit: int = 20) -> list[dict]:
    paths_to_read = []
    if include_project:
        paths_to_read.append(_jsonl_path(scope, "L2"))
    paths_to_read.append(_jsonl_path(scope, "L1"))
    paths_to_read.append(_jsonl_path(scope, "L0"))
    entries: list[dict] = []
    for p in paths_to_read:
        entries.extend(_iter_jsonl(p) or [])
    entries.sort(key=lambda item: item.get("timestamp", ""))
    return entries[-limit:] if limit > 0 else entries


def search(scope: MemoryScope, *, query: str, limit: int = 20) -> list[dict]:
    q = (query or "").lower()
    if not q:
        return []
    entries: list[dict] = []
    if scope.memory_root.is_dir():
        for p in scope.memory_root.rglob("*.jsonl"):
            for item in _iter_jsonl(p) or []:
                haystack = json.dumps(item, ensure_ascii=False).lower()
                if q in haystack:
                    item = dict(item)
                    item["_path"] = str(p)
                    entries.append(item)
    entries.sort(key=lambda item: item.get("timestamp", ""))
    return entries[-limit:] if limit > 0 else entries


def summarize(scope: MemoryScope) -> dict:
    counts: dict[str, int] = {}
    phases: dict[str, int] = {}
    total = 0
    if scope.memory_root.is_dir():
        for p in scope.memory_root.rglob("*.jsonl"):
            for item in _iter_jsonl(p) or []:
                total += 1
                layer = item.get("layer", "event")
                phase = item.get("phase", "unknown")
                counts[layer] = counts.get(layer, 0) + 1
                phases[phase] = phases.get(phase, 0) + 1
    return {"total": total, "by_layer": counts, "by_phase": phases, "root": str(scope.memory_root)}


def promote(scope: MemoryScope, *, from_layer: str = "L1", to_layer: str = "L2", actor: str = "ae-sdd") -> dict:
    from_layer = normalize_layer(from_layer)
    to_layer = normalize_layer(to_layer)
    source_path = _jsonl_path(scope, from_layer)
    target_path = _jsonl_path(scope, to_layer)
    promoted = 0
    now = utc_now()
    for item in _iter_jsonl(source_path) or []:
        if item.get("type") != "memory":
            continue
        promoted_item = dict(item)
        promoted_item["layer"] = to_layer
        promoted_item["promoted_from"] = str(source_path)
        promoted_item["promoted_at"] = now
        promoted_item["promoted_by"] = actor
        _append_jsonl(target_path, promoted_item)
        promoted += 1
    return {"promoted": promoted, "from": str(source_path), "to": str(target_path)}
