"""Review Batch v2 state and transition helpers.

The module keeps the review engine deterministic and independent from the CLI.
Legacy ``reviewLoop`` fields are maintained by the caller as a compatibility
projection; the batch/session fields are the authoritative state for v2.
"""
from __future__ import annotations

import hashlib
import json
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Optional


SCHEMA_VERSION = 2
RULESET_VERSION = "review-batch-v2"
DEFAULT_BUDGETS = {
    1: {"maxAttempts": 3, "maxValidBatches": 2, "maxRemediations": 1, "maxWallClockMinutes": 30},
    2: {"maxAttempts": 4, "maxValidBatches": 3, "maxRemediations": 2, "maxWallClockMinutes": 60},
    3: {"maxAttempts": 5, "maxValidBatches": 3, "maxRemediations": 2, "maxWallClockMinutes": 120},
}

FINDING_STATUSES = {
    "OPEN", "FIXED_PENDING_VERIFY", "CLOSED", "ACCEPTED_RISK",
    "DEFERRED_DEPENDENCY", "BASELINE_DEBT", "DUPLICATE",
}
HIGH_RISK_SEVERITIES = {"P0", "P1", "BLOCKER", "CRITICAL"}


def _now() -> datetime:
    return datetime.now(timezone.utc)


def _iso(value: Optional[datetime] = None) -> str:
    return (value or _now()).strftime("%Y-%m-%dT%H:%M:%SZ")


def _parse_iso(value: Any) -> Optional[datetime]:
    if not value:
        return None
    try:
        return datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    except (TypeError, ValueError):
        return None


def canonical_fingerprint(value: Any) -> str:
    """Hash canonical JSON so equivalent manifests produce the same key."""
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def fingerprint_manifest(manifest: dict) -> str:
    return canonical_fingerprint(manifest)


def fingerprint_files(paths: list[str | Path], root: Optional[Path] = None, extra: Optional[dict] = None) -> str:
    """Build a stable content manifest for git and non-git projects."""
    entries = []
    for raw_path in sorted({str(p) for p in paths}):
        path = Path(raw_path)
        if not path.is_absolute() and root is not None:
            path = root / path
        try:
            content = path.read_bytes()
            digest = hashlib.sha256(content).hexdigest()
            display = str(path.relative_to(root)) if root is not None else str(path)
            entries.append({"path": display.replace("\\", "/"), "sha256": digest})
        except OSError:
            entries.append({"path": str(path).replace("\\", "/"), "missing": True})
    manifest = {"files": entries}
    if extra:
        manifest.update(extra)
    return fingerprint_manifest(manifest)


def fingerprint_layers(*, implementation: Any = None, documentation: Any = None,
                       review: Any = None, toolchain: Any = None) -> dict:
    """Keep implementation/documentation/review invalidation domains separate."""
    return {
        "implementationFingerprint": canonical_fingerprint({"implementation": implementation, "toolchain": toolchain}),
        "documentationFingerprint": canonical_fingerprint({"documentation": documentation}),
        "reviewFingerprint": canonical_fingerprint({"review": review}),
    }


def _budgets(tier: int, override: Optional[dict] = None) -> dict:
    result = dict(DEFAULT_BUDGETS.get(int(tier), DEFAULT_BUDGETS[1]))
    for key in result:
        if override and override.get(key) is not None:
            value = int(override[key])
            if value <= 0:
                raise ValueError(f"{key} must be positive")
            result[key] = value
    return result


def create_session(node: str, tier: int, tier_basis: Optional[dict] = None,
                   input_fingerprint: str = "", ruleset_fingerprint: str = "",
                   budgets: Optional[dict] = None, started_at: Optional[datetime] = None) -> dict:
    started = started_at or _now()
    effective_budgets = _budgets(tier, budgets)
    deadline = started + timedelta(minutes=effective_budgets["maxWallClockMinutes"])
    return {
        "schemaVersion": SCHEMA_VERSION,
        "engine": "batch-v2",
        "node": node,
        "tier": tier,
        "tierBasis": tier_basis or {},
        "policy": "risk-based",
        "inputFingerprint": input_fingerprint or "",
        "rulesetFingerprint": ruleset_fingerprint or canonical_fingerprint({"version": RULESET_VERSION}),
        "startedAt": _iso(started),
        "deadlineAt": _iso(deadline),
        "budgets": effective_budgets,
        "counters": {
            "attempts": 0,
            "validBatches": 0,
            "cleanStreak": 0,
            "remediations": 0,
            "infraFailures": 0,
            "protocolFailures": 0,
        },
        "findings": [],
        "batches": [],
        "exit": None,
        "round": 0,
        "dryCounter": 0,
        "exitReason": None,
        "exitedAt": None,
        "reviewers": [],
        "sessionId": f"review-{canonical_fingerprint({'node': node, 'startedAt': _iso(started)})[7:23]}",
    }


def _normalize_finding(finding: dict, batch_id: str = "") -> dict:
    item = dict(finding or {})
    path = str(item.get("path") or item.get("anchor") or "").strip()
    symbol = str(item.get("symbol") or "").strip()
    rule_id = str(item.get("ruleId") or item.get("rule") or item.get("id") or "UNKNOWN").strip()
    category = str(item.get("category") or "UNKNOWN").strip().upper()
    severity = str(item.get("severity") or category or "UNKNOWN").strip().upper()
    key = str(item.get("findingKey") or "").strip()
    if not key:
        key = canonical_fingerprint({"ruleId": rule_id, "path": path, "symbol": symbol, "category": category})
    status = str(item.get("status") or "OPEN").strip().upper()
    if status not in FINDING_STATUSES:
        status = "OPEN"
    item.update({
        "findingKey": key,
        "ruleId": rule_id,
        "path": path,
        "symbol": symbol,
        "category": category,
        "severity": severity,
        "status": status,
        "firstSeenBatch": item.get("firstSeenBatch") or batch_id,
        "lastSeenBatch": batch_id or item.get("lastSeenBatch"),
    })
    return item


def _finding_requires_disposition(finding: dict) -> bool:
    status = str(finding.get("status") or "OPEN").upper()
    if status == "ACCEPTED_RISK":
        return not bool(finding.get("authorizedBy") or finding.get("authorization"))
    if status == "DEFERRED_DEPENDENCY":
        return not bool(finding.get("dependency") or finding.get("dependencyStory"))
    return False


def _open_high_risk(findings: list[dict]) -> list[dict]:
    return [f for f in findings if str(f.get("status", "OPEN")).upper() in {"OPEN", "FIXED_PENDING_VERIFY"}
            and str(f.get("severity") or f.get("category") or "").upper() in HIGH_RISK_SEVERITIES]


def restart_for_fingerprint(session: dict, input_fingerprint: str,
                            *, ruleset_fingerprint: str = "") -> dict:
    """Start a new input generation while preserving remediation/finding history."""
    if not input_fingerprint:
        raise ValueError("input_fingerprint is required for a new generation")
    previous = session
    next_session = create_session(
        node=str(previous.get("node") or ""),
        tier=int(previous.get("tier") or 1),
        tier_basis=previous.get("tierBasis") or {},
        input_fingerprint=input_fingerprint,
        ruleset_fingerprint=ruleset_fingerprint or str(previous.get("rulesetFingerprint") or ""),
        budgets=previous.get("budgets") or {},
    )
    next_session["sessionId"] = f"{previous.get('sessionId', 'review')}-next"
    next_session["parentSessionId"] = previous.get("sessionId")
    old_counters = previous.get("counters") or {}
    next_session["counters"]["remediations"] = int(old_counters.get("remediations", 0))
    next_session["findings"] = []
    for finding in previous.get("findings", []):
        copied = dict(finding)
        if str(copied.get("status") or "OPEN").upper() == "OPEN":
            copied["status"] = "FIXED_PENDING_VERIFY"
        next_session["findings"].append(copied)
    next_session["findingHistory"] = previous.get("findings", [])
    next_session["legacyImported"] = previous.get("legacyImported", False)
    return next_session


def upgrade_legacy(legacy: dict, node: str = "", tier: Optional[int] = None) -> dict:
    """Import v1 reviewLoop state without treating old rounds as valid batches."""
    if int(legacy.get("schemaVersion", 0) or 0) >= SCHEMA_VERSION:
        return legacy
    effective_tier = int(tier or legacy.get("tier") or 1)
    session = create_session(node or legacy.get("node", ""), effective_tier, legacy.get("tierBasis") or {})
    session["legacyImported"] = True
    session["legacyRound"] = int(legacy.get("round", 0) or 0)
    session["legacyExitReason"] = legacy.get("exitReason")
    session["findings"] = [_normalize_finding(f, batch_id="legacy") for f in legacy.get("findings", [])]
    if legacy.get("round"):
        session["batches"] = [{
            "batchId": "legacy",
            "attempt": int(legacy.get("round", 0) or 0),
            "status": "INVALID_PROTOCOL",
            "legacy": True,
            "inputFingerprint": "",
            "findings": session["findings"],
            "reviewers": legacy.get("reviewers", []),
        }]
    return session


def _required_roles(tier: int) -> list[str]:
    return ["GENERAL"] if tier <= 1 else (["BE", "AR"] if tier == 2 else ["BE", "AR", "QA"])


def _role_status(report: dict) -> str:
    status = str(report.get("status") or "PASS").upper()
    if status in {"429", "RATE_LIMIT", "ERROR_RATE_LIMIT", "TIMEOUT", "CRASH", "INFRA_ERROR"}:
        return "INVALID_INFRA"
    if status in {"INVALID", "MALFORMED", "PROTOCOL_ERROR", "INVALID_PROTOCOL"}:
        return "INVALID_PROTOCOL"
    if status in {"CANCELLED", "INTERRUPTED"}:
        return "CANCELLED"
    return "PASS"


def _policy_clean_target(session: dict) -> int:
    tier = int(session.get("tier") or 1)
    remediations = int((session.get("counters") or {}).get("remediations", 0))
    return 2 if tier >= 3 and remediations > 0 else 1


def _merge_reviewer_reports(existing: list[dict], incoming: list[dict], required_roles: list[str]) -> list[dict]:
    """Merge a retry into an existing batch, retaining successful roles."""
    by_role = {str(r.get("role") or "").upper(): dict(r) for r in existing if r.get("role")}
    for report in incoming:
        role = str(report.get("role") or "").upper()
        if role:
            by_role[role] = dict(report)
    return [by_role[role] for role in required_roles if role in by_role]


def _set_projection(session: dict) -> None:
    counters = session.setdefault("counters", {})
    session["round"] = int(counters.get("attempts", 0))
    session["dryCounter"] = int(counters.get("cleanStreak", 0))
    exit_obj = session.get("exit") or {}
    session["exitReason"] = exit_obj.get("reason")
    session["exitedAt"] = exit_obj.get("at")


def _budget_reason(session: dict, now: Optional[datetime] = None) -> Optional[str]:
    counters = session.setdefault("counters", {})
    budgets = session.setdefault("budgets", _budgets(int(session.get("tier") or 1)))
    if counters.get("attempts", 0) >= budgets["maxAttempts"]:
        return "max-attempts"
    if counters.get("remediations", 0) > budgets["maxRemediations"]:
        return "max-remediations"
    deadline = _parse_iso(session.get("deadlineAt"))
    if deadline and (now or _now()) >= deadline:
        return "max-wall-clock"
    return None


def collect_batch(session: dict, reviewer_reports: list[dict], root_session_id: str,
                  *, input_fingerprint: str = "", ruleset_fingerprint: str = "",
                  batch_id: str = "", has_red_blocker: bool = False,
                  now: Optional[datetime] = None) -> dict:
    """Aggregate one reviewer attempt and return the next mechanical action."""
    now = now or _now()
    session = upgrade_legacy(session)
    counters = session.setdefault("counters", {})
    tier = int(session.get("tier") or 1)
    expected_roles = _required_roles(tier)
    batch_id = batch_id or f"b{int(counters.get('attempts', 0)) + 1}"
    current_fp = input_fingerprint or session.get("inputFingerprint") or ""
    baseline_fp = session.get("inputFingerprint") or ""
    if not baseline_fp and current_fp:
        session["inputFingerprint"] = current_fp
        baseline_fp = current_fp
    ruleset_fp = ruleset_fingerprint or session.get("rulesetFingerprint") or ""

    counters["attempts"] = int(counters.get("attempts", 0)) + 1
    previous_batch = next((b for b in session.get("batches", []) if b.get("batchId") == batch_id), None)
    if previous_batch:
        reviewer_reports = _merge_reviewer_reports(previous_batch.get("reviewers", []), reviewer_reports, expected_roles)
    statuses = [_role_status(r) for r in reviewer_reports]
    role_names = [str(r.get("role") or "").upper() for r in reviewer_reports]
    missing_roles = [role for role in expected_roles if role not in role_names]
    duplicate_roles = sorted({role for role in role_names if role and role_names.count(role) > 1})
    duplicate_sessions = sorted({sid for sid in [str(r.get("sessionId") or "") for r in reviewer_reports]
                                 if sid and [str(r.get("sessionId") or "") for r in reviewer_reports].count(sid) > 1})
    violations = []
    if len(reviewer_reports) < len(expected_roles):
        violations.append(f"reviewer count {len(reviewer_reports)} < required {len(expected_roles)}")
    if missing_roles:
        violations.append("missing roles: " + ",".join(missing_roles))
    if duplicate_roles:
        violations.append("duplicate roles: " + ",".join(duplicate_roles))
    if duplicate_sessions:
        violations.append("duplicate reviewer sessions: " + ",".join(duplicate_sessions))
    for report in reviewer_reports:
        sid = str(report.get("sessionId") or "")
        if root_session_id and sid == root_session_id:
            violations.append(f"role {report.get('role') or '?'} uses root session")
    if current_fp and baseline_fp and current_fp != baseline_fp:
        batch_status = "INVALID_INPUT_DRIFT"
        violations.append("input fingerprint drift")
    elif ruleset_fp and session.get("rulesetFingerprint") and ruleset_fp != session["rulesetFingerprint"]:
        batch_status = "INVALID_INPUT_DRIFT"
        violations.append("ruleset fingerprint drift")
    elif any(s == "INVALID_INFRA" for s in statuses):
        batch_status = "INVALID_INFRA"
    elif any(s == "INVALID_PROTOCOL" for s in statuses) or violations:
        batch_status = "INVALID_PROTOCOL"
    elif any(s == "CANCELLED" for s in statuses):
        batch_status = "CANCELLED"
    else:
        batch_status = "VALID_FINDINGS" if any(r.get("findings") for r in reviewer_reports) else "VALID_CLEAN"

    current_findings = []
    for report in reviewer_reports:
        current_findings.extend(report.get("findings") or [])
    normalized = [_normalize_finding(f, batch_id=batch_id) for f in current_findings]
    if batch_status == "VALID_FINDINGS":
        known = {f.get("findingKey"): f for f in session.get("findings", [])}
        for finding in normalized:
            previous = known.get(finding["findingKey"])
            if previous:
                previous.update({"lastSeenBatch": batch_id, "status": finding.get("status", previous.get("status", "OPEN"))})
            else:
                known[finding["findingKey"]] = finding
        session["findings"] = list(known.values())
        counters["remediations"] = int(counters.get("remediations", 0)) + 1
        counters["cleanStreak"] = 0
    newly_valid = batch_status in {"VALID_FINDINGS", "VALID_CLEAN"} and not (
        previous_batch and previous_batch.get("status") in {"VALID_FINDINGS", "VALID_CLEAN"}
    )
    if newly_valid:
        counters["validBatches"] = int(counters.get("validBatches", 0)) + 1
    if batch_status == "VALID_CLEAN":
        for finding in session.get("findings", []):
            if str(finding.get("status") or "OPEN").upper() == "OPEN":
                finding["status"] = "FIXED_PENDING_VERIFY"
        counters["cleanStreak"] = int(counters.get("cleanStreak", 0)) + 1
        if counters["cleanStreak"] >= _policy_clean_target(session):
            for finding in session.get("findings", []):
                if str(finding.get("status") or "OPEN").upper() == "FIXED_PENDING_VERIFY":
                    finding["status"] = "CLOSED"
    elif batch_status == "INVALID_INFRA":
        counters["infraFailures"] = int(counters.get("infraFailures", 0)) + 1
    elif batch_status == "INVALID_PROTOCOL":
        counters["protocolFailures"] = int(counters.get("protocolFailures", 0)) + 1
    elif batch_status == "INVALID_INPUT_DRIFT":
        counters["cleanStreak"] = 0

    batch = {
        "batchId": batch_id,
        "attempt": counters["attempts"],
        "inputFingerprint": current_fp,
        "rulesetFingerprint": ruleset_fp,
        "requiredRoles": expected_roles,
        "reviewers": [{
            "role": r.get("role"),
            "sessionId": r.get("sessionId"),
            "status": r.get("status") or "PASS",
            "report": r.get("report"),
        } for r in reviewer_reports],
        "status": batch_status,
        "findings": normalized,
        "violations": violations,
        "startedAt": _iso(now),
        "completedAt": _iso(now),
    }
    if previous_batch:
        session["batches"] = [b for b in session.get("batches", []) if b.get("batchId") != batch_id]
    session.setdefault("batches", []).append(batch)
    session["reviewers"] = batch["reviewers"]

    budget_reason = _budget_reason(session, now)
    target = _policy_clean_target(session)
    exit_reason = None
    next_action = "dispatch-next-batch"
    valid_cap_reached = (
        counters.get("validBatches", 0) >= int((session.get("budgets") or {}).get("maxValidBatches", 0))
        and counters.get("cleanStreak", 0) < target
    )
    invalid_disposition = any(_finding_requires_disposition(f) for f in normalized)
    open_high_risk = _open_high_risk(session.get("findings", []))
    if budget_reason or valid_cap_reached:
        exit_reason = "stalled"
        next_action = "escalate-user"
        if valid_cap_reached and not budget_reason:
            budget_reason = "max-valid-batches"
    elif batch_status == "INVALID_INFRA":
        next_action = "retry-failed-roles"
    elif batch_status == "INVALID_PROTOCOL":
        next_action = "repair-protocol-input"
    elif batch_status == "INVALID_INPUT_DRIFT":
        next_action = "start-new-fingerprint-session"
    elif batch_status == "CANCELLED":
        next_action = "resume-same-batch"
    elif batch_status == "VALID_FINDINGS" or invalid_disposition:
        exit_reason = "remediation-required"
        next_action = "remediate-findings"
    elif counters.get("cleanStreak", 0) >= target:
        exit_reason = "passed"
        next_action = "exit-passed"
    elif has_red_blocker:
        next_action = "remediate-findings"

    if exit_reason:
        session["exit"] = {"reason": exit_reason, "at": _iso(now), "batchId": batch_id}
    else:
        session["exit"] = None
    _set_projection(session)
    return {
        "schemaVersion": SCHEMA_VERSION,
        "batchId": batch_id,
        "batchStatus": batch_status,
        "round": session.get("round", 0),
        "attempts": counters.get("attempts", 0),
        "validBatches": counters.get("validBatches", 0),
        "cleanStreak": counters.get("cleanStreak", 0),
        "dryCounter": session.get("dryCounter", 0),
        "exitReason": session.get("exitReason"),
        "nextAction": next_action,
        "newFindings": normalized if batch_status == "VALID_FINDINGS" else [],
        "sessionCheck": {"passed": not violations, "reason": "; ".join(violations) or "batch accepted"},
        "violations": violations,
        "retryRoles": [r.get("role") for r, status in zip(reviewer_reports, statuses) if status != "PASS"],
        "budgetReason": budget_reason,
    }


def verify_exit(session: dict) -> tuple[bool, str]:
    if int(session.get("schemaVersion", 0) or 0) < SCHEMA_VERSION:
        return False, "legacy reviewLoop state requires migration before v2 verification"
    exit_obj = session.get("exit") or {}
    reason = exit_obj.get("reason")
    if reason == "passed":
        target = _policy_clean_target(session)
        clean = int((session.get("counters") or {}).get("cleanStreak", 0))
        if clean >= target:
            return True, f"passed: {clean} valid clean batch(es), target={target}"
        return False, f"passed state is inconsistent (cleanStreak={clean}, target={target})"
    if reason in {"remediation-required", "stalled", "cancelled"}:
        return False, f"review session is not passed: {reason}"
    return False, f"review session has no valid exit (reason={reason})"


def status(session: dict) -> dict:
    counters = session.get("counters") or {}
    return {
        "schemaVersion": session.get("schemaVersion", 1),
        "engine": session.get("engine", "legacy-round-v1"),
        "node": session.get("node"),
        "tier": session.get("tier"),
        "policy": session.get("policy"),
        "inputFingerprint": session.get("inputFingerprint", ""),
        "rulesetFingerprint": session.get("rulesetFingerprint", ""),
        "round": session.get("round", counters.get("attempts", 0)),
        "attempts": counters.get("attempts", 0),
        "validBatches": counters.get("validBatches", 0),
        "cleanStreak": counters.get("cleanStreak", 0),
        "dryCounter": session.get("dryCounter", counters.get("cleanStreak", 0)),
        "remediations": counters.get("remediations", 0),
        "infraFailures": counters.get("infraFailures", 0),
        "exitReason": (session.get("exit") or {}).get("reason") or session.get("exitReason"),
        "findingsCount": len(session.get("findings", [])),
        "batchCount": len(session.get("batches", [])),
        "budgets": session.get("budgets") or {},
        "reviewers": session.get("reviewers", []),
    }
