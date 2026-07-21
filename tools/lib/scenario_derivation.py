"""Derive effective test scenarios from interface capabilities and failure mechanisms."""
from __future__ import annotations

from copy import deepcopy
from typing import Any


CAPABILITY_RULES: dict[str, dict[str, Any]] = {
    "command": {
        "kind": "state-observation",
        "detects": ["lost-or-corrupt-state", "unexpected-side-effect"],
        "perturbations": ["field", "identity", "replay", "dependency-failure"],
    },
    "query": {
        "kind": "relation-observation",
        "detects": ["incorrect-selection-projection-or-ordering"],
        "perturbations": ["field", "identity", "order", "boundary"],
    },
    "state-machine": {
        "kind": "transition-observation",
        "detects": ["illegal-or-missing-state-transition"],
        "perturbations": ["order", "replay", "concurrency", "time"],
    },
    "batch": {
        "kind": "aggregate-observation",
        "detects": ["partial-failure-or-conservation-violation"],
        "perturbations": ["field", "boundary", "dependency-failure", "order"],
    },
    "async": {
        "kind": "eventual-observation",
        "detects": ["stuck-regressing-or-duplicated-async-work"],
        "perturbations": ["time", "replay", "order", "dependency-failure"],
    },
    "file": {
        "kind": "content-observation",
        "detects": ["content-integrity-or-metadata-loss"],
        "perturbations": ["boundary", "field", "replay"],
    },
    "auth": {
        "kind": "isolation-observation",
        "detects": ["authorization-or-tenant-isolation-bypass"],
        "perturbations": ["identity"],
    },
    "idempotent": {
        "kind": "replay-observation",
        "detects": ["duplicate-effect-or-idempotency-key-conflict"],
        "perturbations": ["replay", "time", "concurrency"],
    },
    "concurrent": {
        "kind": "interleaving-observation",
        "detects": ["lost-update-or-invalid-interleaving"],
        "perturbations": ["concurrency", "order", "time"],
    },
}


def _strings(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item).strip() for item in value if str(item).strip()]


def validate_capability_model(model: dict[str, Any]) -> list[dict[str, str]]:
    """Return structured issues instead of accepting a ceremonial model."""
    issues: list[dict[str, str]] = []
    if not isinstance(model, dict):
        return [{"path": "$", "code": "model-type", "message": "model must be an object"}]
    if not str(model.get("interfaceId") or "").strip():
        issues.append({"path": "interfaceId", "code": "required", "message": "interfaceId is required"})
    capabilities = _strings(model.get("capabilities"))
    if not capabilities:
        issues.append({"path": "capabilities", "code": "required", "message": "at least one capability is required"})
    for capability in capabilities:
        if capability not in CAPABILITY_RULES:
            issues.append({"path": "capabilities", "code": "unknown-capability", "message": capability})
    if not _strings(model.get("acIds")):
        issues.append({"path": "acIds", "code": "required", "message": "at least one AC is required"})
    if not isinstance(model.get("action"), dict) or not model.get("action"):
        issues.append({"path": "action", "code": "required", "message": "action contract is required"})
    observations = model.get("observations")
    independent = [item for item in observations or [] if isinstance(item, dict)
                   and str(item.get("id") or "").strip()
                   and item.get("independentOfAction") is True]
    if not independent:
        issues.append({"path": "observations", "code": "independent-observation-required",
                       "message": "declare an observation independent of the action path"})
    observation_ids = [str(item.get("id") or "").strip() for item in observations or []
                       if isinstance(item, dict) and str(item.get("id") or "").strip()]
    if len(observation_ids) != len(set(observation_ids)):
        issues.append({"path": "observations", "code": "duplicate-id",
                       "message": "observation ids must be unique"})
    dimensions = model.get("dimensions") if isinstance(model.get("dimensions"), dict) else {}
    relations = model.get("relations") if isinstance(model.get("relations"), list) else []
    if not (_strings(dimensions.get("changed")) or _strings(dimensions.get("invariants")) or relations):
        issues.append({"path": "dimensions", "code": "oracle-required",
                       "message": "declare changed dimensions, invariants, or cross-view relations"})
    repeatability = model.get("repeatability") if isinstance(model.get("repeatability"), dict) else {}
    for field in ("command", "isolation", "cleanup"):
        if not str(repeatability.get(field) or "").strip():
            issues.append({"path": f"repeatability.{field}", "code": "required",
                           "message": f"repeatability {field} is required"})
    axes = model.get("perturbations") if isinstance(model.get("perturbations"), dict) else {}
    if not any(_strings(values) for values in axes.values()):
        issues.append({"path": "perturbations", "code": "perturbation-required",
                       "message": "declare at least one meaningful perturbation"})
    for capability in capabilities:
        rule = CAPABILITY_RULES.get(capability)
        if rule and not any(_strings(axes.get(axis)) for axis in rule["perturbations"]):
            issues.append({"path": f"capabilities.{capability}",
                           "code": "applicable-perturbation-required",
                           "message": f"declare a perturbation applicable to {capability}"})
    return issues


def derive_scenarios(model: dict[str, Any]) -> list[dict[str, Any]]:
    """Derive one minimal scenario per independent capability failure mechanism."""
    issues = validate_capability_model(model)
    if issues:
        raise ValueError(issues)
    interface_id = str(model["interfaceId"]).strip()
    ac_ids = _strings(model.get("acIds"))
    observations = [deepcopy(item) for item in model["observations"]
                    if isinstance(item, dict) and item.get("independentOfAction") is True]
    dimensions = model.get("dimensions") or {}
    axes = model.get("perturbations") or {}
    states = model.get("states") if isinstance(model.get("states"), dict) else {}
    relations = deepcopy(model.get("relations") or [])
    custom_mechanisms = model.get("failureMechanisms") or {}
    scenarios: list[dict[str, Any]] = []
    mechanism_scenarios: dict[tuple[str, ...], dict[str, Any]] = {}

    for capability in dict.fromkeys(_strings(model["capabilities"])):
        rule = CAPABILITY_RULES[capability]
        detects = _strings(custom_mechanisms.get(capability)) or list(rule["detects"])
        signature = tuple(sorted(detects))
        selected_axes = [axis for axis in rule["perturbations"] if _strings(axes.get(axis))]
        existing = mechanism_scenarios.get(signature)
        if existing is not None:
            existing["capabilities"].append(capability)
            existing["perturbationAxes"] = list(dict.fromkeys(
                [*existing["perturbationAxes"], *selected_axes]
            ))
            existing["rationale"] = (
                f"Capabilities {', '.join(existing['capabilities'])} share the failure mechanism "
                + ", ".join(detects)
            )
            continue
        scenario = {
            "scenarioId": f"{interface_id}:{capability}",
            "capability": capability,
            "capabilities": [capability],
            "kind": rule["kind"],
            "acIds": ac_ids,
            "given": deepcopy(states.get("before") or []),
            "when": deepcopy(model.get("action") or {}),
            "observe": [item["id"] for item in observations],
            "assertions": {
                "changedDimensions": _strings(dimensions.get("changed")),
                "invariants": _strings(dimensions.get("invariants")),
                "forbiddenStates": deepcopy(states.get("forbidden") or []),
                "relations": relations,
            },
            "perturbationAxes": selected_axes,
            "detects": detects,
            "rationale": (
                f"Capability '{capability}' requires {rule['kind']} to detect "
                + ", ".join(detects)
            ),
            "repeatability": deepcopy(model["repeatability"]),
            "internalMocks": False,
        }
        scenarios.append(scenario)
        mechanism_scenarios[signature] = scenario
    return scenarios


def build_manifest(model: dict[str, Any]) -> dict[str, Any]:
    """Build a traceable manifest; callers may serialize it as JSON or YAML."""
    return {
        "schemaVersion": 1,
        "interfaceId": str(model.get("interfaceId") or "").strip(),
        "derivation": deepcopy(model),
        "scenarios": derive_scenarios(model),
    }
