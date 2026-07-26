"""Content-addressed evidence manifest and safe success-cache reuse."""
from __future__ import annotations

import hashlib
import ipaddress
import json
import shutil
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional
from urllib.parse import urlsplit


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


def _normalized_ac_ids(values) -> Optional[list[str]]:
    if not isinstance(values, list) or not values:
        return None
    normalized = []
    for value in values:
        ac_id = str(value or "").strip()
        if not ac_id:
            return None
        normalized.append(ac_id)
    return sorted(set(normalized))


def _is_loopback_host(hostname: str) -> bool:
    value = str(hostname or "").strip().rstrip(".").casefold()
    if value == "localhost" or value.endswith(".localhost"):
        return True
    try:
        return ipaddress.ip_address(value).is_loopback
    except ValueError:
        return False


def _valid_http_stage_url(stage: str, value: str) -> bool:
    try:
        parsed = urlsplit(str(value or "").strip())
        hostname = parsed.hostname or ""
    except ValueError:
        return False
    if parsed.scheme.casefold() not in {"http", "https"} or not hostname:
        return False
    if parsed.username is not None or parsed.password is not None:
        return False
    if parsed.query or parsed.fragment:
        return False
    try:
        address = ipaddress.ip_address(hostname.rstrip("."))
    except ValueError:
        address = None
    if address is not None and (
        address.is_unspecified or address.is_multicast or address.is_link_local
    ):
        return False
    loopback = _is_loopback_host(hostname)
    return loopback if stage == "local" else not loopback


def _evidence_started_at(entry: dict) -> Optional[datetime]:
    try:
        started_at = datetime.fromisoformat(
            str(entry.get("startedAt") or "").replace("Z", "+00:00")
        )
    except (TypeError, ValueError):
        return None
    if started_at.tzinfo is None or started_at.utcoffset() is None:
        return None
    return started_at


def validate_http_acceptance_manifest(
    project_dir: Path,
    story_id: str,
    required_acs: list[str],
    input_fingerprint: str,
    required_scenario_ids: Optional[list[str]] = None,
) -> tuple[bool, str, dict]:
    """Validate real-HTTP acceptance evidence for local then test-env stages.

    Only active ``http-local`` and ``http-test-env`` entries can satisfy the
    contract.  External sandbox/stub evidence is supplemental and is ignored
    when computing required stage completion.
    """
    required = _normalized_ac_ids(required_acs)
    if required is None:
        return False, "http-evidence-required-acs", {"requiredAcs": required_acs}
    if not str(input_fingerprint or "").strip():
        return False, "http-evidence-input-fingerprint", {"requiredAcs": required}
    required_scenarios = sorted(set(str(item).strip() for item in
                                    (required_scenario_ids or []) if str(item).strip()))

    path = manifest_path(project_dir, story_id)
    if not path.is_file():
        return False, "http-evidence-absent", {"requiredAcs": required}
    manifest = load_manifest(project_dir, story_id)
    if manifest.get("corrupt") or manifest.get("_integrityStatus") != "VERIFIED":
        return False, "http-evidence-manifest-integrity", {"requiredAcs": required}
    if str(manifest.get("storyId") or "") != story_id:
        return False, "http-evidence-story", {"requiredAcs": required}

    expected_kinds = {"local": "http-local", "test-env": "http-test-env"}
    stage_entries: dict[str, list[dict]] = {"local": [], "test-env": []}
    active = [
        entry for entry in manifest.get("entries", [])
        if entry.get("status", "active") == "active"
    ]
    for stage, kind in expected_kinds.items():
        candidates = [entry for entry in active if entry.get("kind") == kind]
        stage_entries[stage] = [
            entry for entry in candidates
            if entry.get("inputFingerprint") == input_fingerprint
        ]
        if candidates and not stage_entries[stage]:
            return False, "http-evidence-input-fingerprint", {
                "stage": stage,
                "requiredAcs": required,
            }

    missing_stages = [stage for stage in ("local", "test-env") if not stage_entries[stage]]
    if missing_stages:
        return False, "http-evidence-missing-stage", {
            "requiredAcs": required,
            "missingStages": missing_stages,
        }

    normalized: dict[str, list[dict]] = {"local": [], "test-env": []}
    invalid_candidates: dict[str, list[tuple[str, dict]]] = {"local": [], "test-env": []}
    for stage in ("local", "test-env"):
        for entry in stage_entries[stage]:
            if not entry.get("reusable") or int(entry.get("exitCode", 1)) != 0:
                invalid_candidates[stage].append(("http-evidence-unsuccessful", {
                    "stage": stage,
                    "evidenceId": entry.get("evidenceId"),
                }))
                continue
            summary = entry.get("summary")
            if not isinstance(summary, dict):
                invalid_candidates[stage].append(("http-evidence-summary", {"stage": stage}))
                continue
            if str(summary.get("stage") or "") != stage:
                invalid_candidates[stage].append(("http-evidence-stage", {"stage": stage}))
                continue
            if str(summary.get("result") or "") != "PASS":
                invalid_candidates[stage].append(("http-evidence-result", {"stage": stage}))
                continue
            if summary.get("internalMocks") is not False:
                invalid_candidates[stage].append(("http-evidence-internal-mock", {"stage": stage}))
                continue
            scenario_ids: set[str] = set()
            if required_scenarios:
                scenario_results = summary.get("scenarioResults")
                if not isinstance(scenario_results, list) or not scenario_results:
                    invalid_candidates[stage].append(("http-evidence-scenario-results", {"stage": stage}))
                    continue
                invalid_result = False
                for scenario_result in scenario_results:
                    if not isinstance(scenario_result, dict):
                        invalid_result = True
                        break
                    scenario_id = str(scenario_result.get("scenarioId") or "").strip()
                    assertion_kinds = {
                        str(item).strip().casefold()
                        for item in scenario_result.get("assertionKinds") or []
                        if str(item).strip()
                    }
                    meaningful = assertion_kinds & {
                        "field", "state", "relation", "invariant", "effect", "atomicity"
                    }
                    if (not scenario_id or scenario_result.get("result") != "PASS"
                            or not meaningful
                            or not str(scenario_result.get("rerunCommand") or "").strip()):
                        invalid_result = True
                        break
                    scenario_ids.add(scenario_id)
                if invalid_result:
                    invalid_candidates[stage].append(("http-evidence-scenario-depth", {"stage": stage}))
                    continue
            ac_ids = _normalized_ac_ids(summary.get("acIds"))
            if ac_ids is None:
                invalid_candidates[stage].append(("http-evidence-ac-ids", {"stage": stage}))
                continue
            build_id = str(summary.get("buildId") or "").strip()
            if not build_id:
                invalid_candidates[stage].append(("http-evidence-build-id", {"stage": stage}))
                continue
            base_url = str(summary.get("baseUrl") or "").strip()
            if not _valid_http_stage_url(stage, base_url):
                invalid_candidates[stage].append((
                    "http-evidence-url", {"stage": stage, "baseUrl": base_url}
                ))
                continue
            started_at = _evidence_started_at(entry)
            if started_at is None:
                invalid_candidates[stage].append(("http-evidence-time", {"stage": stage}))
                continue

            artifacts = entry.get("artifacts") or []
            if not artifacts:
                invalid_candidates[stage].append((
                    "http-evidence-artifact-integrity", {"stage": stage}
                ))
                continue
            if not all(_project_artifact_matches(project_dir, artifact)[0]
                       for artifact in artifacts):
                invalid_candidates[stage].append((
                    "http-evidence-artifact-integrity", {"stage": stage}
                ))
                continue

            normalized[stage].append({
                "buildId": build_id,
                "acIds": set(ac_ids),
                "startedAt": started_at,
                "evidenceId": entry.get("evidenceId"),
                "scenarioIds": scenario_ids,
            })

        if not normalized[stage]:
            reason, details = invalid_candidates[stage][0]
            return False, reason, details

    local_builds = {item["buildId"] for item in normalized["local"]}
    test_env_builds = {item["buildId"] for item in normalized["test-env"]}
    common_builds = sorted(local_builds & test_env_builds)
    if not common_builds:
        return False, "http-evidence-build-mismatch", {
            "localBuildIds": sorted(local_builds),
            "testEnvBuildIds": sorted(test_env_builds),
        }

    required_set = set(required)
    coverage_failures = []
    order_failures = []
    for build_id in common_builds:
        local = [item for item in normalized["local"] if item["buildId"] == build_id]
        test_env = [item for item in normalized["test-env"] if item["buildId"] == build_id]
        local_coverage = set().union(*(item["acIds"] for item in local))
        test_env_coverage = set().union(*(item["acIds"] for item in test_env))
        missing_local = sorted(required_set - local_coverage)
        missing_test_env = sorted(required_set - test_env_coverage)
        if missing_local or missing_test_env:
            coverage_failures.append({
                "buildId": build_id,
                "missingLocalAcs": missing_local,
                "missingTestEnvAcs": missing_test_env,
            })
            continue
        if required_scenarios:
            required_scenario_set = set(required_scenarios)
            local_scenarios = set().union(*(item["scenarioIds"] for item in local))
            test_env_scenarios = set().union(*(item["scenarioIds"] for item in test_env))
            missing_local_scenarios = sorted(required_scenario_set - local_scenarios)
            missing_test_env_scenarios = sorted(required_scenario_set - test_env_scenarios)
            if missing_local_scenarios or missing_test_env_scenarios:
                coverage_failures.append({
                    "buildId": build_id,
                    "missingLocalScenarios": missing_local_scenarios,
                    "missingTestEnvScenarios": missing_test_env_scenarios,
                })
                continue
        local_completed_at = max(item["startedAt"] for item in local)
        test_env_started_at = min(item["startedAt"] for item in test_env)
        if local_completed_at > test_env_started_at:
            order_failures.append({"buildId": build_id})
            continue
        return True, "verified", {
            "requiredAcs": required,
            "stages": ["local", "test-env"],
            "buildId": build_id,
            "internalMocks": False,
            "scenarioIds": required_scenarios,
        }

    if coverage_failures:
        return False, "http-evidence-missing-ac", {
            "requiredAcs": required,
            "candidates": coverage_failures,
        }
    return False, "http-evidence-order", {"candidates": order_failures}


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


# --- Append-only evidence ledger (migration oracle parity) -------------------
#
# The ledger is the evidence truth: one canonical JSON event per line forming a
# hash chain.  ``manifest.json`` is only the deterministic active projection of
# the ledger.  These helpers mirror the Rust contract
# ``EvidenceLedgerEventV1`` byte-for-byte: canonical JSON means UTF-8,
# sorted keys, compact separators and unescaped non-ASCII; digests are plain
# 64-character lowercase hex without a ``sha256:`` prefix.  Production Rust
# never calls this module; it exists so the migration oracle can verify the
# same ledger and projection rules.

LEDGER_EVENT_KINDS = ("recorded", "superseded", "finalized", "invalidated")


def ledger_path(project_dir: Path, story_id: str) -> Path:
    return project_dir / ".auto-engineering" / story_id / "evidence" / "ledger.jsonl"


def event_digest(event: dict) -> str:
    """Return the hash-chain digest: sha256 over the canonical event without ``eventDigest``."""
    preimage = {key: value for key, value in event.items() if key != "eventDigest"}
    return hashlib.sha256(_canonical(preimage)).hexdigest()


def make_event(sequence: int, event_id: str, kind: str, logical_key: str,
               input_fingerprint: str, artifact_refs: Optional[list] = None,
               previous_event_digest: Optional[str] = None) -> dict:
    """Build one canonical ledger event, computing its hash-chain digest."""
    if kind not in LEDGER_EVENT_KINDS:
        raise ValueError(f"evidence ledger event kind is unknown: {kind}")
    event = {
        "sequence": int(sequence),
        "eventId": str(event_id),
        "kind": kind,
        "logicalKey": str(logical_key),
        "inputFingerprint": str(input_fingerprint),
        "artifactRefs": [
            {
                "kind": str(ref["kind"]),
                "path": str(ref["path"]),
                "digest": str(ref["digest"]),
                "byteLength": int(ref["byteLength"]),
            }
            for ref in (artifact_refs or [])
        ],
        "previousEventDigest": previous_event_digest or None,
    }
    event["eventDigest"] = event_digest(event)
    return event


def canonical_event_line(event: dict) -> str:
    """Return the canonical JSONL line (without the trailing newline)."""
    return _canonical(event).decode("utf-8")


def verify_ledger(events) -> list:
    """Fail closed unless the events form one contiguous untampered hash chain."""
    verified = []
    previous = None
    for index, event in enumerate(events):
        sequence = index + 1
        if int(event.get("sequence") or 0) != sequence:
            raise ValueError(f"evidence ledger sequence gap at {sequence}")
        if event.get("kind") not in LEDGER_EVENT_KINDS:
            raise ValueError(f"evidence ledger event kind is unknown: {event.get('kind')}")
        declared_previous = event.get("previousEventDigest") or None
        if sequence == 1:
            if declared_previous is not None:
                raise ValueError(
                    "evidence ledger genesis event must not reference a previous digest")
        elif declared_previous != previous:
            raise ValueError(f"evidence ledger chain link is broken at sequence {sequence}")
        if event_digest(event) != event.get("eventDigest"):
            raise ValueError(f"evidence ledger event digest mismatch at sequence {sequence}")
        verified.append(event)
        previous = event.get("eventDigest")
    return verified


def parse_ledger(text: str) -> list:
    """Parse and verify canonical JSONL ledger text, failing closed on tampering."""
    events = []
    for line in text.splitlines():
        if not line.strip():
            continue
        event = json.loads(line)
        if line != canonical_event_line(event):
            raise ValueError("evidence ledger event is not canonical JSON")
        events.append(event)
    if text and not text.endswith("\n"):
        raise ValueError("evidence ledger is truncated")
    return verify_ledger(events)


def project_entries(events, entry_payloads: dict, residue=()) -> list:
    """Fold ledger events into the deterministic active manifest projection.

    ``entry_payloads`` maps a recorded ``eventId`` to the full entry payload the
    event binds (stored as a content-addressed artifact in production).
    ``residue`` holds non-ledger entries (legacy or toolset receipts) that stay
    verbatim at the head of the projection and are never rewritten in place.
    """
    entries = [dict(entry) for entry in residue]

    def _active_index(logical_key: str) -> Optional[int]:
        for position in range(len(entries) - 1, -1, -1):
            entry = entries[position]
            if (entry.get("status") or "active") == "active" \
                    and entry.get("logicalKey") == logical_key:
                return position
        return None

    for index, event in enumerate(events):
        kind = event["kind"]
        if kind == "recorded":
            payload = entry_payloads.get(event["eventId"])
            if payload is None:
                raise ValueError(
                    f"evidence ledger recorded event has no entry payload: {event['eventId']}")
            entry = dict(payload)
            entry["status"] = "active"
            entries.append(entry)
        elif kind in {"superseded", "invalidated"}:
            position = _active_index(str(event.get("logicalKey") or ""))
            if position is None:
                raise ValueError(
                    f"evidence ledger {kind} event has no active entry: {event.get('logicalKey')}")
            entries[position]["status"] = "superseded" if kind == "superseded" else "invalidated"
            if kind == "superseded":
                successor = next(
                    (later["eventId"] for later in events[index + 1:]
                     if later["kind"] == "recorded"
                     and later.get("logicalKey") == event.get("logicalKey")),
                    None,
                )
                if successor is not None:
                    entries[position]["supersededBy"] = successor
        # ``finalized`` binds the projection digest and never alters entries.
    return entries


def rebuild_manifest(story_id: str, entries) -> dict:
    """Rebuild the sealed manifest projection from ledger-derived entries."""
    manifest = {"schemaVersion": 1, "storyId": story_id,
                "entries": [dict(entry) for entry in entries]}
    manifest["contentHash"] = manifest_content_hash(manifest)
    return manifest
