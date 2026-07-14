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


def manifest_content_hash(manifest: dict) -> str:
    payload = {key: value for key, value in manifest.items()
               if key != "contentHash" and not str(key).startswith("_")}
    return fingerprint(payload)


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
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {"schemaVersion": 1, "storyId": story_id, "entries": [], "corrupt": True}
    expected = str(manifest.get("contentHash") or "")
    if not expected:
        manifest["_integrityStatus"] = "UNVERIFIED"
    elif expected != manifest_content_hash(manifest):
        manifest["_integrityStatus"] = "TAMPERED"
    else:
        manifest["_integrityStatus"] = "VERIFIED"
    return manifest


def save_manifest(project_dir: Path, story_id: str, manifest: dict) -> Path:
    path = manifest_path(project_dir, story_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {key: value for key, value in manifest.items()
               if not str(key).startswith("_") and key != "corrupt"}
    payload["contentHash"] = manifest_content_hash(payload)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path


def finalize_manifest(project_dir: Path, story_id: str) -> tuple[Path, dict]:
    """Upgrade an unhashed manifest and verify every artifact before sealing it."""
    path = manifest_path(project_dir, story_id)
    if not path.is_file():
        raise ValueError(f"evidence manifest does not exist: {path}")
    manifest = load_manifest(project_dir, story_id)
    if manifest.get("corrupt"):
        raise ValueError("evidence manifest is not valid JSON")
    if manifest.get("_integrityStatus") == "TAMPERED":
        raise ValueError("evidence manifest contentHash mismatch")
    if str(manifest.get("storyId") or "") != story_id:
        raise ValueError("evidence manifest storyId mismatch")
    root = project_dir.resolve()
    for entry in manifest.get("entries", []):
        for artifact in entry.get("artifacts") or []:
            raw_path = Path(str(artifact.get("path") or ""))
            artifact_path = raw_path if raw_path.is_absolute() else root / raw_path
            try:
                artifact_path = artifact_path.resolve(strict=True)
            except OSError as exc:
                raise ValueError(f"evidence artifact does not exist: {raw_path}") from exc
            if not artifact_path.is_file():
                raise ValueError(f"evidence artifact is not a file: {raw_path}")
            actual = artifact_hash(artifact_path)
            expected = str(artifact.get("sha256") or "")
            if expected and expected != actual:
                raise ValueError(f"evidence artifact hash mismatch: {raw_path}")
            artifact["sha256"] = actual
    path = save_manifest(project_dir, story_id, manifest)
    return path, load_manifest(project_dir, story_id)


def validate_g09_manifest(project_dir: Path, story_id: str,
                          input_fingerprint: str) -> tuple[bool, str]:
    """Validate present, current G-09 provenance without turning it into a waiver."""
    path = manifest_path(project_dir, story_id)
    if not path.is_file():
        return True, "absent"
    manifest = load_manifest(project_dir, story_id)
    if manifest.get("corrupt") or manifest.get("_integrityStatus") != "VERIFIED":
        return False, "manifest-integrity"
    if str(manifest.get("storyId") or "") != story_id:
        return False, "story-mismatch"
    relevant = [
        entry for entry in manifest.get("entries", [])
        if entry.get("kind") == "test-authenticity"
        or (entry.get("kind") == "test" and entry.get("summary", {}).get("gate") == "G-09")
    ]
    if not relevant:
        return True, "no-current-g09-entry"
    entry = relevant[-1]
    if entry.get("inputFingerprint") != input_fingerprint:
        return False, "input-fingerprint"
    if not entry.get("reusable") or int(entry.get("exitCode", 1)) != 0:
        return False, "unsuccessful-entry"
    artifacts = entry.get("artifacts") or []
    if not artifacts or not all(_artifact_matches(artifact) for artifact in artifacts):
        return False, "artifact-integrity"
    return True, "verified"


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
    if manifest.get("corrupt") or manifest.get("_integrityStatus") not in {None, "VERIFIED"}:
        return None
    for entry in reversed(manifest.get("entries", [])):
        if is_reusable(entry, input_fingerprint=input_fingerprint, command=command,
                       toolchain_fingerprint=toolchain_fingerprint):
            return entry
    return None
