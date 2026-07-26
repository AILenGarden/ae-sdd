"""Typed, transport-independent operation protocol for AE-SDD agents."""

from __future__ import annotations

import copy
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Optional

from lib import document_storage, evidence, paths, state, verification_plan, work_item_context
from lib.state_store import LeaseOwner, StateStore, StateStoreError


SCHEMA_VERSION = "1"
REGISTRY_VERSION = "1.1.0"
PROTECTED_TRANSITIONS = {"coding"}


class OperationError(RuntimeError):
    """Stable operation-protocol error returned by every transport."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        details: Optional[dict[str, Any]] = None,
        remediation: str = "",
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.details = details or {}
        self.remediation = remediation

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "message": self.message,
            "remediation": self.remediation,
            "details": self.details,
        }


@dataclass(frozen=True)
class OperationDefinition:
    name: str
    writes: bool
    requires_lease: bool
    requires_confirmation: bool
    input_schema: dict[str, Any]
    output_schema: dict[str, Any]
    handler_name: str

    def describe(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "writes": self.writes,
            "requiresLease": self.requires_lease,
            "requiresConfirmation": self.requires_confirmation,
            "inputSchema": self.input_schema,
            "outputSchema": self.output_schema,
        }


def _object_schema(required: list[str], properties: dict[str, Any]) -> dict[str, Any]:
    return {
        "type": "object",
        "additionalProperties": False,
        "required": required,
        "properties": properties,
    }


COMMON_OUTPUT_SCHEMA = _object_schema(
    [
        "ok",
        "changed",
        "operation",
        "workItem",
        "revisionBefore",
        "revisionAfter",
        "artifacts",
        "gateResults",
        "nextActions",
        "error",
    ],
    {
        "ok": {"type": "boolean"},
        "changed": {"type": "boolean"},
        "operation": {"type": "string"},
        "workItem": {"type": "string"},
        "revisionBefore": {"type": ["integer", "null"]},
        "revisionAfter": {"type": ["integer", "null"]},
        "artifacts": {"type": "array"},
        "gateResults": {"type": "array"},
        "nextActions": {"type": "array"},
        "error": {"type": ["object", "null"]},
    },
)


def _parameter_schema(
    required: list[str] | None = None,
    properties: Optional[dict[str, Any]] = None,
) -> dict[str, Any]:
    return _object_schema(required or [], properties or {})


def _matches_json_type(value: Any, expected: str) -> bool:
    checks = {
        "object": lambda item: isinstance(item, dict),
        "array": lambda item: isinstance(item, list),
        "string": lambda item: isinstance(item, str),
        "integer": lambda item: type(item) is int,
        "boolean": lambda item: type(item) is bool,
        "null": lambda item: item is None,
    }
    return expected in checks and checks[expected](value)


def _schema_violations(value: dict[str, Any], schema: dict[str, Any]) -> list[dict[str, Any]]:
    violations: list[dict[str, Any]] = []
    properties = schema.get("properties") or {}
    if schema.get("additionalProperties") is False:
        for name in sorted(set(value) - set(properties)):
            violations.append({"field": f"parameters.{name}", "reason": "unknown-field"})
    for name, field_schema in properties.items():
        if name not in value:
            continue
        item = value[name]
        expected = field_schema.get("type")
        accepted = expected if isinstance(expected, list) else [expected]
        if expected and not any(_matches_json_type(item, kind) for kind in accepted):
            violations.append({"field": f"parameters.{name}", "reason": "type", "expected": accepted})
            continue
        if type(item) is int:
            if "minimum" in field_schema and item < int(field_schema["minimum"]):
                violations.append({"field": f"parameters.{name}", "reason": "minimum", "minimum": field_schema["minimum"]})
            if "maximum" in field_schema and item > int(field_schema["maximum"]):
                violations.append({"field": f"parameters.{name}", "reason": "maximum", "maximum": field_schema["maximum"]})
        if isinstance(item, (str, list)):
            minimum = field_schema.get("minLength", field_schema.get("minItems"))
            if minimum is not None and len(item) < int(minimum):
                violations.append({"field": f"parameters.{name}", "reason": "minimum-length", "minimum": minimum})
    return violations


# ─── execution.resume migration-oracle（plan P0 Task 12）──────────────────────
#
# 只有 Rust daemon 计算权威 execution capsule/projection；这里的 builder 只镜像
# 薄 Rust CLI（bins/ae-sdd-cli resume-approved-plan）组装的 request shape 与冻结
# 的 response shape，供 migration harness 做 differential parity。只读、零文件
# I/O，且刻意不在 _DEFINITIONS 注册执行入口 —— Python 永不成为 execution.resume
# 的 canary/sole-writer fallback。

EXECUTION_RESUME_OPERATION = "execution.resume"

EXECUTION_RESUME_INPUT_SCHEMA = _parameter_schema(
    [],
    {
        "knownCapsuleDigest": {"type": "string"},
        "knownContextRevision": {"type": "integer", "minimum": 0},
    },
)

EXECUTION_RESUME_RESPONSE_SHAPE = (
    "projectionKind",
    "contextRevision",
    "capsuleDigest",
    "capsule",
    "nextAction",
    "authorityRefreshCount",
)

_EXECUTION_RESUME_REQUIRED_IDENTITY = ("workspaceId", "agentId", "sessionId", "workItemId")
_EXECUTION_RESUME_OPTIONAL_FIELDS = (
    "turnId",
    "capabilityToken",
    "deadlineMs",
    "knownCapsuleDigest",
    "knownContextRevision",
)


def execution_resume_response_shape() -> list[str]:
    """Return the frozen `execution.resume` daemon data keys rendered by both CLIs."""
    return list(EXECUTION_RESUME_RESPONSE_SHAPE)


def build_execution_resume_request(request: dict[str, Any]) -> dict[str, Any]:
    """Assemble the frozen `operation.execute` payload for `execution.resume`.

    Migration-oracle only: mirrors the thin Rust CLI envelope so the migration
    harness can diff request/response shape parity. Pure function — no
    filesystem, clock or environment access.
    """
    if not isinstance(request, dict):
        raise OperationError("OPERATION_SCHEMA_INVALID", "resume request must be a JSON object")
    allowed = set(_EXECUTION_RESUME_REQUIRED_IDENTITY) | set(_EXECUTION_RESUME_OPTIONAL_FIELDS)
    unknown = sorted(set(request) - allowed)
    if unknown:
        raise OperationError(
            "OPERATION_SCHEMA_INVALID",
            "resume request carries unknown fields",
            details={"unknown": unknown},
        )
    missing = [
        name
        for name in _EXECUTION_RESUME_REQUIRED_IDENTITY
        if not isinstance(request.get(name), str) or not request[name].strip()
    ]
    if missing:
        raise OperationError(
            "OPERATION_SCHEMA_INVALID",
            "resume request identity is incomplete",
            details={"missing": missing},
        )
    for name in ("turnId", "capabilityToken"):
        if name in request and not isinstance(request[name], str):
            raise OperationError(
                "OPERATION_SCHEMA_INVALID",
                "resume request identity fields must be strings",
                details={"field": name},
            )
    deadline = request.get("deadlineMs")
    if deadline is not None and (type(deadline) is not int or deadline < 1):
        raise OperationError(
            "OPERATION_SCHEMA_INVALID",
            "deadlineMs must be a positive integer millisecond budget",
            details={"field": "deadlineMs"},
        )
    cursor = {
        key: request[key]
        for key in ("knownCapsuleDigest", "knownContextRevision")
        if key in request
    }
    violations = _schema_violations(cursor, EXECUTION_RESUME_INPUT_SCHEMA)
    if violations:
        raise OperationError(
            "OPERATION_SCHEMA_INVALID",
            "resume cursor does not match the registered schema",
            details={"violations": violations},
        )
    return {
        "operation": EXECUTION_RESUME_OPERATION,
        "dryRun": False,
        "payload": cursor,
    }


_DEFINITIONS = [
    OperationDefinition("workitem.get", False, False, False, _parameter_schema(), COMMON_OUTPUT_SCHEMA, "_handle_workitem_get"),
    OperationDefinition("state.next_actions", False, False, False, _parameter_schema(), COMMON_OUTPUT_SCHEMA, "_handle_state_next_actions"),
    OperationDefinition("lease.acquire", True, False, False, _parameter_schema(["owner", "ttlSeconds"], {"owner": {"type": "object"}, "ttlSeconds": {"type": "integer", "minimum": 30, "maximum": 3600}}), COMMON_OUTPUT_SCHEMA, "_handle_lease_acquire"),
    OperationDefinition("lease.renew", True, True, False, _parameter_schema(["owner", "ttlSeconds"], {"owner": {"type": "object"}, "ttlSeconds": {"type": "integer", "minimum": 30, "maximum": 3600}}), COMMON_OUTPUT_SCHEMA, "_handle_lease_renew"),
    OperationDefinition("lease.status", False, False, False, _parameter_schema(), COMMON_OUTPUT_SCHEMA, "_handle_lease_status"),
    OperationDefinition("lease.release", True, True, False, _parameter_schema(["owner"], {"owner": {"type": "object"}}), COMMON_OUTPUT_SCHEMA, "_handle_lease_release"),
    OperationDefinition("lease.break", True, False, False, _parameter_schema(["actor", "reason"], {"actor": {"type": "object"}, "reason": {"type": "string", "minLength": 1}}), COMMON_OUTPUT_SCHEMA, "_handle_lease_break"),
    OperationDefinition("state.transition", True, True, True, _parameter_schema(["targetPhase"], {"targetPhase": {"type": "string"}}), COMMON_OUTPUT_SCHEMA, "_handle_state_transition"),
    OperationDefinition("execution.plan.set", True, True, False, _parameter_schema(
        ["goal", "changedPaths", "verification"],
        {"goal": {"type": "string", "minLength": 1},
         "changedPaths": {"type": "array", "items": {"type": "string"}, "minItems": 1},
         "verification": {"type": "array", "items": {"type": "object"}, "minItems": 1},
         "risks": {"type": "array", "items": {"type": "string"}},
         "sourceReads": {"type": "array", "items": {"type": "string"}}}),
        COMMON_OUTPUT_SCHEMA, "_handle_execution_plan_set"),
    OperationDefinition("execution.plan.approve", True, True, True, _parameter_schema(
        [], {"approvedBy": {"type": "string"}}),
        COMMON_OUTPUT_SCHEMA, "_handle_execution_plan_approve"),
    OperationDefinition("review.record", True, True, False, _parameter_schema(
        ["status", "findings"],
        {"status": {"type": "string", "enum": ["pending", "passed", "changes_required"]},
         "findings": {"type": "array", "items": {"type": "object"}},
         "reviewedPaths": {"type": "array", "items": {"type": "string"}},
         "evidenceIds": {"type": "array", "items": {"type": "string"}}}),
        COMMON_OUTPUT_SCHEMA, "_handle_review_record"),
    OperationDefinition("document.resolve", False, False, False, _parameter_schema(["intent"], {"intent": {"type": "string"}, "docId": {"type": "string"}, "version": {"type": ["object", "string"]}}), COMMON_OUTPUT_SCHEMA, "_handle_deferred_adapter"),
    OperationDefinition("document.save", True, True, False, _parameter_schema(["intent", "contentFile"], {"intent": {"type": "string"}, "contentFile": {"type": "string"}, "docId": {"type": "string"}, "version": {"type": ["object", "string"]}, "changelogNote": {"type": "string"}}), COMMON_OUTPUT_SCHEMA, "_handle_deferred_adapter"),
    OperationDefinition("gate.check", False, False, False, _parameter_schema([], {"gateIds": {"type": "array", "items": {"type": "string"}}}), COMMON_OUTPUT_SCHEMA, "_handle_deferred_adapter"),
    OperationDefinition("verification.plan", True, True, False, _parameter_schema(["changedPaths"], {"changedPaths": {"type": "array", "items": {"type": "string"}, "minItems": 1}, "sinceFingerprint": {"type": "string"}, "persist": {"type": "boolean"}}), COMMON_OUTPUT_SCHEMA, "_handle_deferred_adapter"),
    OperationDefinition("evidence.record", True, True, False, _parameter_schema(["artifactPath", "inputFingerprint"], {"artifactPath": {"type": "string"}, "inputFingerprint": {"type": "string"}, "kind": {"type": "string"}, "command": {"type": ["string", "array"]}, "toolchainFingerprint": {"type": "string"}, "exitCode": {"type": "integer"}, "summary": {"type": "object"}, "durationMs": {"type": "integer"}, "logicalKey": {"type": "string"}}), COMMON_OUTPUT_SCHEMA, "_handle_deferred_adapter"),
    OperationDefinition("evidence.finalize", True, True, False, _parameter_schema(), COMMON_OUTPUT_SCHEMA, "_handle_deferred_adapter"),
    OperationDefinition("workitem.complete", True, True, True, _parameter_schema(), COMMON_OUTPUT_SCHEMA, "_handle_workitem_complete"),
]


class OperationRegistry:
    def __init__(
        self,
        project_dir: Path,
        *,
        confirmation_checker: Optional[Callable[[str, str, str], bool]] = None,
        gate_checker: Optional[Callable[[list[str], str, str], list[dict[str, Any]]]] = None,
    ) -> None:
        self.project_dir = Path(project_dir).resolve()
        self.ade_sdd = self.project_dir / ".ae-sdd"
        self._confirmation_checker = confirmation_checker or self._default_confirmation_checker
        self._gate_checker = gate_checker or self._default_gate_checker
        self._definitions = {definition.name: definition for definition in _DEFINITIONS}

    def describe(self, name: str = "") -> dict[str, Any]:
        if name:
            definition = self._definitions.get(name)
            if definition is None:
                raise OperationError(
                    "OPERATION_NOT_REGISTERED",
                    f"operation is not registered: {name}",
                    details={"operation": name},
                )
            operations = [definition.describe()]
        else:
            operations = [
                definition.describe()
                for definition in sorted(self._definitions.values(), key=lambda item: item.name)
            ]
        return {
            "schemaVersion": SCHEMA_VERSION,
            "registryVersion": REGISTRY_VERSION,
            "operations": operations,
        }

    def next_actions(self, work_item: str, story: str = "") -> dict[str, Any]:
        state_path = self._resolve_state_path(work_item)
        store = StateStore(state_path)
        snapshot = store.read()
        lease_status = store.lease_status()
        actions: list[dict[str, Any]] = []
        if lease_status["status"] in {"absent", "expired"}:
            actions.append(
                {
                    "operation": "lease.acquire",
                    "required": ["workItem", "idempotencyKey", "parameters.owner"],
                    "parameters": {"ttlSeconds": 300},
                }
            )
        else:
            suggestion = state.next_step_suggestion(snapshot.state)
            if suggestion.get("next"):
                actions.append(
                    {
                        "operation": "state.transition",
                        "required": [
                            "lease",
                            "expectedRevision",
                            "idempotencyKey",
                        ],
                        "parameters": {"targetPhase": suggestion["next"]},
                    }
                )
            actions.extend(
                [
                    {
                        "operation": "lease.renew",
                        "parameters": {"ttlSeconds": 300},
                    },
                    {"operation": "lease.release", "parameters": {}},
                ]
            )
        return {
            "workItem": work_item,
            "story": story or state.get_active_story(snapshot.state) or "",
            "revision": snapshot.revision,
            "phase": state.get_active_phase(snapshot.state),
            "leaseStatus": lease_status,
            "nextActions": actions,
        }

    def execute(self, request: dict[str, Any]) -> dict[str, Any]:
        definition = self._validate_envelope(request)
        handler = getattr(self, definition.handler_name)
        try:
            return handler(request, definition)
        except OperationError:
            raise
        except StateStoreError as exc:
            raise OperationError(
                exc.code,
                exc.message,
                details=exc.details,
                remediation=exc.remediation,
            ) from exc
        except (OSError, ValueError) as exc:
            raise OperationError(
                "OPERATION_EXECUTION_FAILED",
                "operation adapter rejected the request",
                details={"operation": definition.name, "error": str(exc)},
            ) from exc

    def _validate_envelope(self, request: dict[str, Any]) -> OperationDefinition:
        if not isinstance(request, dict):
            raise OperationError("OPERATION_SCHEMA_INVALID", "operation request must be an object")
        if request.get("schemaVersion") != SCHEMA_VERSION:
            raise OperationError(
                "OPERATION_SCHEMA_VERSION_UNSUPPORTED",
                "unsupported operation schemaVersion",
                details={"supported": [SCHEMA_VERSION], "provided": request.get("schemaVersion")},
            )
        operation = request.get("operation")
        definition = self._definitions.get(str(operation))
        if definition is None:
            raise OperationError(
                "OPERATION_NOT_REGISTERED",
                f"operation is not registered: {operation}",
                details={"operation": operation, "describe": "ae-sdd ops describe --json"},
                remediation="call ops describe and choose a registered typed operation",
            )
        provided_project = request.get("project")
        if not provided_project:
            raise OperationError(
                "OPERATION_SCHEMA_INVALID",
                "project is required",
                details={"missing": ["project"]},
            )
        try:
            resolved_project = Path(str(provided_project)).resolve()
        except OSError as exc:
            raise OperationError("PROJECT_ROOT_INVALID", "project root cannot be resolved") from exc
        if resolved_project != self.project_dir:
            raise OperationError(
                "PROJECT_ROOT_MISMATCH",
                "request project does not match the registry project root",
                details={"expected": str(self.project_dir), "provided": str(resolved_project)},
            )
        work_item = str(request.get("workItem") or "").strip()
        if not work_item:
            raise OperationError(
                "OPERATION_SCHEMA_INVALID",
                "workItem is required; implicit active-state writes are not allowed",
                details={"missing": ["workItem"]},
            )
        parameters = request.get("parameters", {})
        if not isinstance(parameters, dict):
            raise OperationError(
                "OPERATION_SCHEMA_INVALID",
                "parameters must be an object",
                details={"field": "parameters"},
            )
        required_parameters = definition.input_schema.get("required") or []
        missing_parameters = [name for name in required_parameters if name not in parameters]
        if missing_parameters:
            raise OperationError(
                "OPERATION_SCHEMA_INVALID",
                "operation parameters are incomplete",
                details={"missing": [f"parameters.{name}" for name in missing_parameters]},
            )
        violations = _schema_violations(parameters, definition.input_schema)
        if violations:
            raise OperationError(
                "OPERATION_SCHEMA_INVALID",
                "operation parameters do not match the registered schema",
                details={"violations": violations},
                remediation="call ae-sdd ops describe and rebuild the typed request",
            )
        if definition.requires_lease:
            missing = self._missing_write_preconditions(request)
            if missing:
                raise OperationError(
                    "OPERATION_PRECONDITION_REQUIRED",
                    "write operation is missing concurrency preconditions",
                    details={"missing": missing},
                    remediation="read state/lease and retry with explicit preconditions",
                )
        elif definition.writes and operation != "lease.break":
            if not request.get("idempotencyKey"):
                raise OperationError(
                    "OPERATION_PRECONDITION_REQUIRED",
                    "write operation requires an idempotency key",
                    details={"missing": ["idempotencyKey"]},
                )
        return definition

    @staticmethod
    def _missing_write_preconditions(request: dict[str, Any]) -> list[str]:
        missing: list[str] = []
        lease = request.get("lease")
        if not isinstance(lease, dict):
            missing.append("lease")
        else:
            if not lease.get("leaseId"):
                missing.append("lease.leaseId")
            if type(lease.get("fencingToken")) is not int:
                missing.append("lease.fencingToken")
        if type(request.get("expectedRevision")) is not int:
            missing.append("expectedRevision")
        if not request.get("idempotencyKey"):
            missing.append("idempotencyKey")
        return missing

    def _handle_workitem_get(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        snapshot = store.read()
        return self._response(
            definition.name,
            work_item,
            changed=False,
            revision_before=snapshot.revision,
            revision_after=snapshot.revision,
            state_value=snapshot.state,
            next_actions=self.next_actions(work_item, str(request.get("story") or ""))["nextActions"],
        )

    def _handle_state_next_actions(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        result = self.next_actions(
            str(request["workItem"]), str(request.get("story") or "")
        )
        return self._response(
            definition.name,
            str(request["workItem"]),
            changed=False,
            revision_before=result["revision"],
            revision_after=result["revision"],
            next_actions=result["nextActions"],
            extra={
                "phase": result["phase"],
                "leaseStatus": result["leaseStatus"],
                "story": result["story"],
            },
        )

    def _handle_lease_acquire(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        before = store.read().revision
        parameters = request["parameters"]
        lease = store.acquire_lease(
            LeaseOwner.from_dict(parameters["owner"]),
            int(parameters["ttlSeconds"]),
            str(request["idempotencyKey"]),
        )
        return self._response(
            definition.name,
            work_item,
            changed=True,
            revision_before=before,
            revision_after=before,
            next_actions=self.next_actions(work_item, str(request.get("story") or ""))["nextActions"],
            extra={"lease": lease.to_dict()},
        )

    def _handle_lease_status(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        snapshot = store.read()
        return self._response(
            definition.name,
            work_item,
            changed=False,
            revision_before=snapshot.revision,
            revision_after=snapshot.revision,
            extra={"leaseStatus": store.lease_status()},
        )

    def _handle_lease_renew(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        snapshot = store.read()
        lease_ref = request["lease"]
        parameters = request["parameters"]
        lease = store.renew_lease(
            LeaseOwner.from_dict(parameters["owner"]),
            str(lease_ref["leaseId"]),
            int(lease_ref["fencingToken"]),
            int(parameters["ttlSeconds"]),
        )
        return self._response(
            definition.name,
            work_item,
            changed=True,
            revision_before=snapshot.revision,
            revision_after=snapshot.revision,
            extra={"lease": lease.to_dict()},
        )

    def _handle_lease_release(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        snapshot = store.read()
        lease_ref = request["lease"]
        result = store.release_lease(
            LeaseOwner.from_dict(request["parameters"]["owner"]),
            str(lease_ref["leaseId"]),
            int(lease_ref["fencingToken"]),
        )
        return self._response(
            definition.name,
            work_item,
            changed=True,
            revision_before=snapshot.revision,
            revision_after=snapshot.revision,
            extra={"leaseStatus": result},
        )

    def _handle_lease_break(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        snapshot = store.read()
        result = store.break_lease(
            LeaseOwner.from_dict(request["parameters"]["actor"]),
            str(request["parameters"]["reason"]),
        )
        return self._response(
            definition.name,
            work_item,
            changed=True,
            revision_before=snapshot.revision,
            revision_after=snapshot.revision,
            extra={"leaseStatus": result},
        )

    def _handle_state_transition(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        story = str(request.get("story") or "")
        target = str(request["parameters"]["targetPhase"])
        if target in PROTECTED_TRANSITIONS and not self._confirmation_checker(
            target, work_item, story
        ):
            raise OperationError(
                "CONFIRMATION_REQUIRED",
                f"transition to {target} requires an explicit user confirmation token",
                details={"phase": target, "workItem": work_item, "story": story},
                remediation=f"run ae-sdd state confirm --phase {target} --work-item {work_item}",
            )
        gate_results = self._transition_gate_results(target, work_item, story)
        failed = [result for result in gate_results if not result.get("pass")]
        if failed:
            raise OperationError(
                "GATE_BLOCKED",
                "one or more transition gates failed",
                details={"gateResults": failed},
            )
        store = StateStore(self._resolve_state_path(work_item))
        lease_ref = request["lease"]
        expected_revision = int(request["expectedRevision"])

        def apply_transition(state_value: dict[str, Any]) -> dict[str, Any]:
            self._validate_legal_transition(state_value, target)
            if state.is_nested_state(state_value):
                story_id = story or state.get_active_story(state_value) or ""
                if not story_id or state.get_story_substate(state_value, story_id) is None:
                    raise OperationError(
                        "STORY_SCOPE_REQUIRED",
                        "nested state transition requires a managed Story",
                        details={"story": story_id},
                    )
                state.set_story_substate_phase(
                    state_value, story_id, target, by="ae-sdd ops state.transition"
                )
            else:
                state.set_phase(state_value, target, by="ae-sdd ops state.transition")
            return {"phase": target}

        if bool(request.get("dryRun", False)):
            snapshot = store.validate_mutation(
                expected_revision=expected_revision,
                lease_id=str(lease_ref["leaseId"]),
                fencing_token=int(lease_ref["fencingToken"]),
            )
            projected = copy.deepcopy(snapshot.state)
            apply_transition(projected)
            return self._response(
                definition.name,
                work_item,
                changed=False,
                revision_before=snapshot.revision,
                revision_after=snapshot.revision + 1,
                gate_results=gate_results,
                extra={"dryRun": True, "projectedState": projected, "replayed": False},
            )
        response = store.mutate(
            expected_revision=expected_revision,
            lease_id=str(lease_ref["leaseId"]),
            fencing_token=int(lease_ref["fencingToken"]),
            idempotency_key=str(request["idempotencyKey"]),
            operation=definition.name,
            payload=request["parameters"],
            mutate=apply_transition,
        )
        return self._response(
            definition.name,
            work_item,
            changed=response.changed,
            revision_before=response.revision_before,
            revision_after=response.revision_after,
            gate_results=gate_results,
            state_value=response.state,
            next_actions=self.next_actions(work_item, story)["nextActions"],
            extra={"dryRun": False, "replayed": response.replayed},
        )

    def _handle_workitem_complete(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        forwarded = dict(request)
        forwarded["operation"] = "state.transition"
        forwarded["parameters"] = {"targetPhase": "completed"}
        result = self._handle_state_transition(forwarded, self._definitions["state.transition"])
        result["operation"] = definition.name
        return result

    def _mutate_compact_process_state(
        self, request: dict[str, Any], definition: OperationDefinition,
        mutate_state: Callable[[dict[str, Any]], dict[str, Any]],
    ) -> dict[str, Any]:
        work_item = str(request["workItem"])
        store = StateStore(self._resolve_state_path(work_item))
        lease_ref = request["lease"]
        expected_revision = int(request["expectedRevision"])
        if bool(request.get("dryRun", False)):
            snapshot = store.validate_mutation(
                expected_revision=expected_revision,
                lease_id=str(lease_ref["leaseId"]),
                fencing_token=int(lease_ref["fencingToken"]),
            )
            projected = copy.deepcopy(snapshot.state)
            result = mutate_state(projected)
            return self._response(
                definition.name, work_item, changed=False,
                revision_before=snapshot.revision, revision_after=snapshot.revision + 1,
                state_value=projected, extra={"dryRun": True, "result": result},
            )
        response = store.mutate(
            expected_revision=expected_revision,
            lease_id=str(lease_ref["leaseId"]),
            fencing_token=int(lease_ref["fencingToken"]),
            idempotency_key=str(request["idempotencyKey"]),
            operation=definition.name,
            payload=request["parameters"],
            mutate=mutate_state,
        )
        return self._response(
            definition.name, work_item, changed=response.changed,
            revision_before=response.revision_before, revision_after=response.revision_after,
            state_value=response.state, next_actions=self.next_actions(
                work_item, str(request.get("story") or "")
            )["nextActions"], extra={"dryRun": False, "replayed": response.replayed,
                                      "result": response.result},
        )

    def _handle_execution_plan_set(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        parameters = request["parameters"]

        def mutate_state(state_value: dict[str, Any]) -> dict[str, Any]:
            return state.set_execution_plan(
                state_value,
                goal=str(parameters["goal"]),
                changed_paths=[str(item) for item in parameters["changedPaths"]],
                verification=list(parameters["verification"]),
                risks=[str(item) for item in parameters.get("risks") or []],
                source_reads=[str(item) for item in parameters.get("sourceReads") or []],
                by="ae-sdd ops execution.plan.set",
            )

        return self._mutate_compact_process_state(request, definition, mutate_state)

    def _handle_execution_plan_approve(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        approved_by = str(request["parameters"].get("approvedBy") or "user")

        def mutate_state(state_value: dict[str, Any]) -> dict[str, Any]:
            return state.approve_execution_plan(state_value, by=approved_by)

        return self._mutate_compact_process_state(request, definition, mutate_state)

    def _handle_review_record(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        parameters = request["parameters"]

        def mutate_state(state_value: dict[str, Any]) -> dict[str, Any]:
            return state.record_review(
                state_value,
                status=str(parameters["status"]),
                findings=list(parameters["findings"]),
                reviewed_paths=[str(item) for item in parameters.get("reviewedPaths") or []],
                evidence_ids=[str(item) for item in parameters.get("evidenceIds") or []],
                by="ae-sdd ops review.record",
            )

        return self._mutate_compact_process_state(request, definition, mutate_state)

    def _handle_deferred_adapter(
        self, request: dict[str, Any], definition: OperationDefinition
    ) -> dict[str, Any]:
        handlers = {
            "document.resolve": self._handle_document_resolve,
            "document.save": self._handle_document_save,
            "gate.check": self._handle_gate_check,
            "verification.plan": self._handle_verification_plan,
            "evidence.record": self._handle_evidence_record,
            "evidence.finalize": self._handle_evidence_finalize,
        }
        handler = handlers.get(definition.name)
        if handler is None:
            raise OperationError(
                "OPERATION_ADAPTER_UNAVAILABLE",
                f"adapter is not available yet for {definition.name}",
                details={"operation": definition.name},
            )
        return handler(request, definition)

    def _project_key(self, request: dict[str, Any]) -> str:
        value = str(request.get("projectKey") or "").strip()
        if value:
            return value
        config = paths.read_config(self.ade_sdd)
        value = str(config.get("projectKey") or config.get("workspaceKey") or "").strip()
        if not value:
            raise OperationError(
                "PROJECT_KEY_REQUIRED",
                "projectKey is required for document and gate operations",
                remediation="set projectKey in .ae-sdd/config.yaml or request.projectKey",
            )
        return value

    def _story(self, request: dict[str, Any], state_value: Optional[dict[str, Any]] = None) -> str:
        story = str(request.get("story") or "").strip()
        if story:
            return story
        if state_value is not None:
            return str(state.get_active_story(state_value) or "")
        return ""

    def _resolve_project_file(self, value: str) -> Path:
        raw = Path(str(value or ""))
        if not str(value or "").strip():
            raise OperationError("PATH_REQUIRED", "artifact/content path is required")
        candidate = raw if raw.is_absolute() else self.project_dir / raw
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(self.project_dir)
        except (OSError, ValueError) as exc:
            raise OperationError(
                "PATH_OUTSIDE_PROJECT",
                "requested path must be an existing file inside project root",
                details={"path": str(value), "project": str(self.project_dir)},
            ) from exc
        if not resolved.is_file():
            raise OperationError("PATH_NOT_FILE", "requested path is not a file", details={"path": str(resolved)})
        return resolved

    def _handle_document_resolve(self, request: dict[str, Any], definition: OperationDefinition) -> dict[str, Any]:
        work_item = str(request["workItem"])
        story = self._story(request)
        parameters = request["parameters"]
        try:
            resolved = document_storage.resolve_path(
                self.ade_sdd,
                self._project_key(request),
                str(parameters["intent"]),
                story_id=story or None,
                doc_id=str(parameters.get("docId") or "") or None,
                version=parameters.get("version"),
                work_item_id=work_item,
            )
        except document_storage.DocStorageError as exc:
            raise OperationError(exc.code, str(exc), details={"intent": parameters["intent"]}) from exc
        return self._response(
            definition.name, work_item, changed=False,
            revision_before=None, revision_after=None,
            artifacts=[{"path": resolved.full_path, "scope": resolved.scope,
                        "category": resolved.storing_index_update.get("category")}],
        )

    def _handle_document_save(self, request: dict[str, Any], definition: OperationDefinition) -> dict[str, Any]:
        work_item = str(request["workItem"])
        parameters = request["parameters"]
        content_path = self._resolve_project_file(str(parameters["contentFile"]))
        content = content_path.read_text(encoding="utf-8")
        story = self._story(request)
        try:
            resolved = document_storage.resolve_path(
                self.ade_sdd, self._project_key(request), str(parameters["intent"]),
                story_id=story or None, doc_id=str(parameters.get("docId") or "") or None,
                version=parameters.get("version"), work_item_id=work_item,
            )
        except document_storage.DocStorageError as exc:
            raise OperationError(exc.code, str(exc)) from exc
        store = StateStore(self._resolve_state_path(work_item))
        lease_ref = request["lease"]
        expected_revision = int(request["expectedRevision"])
        if request.get("dryRun"):
            snapshot = store.validate_mutation(expected_revision=expected_revision,
                                               lease_id=str(lease_ref["leaseId"]),
                                               fencing_token=int(lease_ref["fencingToken"]))
            return self._response(definition.name, work_item, changed=False,
                                  revision_before=snapshot.revision, revision_after=snapshot.revision + 1,
                                  artifacts=[{"path": resolved.full_path}], extra={"dryRun": True})

        def mutate(_: dict[str, Any]) -> dict[str, Any]:
            result = document_storage.save_doc(
                self.ade_sdd, self._project_key(request), str(parameters["intent"]), content,
                story_id=story or None, doc_id=str(parameters.get("docId") or "") or None,
                version=parameters.get("version"), changelog_note=str(parameters.get("changelogNote") or "") or None,
                work_item_id=work_item,
            )
            if not result.success:
                raise OperationError("DOCUMENT_SAVE_FAILED", result.error or "document save failed")
            return {"path": result.full_path, "newVersion": result.new_version}

        response = store.mutate(expected_revision=expected_revision, lease_id=str(lease_ref["leaseId"]),
                                fencing_token=int(lease_ref["fencingToken"]),
                                idempotency_key=str(request["idempotencyKey"]), operation=definition.name,
                                payload=parameters, mutate=mutate)
        return self._response(definition.name, work_item, changed=response.changed,
                              revision_before=response.revision_before, revision_after=response.revision_after,
                              state_value=response.state, artifacts=[response.result or {}],
                              extra={"dryRun": False, "replayed": response.replayed})

    def _handle_gate_check(self, request: dict[str, Any], definition: OperationDefinition) -> dict[str, Any]:
        work_item = str(request["workItem"])
        parameters = request.get("parameters") or {}
        gate_ids = [str(item) for item in parameters.get("gateIds") or ["G-08", "G-14", "G-CODEPLAN-SRC", "G-12", "G-13"]]
        story = self._story(request)
        results = self._gate_checker(gate_ids, work_item, story)
        return self._response(definition.name, work_item, changed=False,
                              revision_before=None, revision_after=None, gate_results=results,
                              extra={"gateIds": gate_ids})

    def _handle_verification_plan(self, request: dict[str, Any], definition: OperationDefinition) -> dict[str, Any]:
        work_item = str(request["workItem"])
        parameters = request["parameters"]
        story = self._story(request)
        if not story:
            raise OperationError("STORY_REQUIRED", "verification.plan requires story")
        try:
            changed_paths = verification_plan.validate_changed_paths(
                self.project_dir, parameters["changedPaths"]
            )
        except ValueError as exc:
            raw_paths = [str(item) for item in parameters.get("changedPaths") or []]
            path_escape = any(Path(item).is_absolute() or ".." in Path(item).parts for item in raw_paths)
            raise OperationError(
                "PATH_OUTSIDE_PROJECT" if path_escape else "CHANGED_PATH_INVALID",
                "verification changedPaths must be existing files inside project root",
                details={"changedPaths": raw_paths, "error": str(exc)},
            ) from exc
        plan = verification_plan.build_plan(
            self.project_dir, story, changed_paths,
            str(parameters.get("sinceFingerprint") or ""), work_item,
        )
        if not bool(parameters.get("persist", True)):
            return self._response(definition.name, work_item, changed=False,
                                  revision_before=None, revision_after=None,
                                  artifacts=[{"verificationPlan": plan}])
        store = StateStore(self._resolve_state_path(work_item))
        lease_ref = request["lease"]
        expected_revision = int(request["expectedRevision"])
        if request.get("dryRun"):
            snapshot = store.validate_mutation(expected_revision=expected_revision,
                                               lease_id=str(lease_ref["leaseId"]),
                                               fencing_token=int(lease_ref["fencingToken"]))
            projected = copy.deepcopy(snapshot.state)
            projected["verificationPlan"] = plan
            return self._response(definition.name, work_item, changed=False,
                                  revision_before=snapshot.revision, revision_after=snapshot.revision + 1,
                                  artifacts=[{"verificationPlan": plan}], state_value=projected,
                                  extra={"dryRun": True})

        def mutate(state_value: dict[str, Any]) -> dict[str, Any]:
            state_value["verificationPlan"] = plan
            return {"verificationPlan": plan}

        response = store.mutate(expected_revision=expected_revision, lease_id=str(lease_ref["leaseId"]),
                                fencing_token=int(lease_ref["fencingToken"]),
                                idempotency_key=str(request["idempotencyKey"]), operation=definition.name,
                                payload=parameters, mutate=mutate)
        return self._response(definition.name, work_item, changed=response.changed,
                              revision_before=response.revision_before, revision_after=response.revision_after,
                              state_value=response.state, artifacts=[response.result or {}],
                              next_actions=plan.get("nextActions", []),
                              extra={"dryRun": False, "replayed": response.replayed})

    def _handle_evidence_record(self, request: dict[str, Any], definition: OperationDefinition) -> dict[str, Any]:
        work_item = str(request["workItem"])
        parameters = request["parameters"]
        artifact_path = self._resolve_project_file(str(parameters["artifactPath"]))
        story = self._story(request)
        if not story:
            raise OperationError("STORY_REQUIRED", "evidence.record requires story")
        input_fingerprint = str(parameters.get("inputFingerprint") or "")
        if not input_fingerprint:
            raise OperationError("INPUT_FINGERPRINT_REQUIRED", "evidence.record requires inputFingerprint")
        store = StateStore(self._resolve_state_path(work_item))
        lease_ref = request["lease"]
        expected_revision = int(request["expectedRevision"])
        if request.get("dryRun"):
            snapshot = store.validate_mutation(expected_revision=expected_revision,
                                               lease_id=str(lease_ref["leaseId"]),
                                               fencing_token=int(lease_ref["fencingToken"]))
            return self._response(definition.name, work_item, changed=False,
                                  revision_before=snapshot.revision, revision_after=snapshot.revision + 1,
                                  artifacts=[{"path": str(artifact_path)}], extra={"dryRun": True})

        def mutate(_: dict[str, Any]) -> dict[str, Any]:
            entry = evidence.record(
                self.project_dir, story, kind=str(parameters.get("kind") or "test"),
                command=parameters.get("command") or "", input_fingerprint=input_fingerprint,
                toolchain_fingerprint=str(parameters.get("toolchainFingerprint") or "unknown"),
                exit_code=int(parameters.get("exitCode", 0)),
                artifacts=[{"path": str(artifact_path), "sha256": evidence.artifact_hash(artifact_path)}],
                summary=parameters.get("summary") or {}, duration_ms=int(parameters.get("durationMs", 0)),
                logical_key=str(parameters.get("logicalKey") or ""),
            )
            return entry

        response = store.mutate(expected_revision=expected_revision, lease_id=str(lease_ref["leaseId"]),
                                fencing_token=int(lease_ref["fencingToken"]),
                                idempotency_key=str(request["idempotencyKey"]), operation=definition.name,
                                payload=parameters, mutate=mutate)
        return self._response(definition.name, work_item, changed=response.changed,
                              revision_before=response.revision_before, revision_after=response.revision_after,
                              state_value=response.state, artifacts=[response.result or {}],
                              extra={"dryRun": False, "replayed": response.replayed})

    def _handle_evidence_finalize(self, request: dict[str, Any], definition: OperationDefinition) -> dict[str, Any]:
        work_item = str(request["workItem"])
        story = self._story(request)
        if not story:
            raise OperationError("STORY_REQUIRED", "evidence.finalize requires story")
        store = StateStore(self._resolve_state_path(work_item))
        lease_ref = request["lease"]
        expected_revision = int(request["expectedRevision"])
        if request.get("dryRun"):
            snapshot = store.validate_mutation(expected_revision=expected_revision,
                                               lease_id=str(lease_ref["leaseId"]),
                                               fencing_token=int(lease_ref["fencingToken"]))
            return self._response(definition.name, work_item, changed=False,
                                  revision_before=snapshot.revision, revision_after=snapshot.revision + 1,
                                  artifacts=[{"manifest": str(evidence.manifest_path(self.project_dir, story))}],
                                  extra={"dryRun": True})

        def mutate(_: dict[str, Any]) -> dict[str, Any]:
            path, manifest = evidence.finalize_manifest(self.project_dir, story)
            return {"manifest": str(path), "entryCount": len(manifest.get("entries", []))}

        response = store.mutate(expected_revision=expected_revision, lease_id=str(lease_ref["leaseId"]),
                                fencing_token=int(lease_ref["fencingToken"]),
                                idempotency_key=str(request["idempotencyKey"]), operation=definition.name,
                                payload=request.get("parameters") or {}, mutate=mutate)
        return self._response(definition.name, work_item, changed=response.changed,
                              revision_before=response.revision_before, revision_after=response.revision_after,
                              state_value=response.state, artifacts=[response.result or {}],
                              extra={"dryRun": False, "replayed": response.replayed})

    def _validate_legal_transition(
        self, state_value: dict[str, Any], target: str
    ) -> None:
        current = state.get_active_phase(state_value)
        if target == current:
            return
        suggestion = state.next_step_suggestion(state_value)
        if suggestion.get("next") != target:
            raise OperationError(
                "ILLEGAL_STATE_TRANSITION",
                f"transition {current} -> {target} is not the legal next step",
                details={"currentPhase": current, "targetPhase": target, "next": suggestion.get("next")},
            )

    def _transition_gate_results(
        self, target: str, work_item: str, story: str
    ) -> list[dict[str, Any]]:
        if target == "coding":
            return self._gate_checker(
                ["G-08", "G-14", "G-CODEPLAN-SRC"], work_item, story
            )
        if target == "completed":
            return self._gate_checker(["G-12", "G-13"], work_item, story)
        return []

    def _resolve_state_path(self, work_item: str) -> Path:
        if not self.ade_sdd.is_dir():
            raise OperationError(
                "AE_SDD_PROJECT_NOT_INITIALIZED",
                "project does not contain .ae-sdd",
                details={"project": str(self.project_dir)},
            )
        state_path = paths.find_work_item_state_path(self.ade_sdd, work_item)
        if state_path is not None:
            return state_path
        folded = work_item.casefold()
        matches = []
        for item in work_item_context.list_work_item_states(self.ade_sdd):
            identifiers = [item.key, str(item.data.get("stateMachineName") or "")]
            if folded in {identifier.casefold() for identifier in identifiers if identifier}:
                matches.append(item.path)
        if len(matches) == 1:
            return matches[0]
        if len(matches) > 1:
            raise OperationError(
                "WORK_ITEM_AMBIGUOUS",
                "multiple state files match the explicit Work Item",
                details={"workItem": work_item, "candidates": [str(path) for path in matches]},
            )
        raise OperationError(
            "WORK_ITEM_NOT_FOUND",
            "no state file matches the explicit Work Item",
            details={"workItem": work_item},
        )

    def _default_confirmation_checker(self, phase: str, work_item: str, story: str) -> bool:
        for session_path in sorted((self.project_dir / ".auto-engineering").glob("*/session.json")):
            try:
                import json

                data = json.loads(session_path.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                continue
            identifiers = {
                str(data.get("workItemKey") or "").casefold(),
                session_path.parent.name.casefold(),
            }
            features = data.get("features") or {}
            identifiers.update(str(value).casefold() for value in features.values() if isinstance(value, str))
            if work_item.casefold() not in identifiers and story.casefold() not in identifiers:
                continue
            if any(item.get("phase") == phase for item in data.get("userConfirmedPhases") or []):
                return True
        return False

    def _default_gate_checker(
        self, gate_ids: list[str], work_item: str, story: str
    ) -> list[dict[str, Any]]:
        from lib import gates

        state_path = self._resolve_state_path(work_item)
        state_value = StateStore(state_path).read().state
        current_story = story or state.get_active_story(state_value) or ""
        effective = dict(state_value)
        effective["currentStory"] = current_story
        effective["phase"] = state.get_active_phase(state_value)
        project = self.project_dir
        checks = {
            "G-08": gates.check_g08,
            "G-14": gates.check_g14,
            "G-CODEPLAN-SRC": gates.check_g_codeplan_src,
            "G-12": gates.check_g12,
            "G-13": gates.check_g13,
        }
        unknown = sorted(set(gate_ids) - set(checks))
        if unknown:
            raise OperationError(
                "GATE_NOT_REGISTERED",
                "one or more requested gates are not registered for typed execution",
                details={"gateIds": unknown, "registered": sorted(checks)},
            )
        results = []
        for gate_id in gate_ids:
            checker = checks[gate_id]
            result = checker(project, effective, current_story)
            results.append(
                {
                    "gateId": result.gate_id,
                    "name": result.name,
                    "pass": result.pass_,
                    "severity": result.severity,
                    "message": result.message,
                    "action": result.action,
                    "details": result.details,
                }
            )
        return results

    @staticmethod
    def _response(
        operation: str,
        work_item: str,
        *,
        changed: bool,
        revision_before: Optional[int],
        revision_after: Optional[int],
        artifacts: Optional[list[dict[str, Any]]] = None,
        gate_results: Optional[list[dict[str, Any]]] = None,
        next_actions: Optional[list[dict[str, Any]]] = None,
        state_value: Optional[dict[str, Any]] = None,
        extra: Optional[dict[str, Any]] = None,
    ) -> dict[str, Any]:
        response: dict[str, Any] = {
            "ok": True,
            "changed": changed,
            "operation": operation,
            "workItem": work_item,
            "revisionBefore": revision_before,
            "revisionAfter": revision_after,
            "artifacts": artifacts or [],
            "gateResults": gate_results or [],
            "nextActions": next_actions or [],
            "error": None,
        }
        if state_value is not None:
            response["state"] = state_value
        if extra:
            response.update(extra)
        return response
