"""Review Batch v2 state-machine coverage."""
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import review_batch as rb


def _reports(statuses=None, findings=None):
    statuses = statuses or ["PASS", "PASS", "PASS"]
    roles = ["BE", "AR", "QA"]
    return [
        {
            "role": roles[i],
            "sessionId": f"sid-{role}",
            "status": status,
            "report": f"{role}.md",
            "findings": (findings or []) if i == 0 else [],
        }
        for i, (role, status) in enumerate(zip(roles, statuses))
    ]


def test_tier3_first_clean_passes():
    session = rb.create_session("code-review", 3, input_fingerprint="sha256:input")
    result = rb.collect_batch(session, _reports(), "root", input_fingerprint="sha256:input")
    assert result["batchStatus"] == "VALID_CLEAN"
    assert result["exitReason"] == "passed"
    assert rb.verify_exit(session)[0] is True


def test_infra_failure_does_not_increment_clean_and_only_retries_failed_role():
    session = rb.create_session("code-review", 3, input_fingerprint="sha256:input")
    result = rb.collect_batch(
        session,
        _reports(["PASS", "ERROR_RATE_LIMIT", "PASS"]),
        "root",
        input_fingerprint="sha256:input",
    )
    assert result["batchStatus"] == "INVALID_INFRA"
    assert result["cleanStreak"] == 0
    assert result["retryRoles"] == ["AR"]
    assert rb.verify_exit(session)[0] is False


def test_input_drift_invalidates_batch():
    session = rb.create_session("code-review", 3, input_fingerprint="sha256:old")
    result = rb.collect_batch(session, _reports(), "root", input_fingerprint="sha256:new")
    assert result["batchStatus"] == "INVALID_INPUT_DRIFT"
    assert result["nextAction"] == "start-new-fingerprint-session"
    assert result["cleanStreak"] == 0


def test_tier3_remediation_requires_two_clean_batches():
    finding = {"ruleId": "CR-001", "path": "src/A.java", "symbol": "A.run", "severity": "P1"}
    session = rb.create_session("code-review", 3, input_fingerprint="sha256:input")
    first = rb.collect_batch(session, _reports(findings=[finding]), "root", input_fingerprint="sha256:input")
    assert first["exitReason"] == "remediation-required"
    clean1 = rb.collect_batch(session, _reports(), "root", input_fingerprint="sha256:input")
    assert clean1["exitReason"] is None
    clean2 = rb.collect_batch(session, _reports(), "root", input_fingerprint="sha256:input")
    assert clean2["exitReason"] == "passed"


def test_budget_exhaustion_is_stalled_not_passed():
    session = rb.create_session(
        "code-review",
        3,
        input_fingerprint="sha256:input",
        budgets={"maxAttempts": 1},
    )
    result = rb.collect_batch(
        session,
        _reports(["PASS", "ERROR_RATE_LIMIT", "PASS"]),
        "root",
        input_fingerprint="sha256:input",
    )
    assert result["exitReason"] == "stalled"
    assert result["nextAction"] == "escalate-user"
    assert rb.verify_exit(session)[0] is False


def test_legacy_import_does_not_fabricate_valid_batches():
    legacy = {"node": "code-review", "tier": 3, "round": 2, "dryCounter": 2, "findings": []}
    session = rb.upgrade_legacy(legacy)
    assert session["schemaVersion"] == 2
    assert session["counters"]["validBatches"] == 0
    assert session["legacyImported"] is True


def test_fingerprint_manifest_is_order_independent():
    assert rb.fingerprint_manifest({"b": 2, "a": 1}) == rb.fingerprint_manifest({"a": 1, "b": 2})


def test_retry_merges_failed_role_into_same_batch():
    session = rb.create_session("code-review", 3, input_fingerprint="sha256:input")
    first = rb.collect_batch(
        session,
        _reports(["PASS", "ERROR_RATE_LIMIT", "PASS"]),
        "root",
        input_fingerprint="sha256:input",
        batch_id="b1",
    )
    assert first["batchStatus"] == "INVALID_INFRA"
    retry = rb.collect_batch(
        session,
        [{"role": "AR", "sessionId": "sid-ar-retry", "status": "PASS", "report": "AR-retry.md", "findings": []}],
        "root",
        input_fingerprint="sha256:input",
        batch_id="b1",
    )
    assert retry["batchStatus"] == "VALID_CLEAN"
    assert retry["validBatches"] == 1
    assert len(session["batches"]) == 1
    assert {item["role"] for item in session["batches"][0]["reviewers"]} == {"BE", "AR", "QA"}


def test_new_input_generation_preserves_remediation_policy():
    session = rb.create_session("code-review", 3, input_fingerprint="sha256:old")
    rb.collect_batch(session, _reports(findings=[{"ruleId": "CR-1", "path": "src/A.java", "severity": "P1"}]), "root", input_fingerprint="sha256:old")
    next_session = rb.restart_for_fingerprint(session, "sha256:new")
    assert next_session["inputFingerprint"] == "sha256:new"
    assert next_session["counters"]["remediations"] == 1
    assert next_session["findings"][0]["status"] == "FIXED_PENDING_VERIFY"
    assert rb._policy_clean_target(next_session) == 2


def test_disposition_requires_authorization_or_dependency():
    session = rb.create_session("code-review", 2, input_fingerprint="sha256:input")
    result = rb.collect_batch(
        session,
        _reports(findings=[{"ruleId": "CR-2", "path": "src/A.java", "severity": "P2", "status": "ACCEPTED_RISK"}])[:2],
        "root",
        input_fingerprint="sha256:input",
    )
    assert result["exitReason"] == "remediation-required"
