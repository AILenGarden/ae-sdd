from __future__ import annotations

import json
from pathlib import Path

import pytest

from lib.operations import OperationError, OperationRegistry


EXPECTED_OPERATIONS = {
    "workitem.get",
    "state.next_actions",
    "lease.acquire",
    "lease.renew",
    "lease.status",
    "lease.release",
    "lease.break",
    "state.transition",
    "execution.plan.set",
    "execution.plan.approve",
    "review.record",
    "document.resolve",
    "document.save",
    "gate.check",
    "verification.plan",
    "evidence.record",
    "evidence.finalize",
    "workitem.complete",
}


@pytest.fixture
def project(tmp_path: Path) -> Path:
    (tmp_path / ".ae-sdd").mkdir()
    (tmp_path / ".ae-sdd" / "config.yaml").write_text(
        "projectKey: test\n", encoding="utf-8"
    )
    state_dir = tmp_path / ".auto-engineering" / "WORK-001"
    state_dir.mkdir(parents=True)
    (state_dir / "state.json").write_text(
        json.dumps(
            {
                "version": "1",
                "projectKey": "test",
                "phase": "initialized",
                "scale": "微",
                "currentStory": "STORY-001",
                "history": [],
            }
        ),
        encoding="utf-8",
    )
    return tmp_path


def registry(project: Path, *, confirmed: bool = True) -> OperationRegistry:
    return OperationRegistry(
        project,
        confirmation_checker=lambda phase, work_item, story: confirmed,
        gate_checker=lambda gate_ids, work_item, story: [
            {"gateId": gate_id, "pass": True} for gate_id in gate_ids
        ],
    )


def acquire(registry: OperationRegistry, project: Path, key: str = "acquire-a") -> dict:
    return registry.execute(
        {
            "schemaVersion": "1",
            "operation": "lease.acquire",
            "project": str(project),
            "workItem": "WORK-001",
            "idempotencyKey": key,
            "parameters": {
                "owner": {
                    "agentId": "A",
                    "sessionId": "session-A",
                    "host": "test-host",
                    "pid": 1,
                },
                "ttlSeconds": 300,
            },
        }
    )


def transition_request(project: Path, lease: dict, **overrides: object) -> dict:
    request: dict = {
        "schemaVersion": "1",
        "operation": "state.transition",
        "project": str(project),
        "workItem": "WORK-001",
        "story": "STORY-001",
        "lease": {
            "leaseId": lease["leaseId"],
            "fencingToken": lease["fencingToken"],
        },
        "expectedRevision": 0,
        "idempotencyKey": "transition-1",
        "dryRun": False,
        "parameters": {"targetPhase": "route-selected"},
    }
    request.update(overrides)
    return request


def compact_write_request(
    project: Path, lease: dict, operation: str, parameters: dict,
    *, revision: int, key: str,
) -> dict:
    return {
        "schemaVersion": "1",
        "operation": operation,
        "project": str(project),
        "workItem": "WORK-001",
        "story": "STORY-001",
        "lease": {
            "leaseId": lease["leaseId"],
            "fencingToken": lease["fencingToken"],
        },
        "expectedRevision": revision,
        "idempotencyKey": key,
        "parameters": parameters,
    }


def assert_operation_error(exc: pytest.ExceptionInfo[OperationError], code: str) -> None:
    assert exc.value.code == code


def test_describe_exposes_versioned_typed_registry_without_raw_patch(project: Path) -> None:
    description = registry(project).describe()

    assert description["schemaVersion"] == "1"
    assert description["registryVersion"] == "1.1.0"
    names = {item["name"] for item in description["operations"]}
    assert names == EXPECTED_OPERATIONS
    assert "state.patch" not in names
    for item in description["operations"]:
        assert "inputSchema" in item
        assert "outputSchema" in item
        assert isinstance(item["writes"], bool)
        required = set(item["inputSchema"].get("required") or [])
        assert required <= set(item["inputSchema"].get("properties") or {})


@pytest.mark.parametrize("operation", ["state.patch", "unknown.operation"])
def test_unknown_or_raw_patch_operation_is_rejected(
    project: Path, operation: str
) -> None:
    with pytest.raises(OperationError) as exc:
        registry(project).execute(
            {
                "schemaVersion": "1",
                "operation": operation,
                "project": str(project),
                "workItem": "WORK-001",
                "parameters": {},
            }
        )

    assert_operation_error(exc, "OPERATION_NOT_REGISTERED")


@pytest.mark.parametrize(
    "parameters",
    [
        {"owner": {"agentId": "A"}, "ttlSeconds": "300"},
        {"owner": {"agentId": "A"}, "ttlSeconds": 29},
        {"owner": {"agentId": "A"}, "ttlSeconds": 300, "rawPatch": {}},
    ],
)
def test_execute_enforces_described_parameter_schema(project: Path, parameters: dict) -> None:
    with pytest.raises(OperationError) as exc:
        registry(project).execute({
            "schemaVersion": "1", "operation": "lease.acquire", "project": str(project),
            "workItem": "WORK-001", "idempotencyKey": "schema-check", "parameters": parameters,
        })
    assert_operation_error(exc, "OPERATION_SCHEMA_INVALID")


def test_next_actions_requires_lease_before_state_transition(project: Path) -> None:
    result = registry(project).next_actions("WORK-001", "STORY-001")

    assert result["revision"] == 0
    assert result["leaseStatus"]["status"] == "absent"
    assert result["nextActions"][0]["operation"] == "lease.acquire"


def test_next_actions_exposes_legal_transition_for_active_owner(project: Path) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]

    result = reg.next_actions("WORK-001", "STORY-001")

    assert result["leaseStatus"]["status"] == "active"
    assert any(
        action["operation"] == "state.transition"
        and action["parameters"]["targetPhase"] == "route-selected"
        for action in result["nextActions"]
    )


def test_lease_acquire_response_uses_common_operation_envelope(project: Path) -> None:
    response = acquire(registry(project), project)

    assert response["ok"] is True
    assert response["changed"] is True
    assert response["operation"] == "lease.acquire"
    assert response["workItem"] == "WORK-001"
    assert response["revisionBefore"] == 0
    assert response["revisionAfter"] == 0
    assert response["lease"]["fencingToken"] == 1
    assert response["error"] is None
    assert response["nextActions"]


@pytest.mark.parametrize(
    "missing_field",
    ["lease", "expectedRevision", "idempotencyKey"],
)
def test_state_transition_missing_write_precondition_fails_closed(
    project: Path, missing_field: str
) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    request = transition_request(project, lease)
    request.pop(missing_field)
    state_path = project / ".auto-engineering" / "WORK-001" / "state.json"
    before = state_path.read_bytes()

    with pytest.raises(OperationError) as exc:
        reg.execute(request)

    assert_operation_error(exc, "OPERATION_PRECONDITION_REQUIRED")
    assert missing_field in exc.value.details["missing"]
    assert state_path.read_bytes() == before


def test_dry_run_validates_and_projects_without_writing_any_file(project: Path) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    state_path = project / ".auto-engineering" / "WORK-001" / "state.json"
    lease_path = state_path.parent / "state.lease.json"
    before_state = state_path.read_bytes()
    before_lease = lease_path.read_bytes()
    request = transition_request(project, lease, dryRun=True)

    response = reg.execute(request)

    assert response["ok"] is True
    assert response["changed"] is False
    assert response["dryRun"] is True
    assert response["revisionBefore"] == 0
    assert response["revisionAfter"] == 1
    assert response["projectedState"]["phase"] == "route-selected"
    assert state_path.read_bytes() == before_state
    assert lease_path.read_bytes() == before_lease
    assert not (state_path.parent / "state.operations.json").exists()


def test_idempotent_retry_does_not_advance_state_twice(project: Path) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    request = transition_request(project, lease)

    first = reg.execute(request)
    retried = reg.execute(request)

    state = json.loads(
        (project / ".auto-engineering" / "WORK-001" / "state.json").read_text(
            encoding="utf-8"
        )
    )
    assert first["revisionAfter"] == 1
    assert retried["revisionAfter"] == 1
    assert retried["replayed"] is True
    assert state["revision"] == 1
    assert state["phase"] == "route-selected"


def test_same_idempotency_key_with_different_payload_is_rejected(project: Path) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    request = transition_request(project, lease)
    reg.execute(request)
    changed = transition_request(
        project,
        lease,
        parameters={"targetPhase": "requirement-analyzed"},
    )

    with pytest.raises(OperationError) as exc:
        reg.execute(changed)

    assert_operation_error(exc, "IDEMPOTENCY_KEY_REUSED")


def test_protected_coding_transition_requires_user_confirmation(project: Path) -> None:
    reg = registry(project, confirmed=False)
    lease = acquire(reg, project)["lease"]
    request = transition_request(
        project,
        lease,
        parameters={"targetPhase": "coding"},
    )
    before = (
        project / ".auto-engineering" / "WORK-001" / "state.json"
    ).read_bytes()

    with pytest.raises(OperationError) as exc:
        reg.execute(request)

    assert_operation_error(exc, "CONFIRMATION_REQUIRED")
    assert (
        project / ".auto-engineering" / "WORK-001" / "state.json"
    ).read_bytes() == before


def test_project_path_outside_registry_root_is_rejected(project: Path, tmp_path: Path) -> None:
    outside = tmp_path.parent / "outside-project"

    with pytest.raises(OperationError) as exc:
        registry(project).execute(
            {
                "schemaVersion": "1",
                "operation": "workitem.get",
                "project": str(outside),
                "workItem": "WORK-001",
                "parameters": {},
            }
        )

    assert_operation_error(exc, "PROJECT_ROOT_MISMATCH")


def test_document_resolve_and_save_use_work_item_scope_and_state_revision(project: Path) -> None:
    assets = project / ".ae-sdd" / "assets" / "test.assets.md"
    assets.parent.mkdir(parents=True, exist_ok=True)
    assets.write_text(f"| gitPath | {project} |\n", encoding="utf-8")
    source = project / "content.md"
    source.write_text("# generated\n", encoding="utf-8")
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    resolved = reg.execute({
        "schemaVersion": "1", "operation": "document.resolve", "project": str(project),
        "workItem": "WORK-001", "story": "STORY-001", "parameters": {"intent": "STORY"},
    })
    assert resolved["artifacts"][0]["path"].endswith("STORY-001.md")
    saved = reg.execute({
        "schemaVersion": "1", "operation": "document.save", "project": str(project),
        "workItem": "WORK-001", "story": "STORY-001", "lease": lease,
        "expectedRevision": 0, "idempotencyKey": "doc-save-1",
        "parameters": {"intent": "STORY", "contentFile": "content.md"},
    })
    assert saved["revisionAfter"] == 1
    assert Path(saved["artifacts"][0]["path"]).read_text(encoding="utf-8") == "# generated\n"


def test_verification_and_evidence_adapters_persist_through_store(project: Path) -> None:
    changed = project / "changed.java"
    changed.write_text("class Changed {}\n", encoding="utf-8")
    artifact = project / "result.json"
    artifact.write_text("{}\n", encoding="utf-8")
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    plan = reg.execute({
        "schemaVersion": "1", "operation": "verification.plan", "project": str(project),
        "workItem": "WORK-001", "story": "STORY-001", "lease": lease,
        "expectedRevision": 0, "idempotencyKey": "plan-1",
        "parameters": {"changedPaths": ["changed.java"]},
    })
    assert plan["revisionAfter"] == 1
    state_path = project / ".auto-engineering" / "WORK-001" / "state.json"
    current_lease = json.loads((state_path.parent / "state.lease.json").read_text(encoding="utf-8"))
    evidence_result = reg.execute({
        "schemaVersion": "1", "operation": "evidence.record", "project": str(project),
        "workItem": "WORK-001", "story": "STORY-001", "lease": lease,
        "expectedRevision": 1, "idempotencyKey": "evidence-1",
        "parameters": {"artifactPath": "result.json", "inputFingerprint": plan["artifacts"][0]["verificationPlan"]["evidenceInputFingerprint"],
                       "command": "pytest", "logicalKey": "g09"},
    })
    assert evidence_result["revisionAfter"] == 2
    assert evidence_result["artifacts"][0]["status"] == "active"
    finalized = reg.execute({
        "schemaVersion": "1", "operation": "evidence.finalize", "project": str(project),
        "workItem": "WORK-001", "story": "STORY-001", "lease": lease,
        "expectedRevision": 2, "idempotencyKey": "evidence-finalize-1", "parameters": {},
    })
    assert finalized["revisionAfter"] == 3


def test_compact_execution_plan_approval_and_review_are_state_only(project: Path) -> None:
    source = project / "src" / "service.py"
    source.parent.mkdir()
    source.write_text("def run():\n    return True\n", encoding="utf-8")
    reg = registry(project)
    lease = acquire(reg, project)["lease"]

    planned = reg.execute(compact_write_request(
        project, lease, "execution.plan.set",
        {
            "goal": "Implement AC-1",
            "changedPaths": ["src/service.py"],
            "verification": [{"id": "V-1", "acId": "AC-1", "command": "pytest -q"}],
            "risks": ["API compatibility"],
            "sourceReads": ["src/service.py"],
        },
        revision=0, key="compact-plan-set",
    ))
    assert planned["revisionAfter"] == 1
    assert planned["state"]["executionPlan"]["approved"] is False

    approved = reg.execute(compact_write_request(
        project, lease, "execution.plan.approve", {"approvedBy": "user"},
        revision=1, key="compact-plan-approve",
    ))
    assert approved["revisionAfter"] == 2
    assert approved["state"]["executionPlan"]["approved"] is True

    reviewed = reg.execute(compact_write_request(
        project, lease, "review.record",
        {
            "status": "changes_required",
            "findings": [{"severity": "P1", "problem": "Missing guard"}],
            "reviewedPaths": ["src/service.py"],
        },
        revision=2, key="compact-review-record",
    ))
    assert reviewed["revisionAfter"] == 3
    assert reviewed["state"]["review"]["status"] == "changes_required"
    assert not list(project.rglob("*CodingReport*.md"))
    assert not list(project.rglob("*TestReport*.md"))
    assert not list(project.rglob("*CodeReview*.md"))


def test_review_record_rejects_unstructured_findings(project: Path) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    request = compact_write_request(
        project, lease, "review.record",
        {"status": "changes_required", "findings": [{"problem": "No severity"}]},
        revision=0, key="compact-review-invalid",
    )

    with pytest.raises(OperationError) as exc:
        reg.execute(request)
    assert_operation_error(exc, "OPERATION_EXECUTION_FAILED")
    assert "severity" in exc.value.details["error"]


def test_verification_plan_rejects_changed_path_escape_with_stable_error(project: Path) -> None:
    reg = registry(project)
    lease = acquire(reg, project)["lease"]
    with pytest.raises(OperationError) as exc:
        reg.execute({
            "schemaVersion": "1", "operation": "verification.plan", "project": str(project),
            "workItem": "WORK-001", "story": "STORY-001", "lease": lease,
            "expectedRevision": 0, "idempotencyKey": "plan-escape",
            "parameters": {"changedPaths": ["../outside.java"]},
        })
    assert_operation_error(exc, "PATH_OUTSIDE_PROJECT")


def test_gate_check_rejects_unknown_gate_with_stable_error(project: Path) -> None:
    with pytest.raises(OperationError) as exc:
        OperationRegistry(project).execute({
            "schemaVersion": "1", "operation": "gate.check", "project": str(project),
            "workItem": "WORK-001", "story": "STORY-001",
            "parameters": {"gateIds": ["G-NOT-REAL"]},
        })
    assert_operation_error(exc, "GATE_NOT_REGISTERED")
