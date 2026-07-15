"""Content-addressed evidence manifest and safe success-cache reuse."""
from __future__ import annotations

import hashlib
import json
import shutil
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
        if entry.get("status") == "superseded":
            continue
        for artifact in entry.get("artifacts") or []:
            raw_path = Path(str(artifact.get("snapshotPath") or artifact.get("path") or ""))
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


def _project_artifact_matches(project_dir: Path, artifact: dict) -> tuple[bool, Optional[Path]]:
    """Validate a content-addressed artifact rooted inside ``project_dir``.

    Evidence manifests are portable project assets.  Absolute paths (even when
    they currently point inside the project) and parent traversal would make
    the same manifest mean something different on another machine, so both are
    rejected instead of normalized permissively.
    """
    raw = str(artifact.get("snapshotPath") or artifact.get("path") or "").strip().replace("\\", "/")
    relative = Path(raw)
    if not raw or relative.is_absolute() or ".." in relative.parts:
        return False, None
    root = project_dir.resolve()
    try:
        resolved = (root / relative).resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError):
        return False, None
    expected = str(artifact.get("sha256") or "")
    try:
        if not resolved.is_file() or not expected or artifact_hash(resolved) != expected:
            return False, None
    except OSError:
        return False, None
    return True, resolved


def _normalized_scope(values) -> Optional[list[str]]:
    if not isinstance(values, list) or not values:
        return None
    normalized = []
    for value in values:
        raw = str(value or "").strip().replace("\\", "/")
        path = Path(raw)
        if not raw or path.is_absolute() or ".." in path.parts:
            return None
        normalized.append(path.as_posix())
    return sorted(set(normalized))


def validate_g09_manifest(project_dir: Path, story_id: str,
                          input_fingerprint: str,
                          expected_scope: Optional[list[str]] = None) -> tuple[bool, str]:
    """Validate present, current, semantically bound G-09 provenance."""
    path = manifest_path(project_dir, story_id)
    if not path.is_file():
        return False, "absent"
    manifest = load_manifest(project_dir, story_id)
    if manifest.get("corrupt") or manifest.get("_integrityStatus") != "VERIFIED":
        return False, "manifest-integrity"
    if str(manifest.get("storyId") or "") != story_id:
        return False, "story-mismatch"
    relevant = [
        entry for entry in manifest.get("entries", [])
        if entry.get("status") != "superseded"
        if entry.get("kind") == "test-authenticity"
        or (entry.get("kind") == "test" and entry.get("summary", {}).get("gate") == "G-09")
    ]
    if not relevant:
        return False, "no-current-g09-entry"
    entry = relevant[-1]
    if entry.get("inputFingerprint") != input_fingerprint:
        return False, "input-fingerprint"
    if not entry.get("reusable") or int(entry.get("exitCode", 1)) != 0:
        return False, "unsuccessful-entry"
    summary = entry.get("summary")
    if not isinstance(summary, dict):
        return False, "summary"
    scope = _normalized_scope(expected_scope or summary.get("changedPaths"))
    if scope is None:
        return False, "summary-scope"
    if _normalized_scope(summary.get("changedPaths")) != scope:
        return False, "summary-changed-paths"
    if _normalized_scope(summary.get("scope")) != scope:
        return False, "summary-scope"
    if str(summary.get("gate") or "") != "G-09":
        return False, "summary-gate"
    if str(summary.get("storyId") or "") != story_id:
        return False, "summary-story"
    if str(summary.get("status") or "") != "PASS":
        return False, "summary-status"
    command = str(entry.get("commandHash") or "")
    toolchain = str(entry.get("toolchainFingerprint") or "")
    if not command or str(summary.get("commandHash") or "") != command:
        return False, "summary-command"
    if not toolchain or str(summary.get("toolchainFingerprint") or "") != toolchain:
        return False, "summary-toolchain"

    artifacts = entry.get("artifacts") or []
    checked_artifacts = []
    for artifact in artifacts:
        ok, resolved = _project_artifact_matches(project_dir, artifact)
        if not ok or resolved is None:
            return False, "artifact-integrity"
        checked_artifacts.append((artifact, resolved))
    if not checked_artifacts:
        return False, "artifact-integrity"
    report_ref = str(summary.get("report") or "").strip().replace("\\", "/")
    if not report_ref or report_ref not in {
        str(artifact.get("path") or "").strip().replace("\\", "/")
        for artifact, _ in checked_artifacts
    }:
        return False, "summary-report"
    report_path = next(
        resolved for artifact, resolved in checked_artifacts
        if str(artifact.get("path") or "").strip().replace("\\", "/") == report_ref
    )
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False, "report-integrity"
    if str(report.get("storyId") or "") != story_id:
        return False, "report-story"
    if str(report.get("status") or "") != "PASS":
        return False, "report-status"
    if _normalized_scope(report.get("scope")) != scope:
        return False, "report-scope"
    if str(report.get("commandHash") or "") != command:
        return False, "report-command"
    if str(report.get("toolchainFingerprint") or "") != toolchain:
        return False, "report-toolchain"
    return True, "verified"


def record(project_dir: Path, story_id: str, *, kind: str, command: str | list[str],
           input_fingerprint: str, toolchain_fingerprint: str, exit_code: int,
           artifacts: list[dict], summary: Optional[dict] = None, duration_ms: int = 0,
           freshness_window_seconds: Optional[int] = None,
           logical_key: str = "") -> dict:
    manifest = load_manifest(project_dir, story_id)
    root = project_dir.resolve()
    normalized_artifacts = []
    for raw_artifact in artifacts:
        artifact = dict(raw_artifact or {})
        raw_path = str(artifact.get("path") or "").strip()
        source = Path(raw_path)
        if not raw_path:
            raise ValueError("evidence artifact path is empty")
        source = source if source.is_absolute() else root / source
        try:
            source = source.resolve(strict=True)
            source.relative_to(root)
        except (OSError, ValueError) as exc:
            raise ValueError(f"evidence artifact must be a file inside project: {raw_path}") from exc
        if not source.is_file():
            raise ValueError(f"evidence artifact is not a file: {raw_path}")
        digest = artifact_hash(source)
        expected = str(artifact.get("sha256") or "")
        if expected and expected != digest:
            raise ValueError(f"evidence artifact hash mismatch: {raw_path}")
        relative = source.relative_to(root).as_posix()
        snapshot_relative = (Path(".auto-engineering") / story_id / "evidence" / "artifacts" /
                             f"{digest[7:]}-{source.name}").as_posix()
        snapshot = root / snapshot_relative
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        if not snapshot.is_file() or artifact_hash(snapshot) != digest:
            shutil.copyfile(source, snapshot)
        artifact["path"] = relative
        artifact["sha256"] = digest
        artifact["snapshotPath"] = snapshot_relative
        normalized_artifacts.append(artifact)
    command_digest = command_hash(command)
    logical_key = str(logical_key or "").strip() or fingerprint({
        "kind": kind,
        "commandHash": command_digest,
        "artifacts": [str(item.get("path") or "") for item in normalized_artifacts],
    })
    entry = {
        "evidenceId": "ev-" + fingerprint({"kind": kind, "command": command, "input": input_fingerprint})[7:23],
        "kind": kind,
        "commandHash": command_digest,
        "inputFingerprint": input_fingerprint,
        "toolchainFingerprint": toolchain_fingerprint,
        "startedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "durationMs": int(duration_ms),
        "exitCode": int(exit_code),
        "summary": summary or {},
        "artifacts": normalized_artifacts,
        "reusable": int(exit_code) == 0,
        "logicalKey": logical_key,
        "status": "active",
    }
    if freshness_window_seconds is not None:
        entry["freshnessWindowSeconds"] = int(freshness_window_seconds)
    manifest.setdefault("schemaVersion", 1)
    manifest["storyId"] = story_id
    entries = manifest.setdefault("entries", [])
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    for previous in entries:
        if previous.get("status", "active") == "active" and previous.get("logicalKey") == logical_key:
            previous["status"] = "superseded"
            previous["supersededAt"] = now
            previous["supersededBy"] = entry["evidenceId"]
    entries.append(entry)
    save_manifest(project_dir, story_id, manifest)
    return entry


def _artifact_matches(artifact: dict, project_dir: Optional[Path] = None) -> bool:
    path = Path(str(artifact.get("snapshotPath") or artifact.get("path") or ""))
    if project_dir is not None and not path.is_absolute():
        path = project_dir / path
    expected = str(artifact.get("sha256") or "")
    try:
        return bool(path.is_file() and expected and artifact_hash(path) == expected)
    except OSError:
        return False


def is_reusable(entry: dict, *, input_fingerprint: str, command: str | list[str],
                toolchain_fingerprint: str, project_dir: Optional[Path] = None) -> bool:
    if entry.get("status") == "superseded":
        return False
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
    return all(_artifact_matches(a, project_dir) for a in entry.get("artifacts", []))


def find_reusable(project_dir: Path, story_id: str, *, input_fingerprint: str,
                  command: str | list[str], toolchain_fingerprint: str) -> Optional[dict]:
    manifest = load_manifest(project_dir, story_id)
    if manifest.get("corrupt") or manifest.get("_integrityStatus") not in {None, "VERIFIED"}:
        return None
    for entry in reversed(manifest.get("entries", [])):
        if is_reusable(entry, input_fingerprint=input_fingerprint, command=command,
                       toolchain_fingerprint=toolchain_fingerprint, project_dir=project_dir):
            return entry
    return None
