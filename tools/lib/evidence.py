"""Content-addressed evidence manifest and safe success-cache reuse."""
from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional


def _canonical(value) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def fingerprint(value) -> str:
    return "sha256:" + hashlib.sha256(_canonical(value)).hexdigest()


def manifest_path(project_dir: Path, story_id: str) -> Path:
    return project_dir / ".auto-engineering" / story_id / "evidence" / "manifest.json"


def artifact_hash(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def command_hash(command: str | list[str]) -> str:
    return fingerprint(command)


def load_manifest(project_dir: Path, story_id: str) -> dict:
    path = manifest_path(project_dir, story_id)
    if not path.is_file():
        return {"schemaVersion": 1, "storyId": story_id, "entries": []}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {"schemaVersion": 1, "storyId": story_id, "entries": [], "corrupt": True}


def save_manifest(project_dir: Path, story_id: str, manifest: dict) -> Path:
    path = manifest_path(project_dir, story_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path


def record(project_dir: Path, story_id: str, *, kind: str, command: str | list[str],
           input_fingerprint: str, toolchain_fingerprint: str, exit_code: int,
           artifacts: list[dict], summary: Optional[dict] = None, duration_ms: int = 0,
           freshness_window_seconds: Optional[int] = None) -> dict:
    manifest = load_manifest(project_dir, story_id)
    entry = {
        "evidenceId": "ev-" + fingerprint({"kind": kind, "command": command, "input": input_fingerprint})[7:23],
        "kind": kind,
        "commandHash": command_hash(command),
        "inputFingerprint": input_fingerprint,
        "toolchainFingerprint": toolchain_fingerprint,
        "startedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "durationMs": int(duration_ms),
        "exitCode": int(exit_code),
        "summary": summary or {},
        "artifacts": artifacts,
        "reusable": int(exit_code) == 0,
    }
    if freshness_window_seconds is not None:
        entry["freshnessWindowSeconds"] = int(freshness_window_seconds)
    manifest.setdefault("schemaVersion", 1)
    manifest["storyId"] = story_id
    manifest.setdefault("entries", []).append(entry)
    save_manifest(project_dir, story_id, manifest)
    return entry


def _artifact_matches(artifact: dict) -> bool:
    path = Path(str(artifact.get("path") or ""))
    expected = str(artifact.get("sha256") or "")
    try:
        return bool(path.is_file() and expected and artifact_hash(path) == expected)
    except OSError:
        return False


def is_reusable(entry: dict, *, input_fingerprint: str, command: str | list[str],
                toolchain_fingerprint: str) -> bool:
    if not entry.get("reusable") or int(entry.get("exitCode", 1)) != 0:
        return False
    if entry.get("inputFingerprint") != input_fingerprint:
        return False
    if entry.get("commandHash") != command_hash(command):
        return False
    if entry.get("toolchainFingerprint") != toolchain_fingerprint:
        return False
    freshness = entry.get("freshnessWindowSeconds")
    if freshness is not None:
        try:
            started = datetime.fromisoformat(str(entry.get("startedAt")).replace("Z", "+00:00"))
            age = (datetime.now(timezone.utc) - started).total_seconds()
            if age < 0 or age > int(freshness):
                return False
        except (TypeError, ValueError, OverflowError):
            return False
    return all(_artifact_matches(a) for a in entry.get("artifacts", []))


def find_reusable(project_dir: Path, story_id: str, *, input_fingerprint: str,
                  command: str | list[str], toolchain_fingerprint: str) -> Optional[dict]:
    manifest = load_manifest(project_dir, story_id)
    if manifest.get("corrupt"):
        return None
    for entry in reversed(manifest.get("entries", [])):
        if is_reusable(entry, input_fingerprint=input_fingerprint, command=command,
                       toolchain_fingerprint=toolchain_fingerprint):
            return entry
    return None
