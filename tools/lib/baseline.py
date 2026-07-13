"""Explicit, auditable baseline/delta handling for coding authenticity findings."""
from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional


def canonical_json(value: dict) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def finding_key(finding: dict) -> str:
    explicit = str(finding.get("findingKey") or "").strip()
    if explicit:
        return explicit
    rule = finding.get("ruleId") or finding.get("rule") or "UNKNOWN"
    path = str(finding.get("path") or "").replace("\\", "/")
    symbol = finding.get("symbol") or finding.get("line") or ""
    category = finding.get("category") or finding.get("severity") or "UNKNOWN"
    raw = f"{rule}\n{path}\n{symbol}\n{category}".encode("utf-8")
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def normalize_finding(finding: dict) -> dict:
    item = dict(finding)
    item["findingKey"] = finding_key(item)
    item["ruleId"] = item.get("ruleId") or item.get("rule") or "UNKNOWN"
    item["path"] = str(item.get("path") or "").replace("\\", "/")
    item["symbol"] = item.get("symbol") or item.get("line")
    item["severity"] = item.get("severity") or item.get("category") or "UNKNOWN"
    return item


def baseline_path(project_dir: Path, gate_id: str = "G-CODE-1") -> Path:
    return project_dir / ".ae-sdd" / "baselines" / f"{gate_id}.json"


def _payload_without_hash(payload: dict) -> dict:
    value = dict(payload)
    value.pop("contentHash", None)
    return value


def content_hash(payload: dict) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(_payload_without_hash(payload))).hexdigest()


def load(project_dir: Path, gate_id: str = "G-CODE-1") -> tuple[Optional[dict], Optional[str]]:
    path = baseline_path(project_dir, gate_id)
    if not path.is_file():
        return None, "missing"
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return None, f"invalid-json: {exc}"
    expected = payload.get("contentHash")
    if not expected or expected != content_hash(payload):
        return payload, "tampered"
    return payload, None


def create(project_dir: Path, gate_id: str, findings: Iterable[dict], *,
           created_by: str, scanner_version: str, ruleset_fingerprint: str,
           project_fingerprint: str, require_user_approval: bool = False) -> dict:
    if not require_user_approval:
        raise PermissionError("baseline creation requires explicit user approval")
    normalized = [normalize_finding(f) for f in findings]
    payload = {
        "schemaVersion": 1,
        "gateId": gate_id,
        "createdAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "createdBy": created_by,
        "approval": "user-approved",
        "scannerVersion": scanner_version,
        "rulesetFingerprint": ruleset_fingerprint,
        "projectFingerprint": project_fingerprint,
        "findings": [{
            "findingKey": f["findingKey"],
            "ruleId": f.get("ruleId"),
            "path": f.get("path"),
            "symbol": f.get("symbol"),
            "severity": f.get("severity"),
            "evidenceHash": f.get("evidenceHash") or _evidence_hash(f),
        } for f in normalized],
    }
    payload["contentHash"] = content_hash(payload)
    path = baseline_path(project_dir, gate_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return payload


def _evidence_hash(finding: dict) -> str:
    raw = f"{finding.get('path','')}:{finding.get('line','')}:{finding.get('snippet','')}".encode("utf-8")
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def compare(baseline_payload: Optional[dict], current_findings: Iterable[dict], *,
            ruleset_fingerprint: str = "", touched_paths: Optional[Iterable[str]] = None) -> dict:
    if baseline_payload is None:
        return {"status": "BLOCK_BASELINE_REQUIRED", "baseline": 0, "current": 0, "new": [], "resolved": [], "touchedDebt": []}
    if ruleset_fingerprint and baseline_payload.get("rulesetFingerprint") != ruleset_fingerprint:
        return {"status": "BLOCK_BASELINE_INVALID", "reason": "ruleset fingerprint mismatch", "baseline": len(baseline_payload.get("findings", [])), "current": 0, "new": [], "resolved": [], "touchedDebt": []}
    touched = {str(p).replace("\\", "/") for p in (touched_paths or [])}
    baseline_findings = [normalize_finding(f) for f in baseline_payload.get("findings", [])]
    current = [normalize_finding(f) for f in current_findings]
    baseline_by_key = {f["findingKey"]: f for f in baseline_findings}
    current_by_key = {f["findingKey"]: f for f in current}
    new = [f for key, f in current_by_key.items() if key not in baseline_by_key and str(f.get("severity", "")).upper() == "BLOCKER"]
    resolved = [f for key, f in baseline_by_key.items() if key not in current_by_key]
    touched_debt = [f for f in baseline_findings if f.get("path") in touched]
    if new:
        status = "BLOCK_NEW_FINDINGS"
    elif touched_debt:
        status = "BLOCK_TOUCHED_DEBT"
    else:
        status = "PASS_WITH_BASELINE_DEBT"
    return {
        "status": status,
        "baseline": len(baseline_findings),
        "current": len(current),
        "new": new,
        "resolved": resolved,
        "touchedDebt": touched_debt,
    }
