"""Validate derived scenario manifests and evaluate observable state changes."""
from __future__ import annotations

from typing import Any

from lib.scenario_derivation import CAPABILITY_RULES, validate_capability_model


def _nonempty(value: Any) -> bool:
    return isinstance(value, list) and any(str(item).strip() for item in value)


def validate_manifest(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    issues: list[dict[str, Any]] = []
    if not isinstance(manifest, dict):
        return [{"path": "$", "code": "manifest-type"}]
    derivation = manifest.get("derivation")
    model_issues = validate_capability_model(derivation)
    issues.extend({"path": f"derivation.{item['path']}", **{k: v for k, v in item.items() if k != "path"}}
                  for item in model_issues)
    if model_issues:
        return issues
    scenarios = manifest.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        return issues + [{"path": "scenarios", "code": "required"}]

    capabilities = set(derivation["capabilities"])
    required_acs = set(derivation["acIds"])
    observations = {str(item.get("id")): item for item in derivation["observations"]
                    if isinstance(item, dict)}
    covered: set[str] = set()
    covered_acs: set[str] = set()
    mechanism_owners: dict[tuple[str, ...], str] = {}
    scenario_ids: set[str] = set()
    for index, scenario in enumerate(scenarios):
        path = f"scenarios[{index}]"
        if not isinstance(scenario, dict):
            issues.append({"path": path, "code": "scenario-type"})
            continue
        scenario_id = str(scenario.get("scenarioId") or "").strip()
        if not scenario_id:
            issues.append({"path": f"{path}.scenarioId", "code": "required"})
        elif scenario_id in scenario_ids:
            issues.append({"path": f"{path}.scenarioId", "code": "duplicate-id"})
        scenario_ids.add(scenario_id)
        primary_capability = str(scenario.get("capability") or "").strip()
        scenario_capabilities = scenario.get("capabilities")
        if not isinstance(scenario_capabilities, list) or not scenario_capabilities:
            scenario_capabilities = [primary_capability]
        scenario_capabilities = [str(item).strip() for item in scenario_capabilities
                                 if str(item).strip()]
        primary_matches = bool(scenario_capabilities) and primary_capability == scenario_capabilities[0]
        if not primary_matches:
            issues.append({"path": f"{path}.capabilities", "code": "primary-capability-mismatch"})
        if primary_capability not in capabilities:
            issues.append({"path": f"{path}.capability", "code": "irrelevant-capability"})
        irrelevant = sorted(set(scenario_capabilities) - capabilities)
        if irrelevant:
            issues.append({"path": f"{path}.capabilities", "code": "irrelevant-capability",
                           "capabilities": irrelevant})
        if primary_matches:
            covered.update(set(scenario_capabilities) & capabilities)
        scenario_acs = {str(item).strip() for item in scenario.get("acIds") or []
                        if str(item).strip()}
        if not scenario_acs:
            issues.append({"path": f"{path}.acIds", "code": "required"})
        unknown_acs = sorted(scenario_acs - required_acs)
        if unknown_acs:
            issues.append({"path": f"{path}.acIds", "code": "unknown-ac", "acIds": unknown_acs})
        covered_acs.update(scenario_acs & required_acs)
        if not str(scenario.get("rationale") or "").strip():
            issues.append({"path": f"{path}.rationale", "code": "derivation-rationale-required"})
        detects = sorted(set(str(item).strip() for item in scenario.get("detects") or [] if str(item).strip()))
        if not detects:
            issues.append({"path": f"{path}.detects", "code": "failure-mechanism-required"})
        else:
            signature = tuple(detects)
            owner = mechanism_owners.get(signature)
            if owner is not None:
                issues.append({"path": f"{path}.detects", "code": "duplicate-failure-mechanism",
                               "otherScenario": owner})
            mechanism_owners[signature] = str(scenario.get("scenarioId") or path)
        observed = scenario.get("observe")
        if (not isinstance(observed, list) or not observed
                or any(not isinstance(item, str) or item not in observations
                       or observations[item].get("independentOfAction") is not True
                       for item in observed)):
            issues.append({"path": f"{path}.observe", "code": "independent-observation-required"})
        assertions = scenario.get("assertions") if isinstance(scenario.get("assertions"), dict) else {}
        meaningful = any((
            _nonempty(assertions.get("changedDimensions")),
            _nonempty(assertions.get("invariants")),
            bool(assertions.get("forbiddenStates")),
            bool(assertions.get("relations")),
        ))
        if not meaningful:
            issues.append({"path": f"{path}.assertions", "code": "status-only"})
        if scenario.get("internalMocks") is not False:
            issues.append({"path": f"{path}.internalMocks", "code": "internal-mock-not-primary-evidence"})
        perturbation_axes = scenario.get("perturbationAxes")
        allowed_axes = {
            axis for capability in scenario_capabilities
            for axis in CAPABILITY_RULES.get(capability, {}).get("perturbations", [])
        }
        declared_axes = derivation.get("perturbations") or {}
        if (not isinstance(perturbation_axes, list) or not perturbation_axes
                or any(axis not in allowed_axes or not _nonempty(declared_axes.get(axis))
                       for axis in perturbation_axes)):
            issues.append({"path": f"{path}.perturbationAxes",
                           "code": "applicable-perturbation-required"})
        repeatability = scenario.get("repeatability") if isinstance(scenario.get("repeatability"), dict) else {}
        for field in ("command", "isolation", "cleanup"):
            if not str(repeatability.get(field) or "").strip():
                issues.append({"path": f"{path}.repeatability.{field}", "code": "required"})
    for capability in sorted(capabilities - covered):
        issues.append({"path": "scenarios", "code": "capability-uncovered", "capability": capability})
    for ac_id in sorted(required_acs - covered_acs):
        issues.append({"path": "scenarios", "code": "ac-uncovered", "acId": ac_id})
    return issues


def evaluate_observation(before: dict[str, Any], after: dict[str, Any],
                         expectation: dict[str, Any]) -> list[dict[str, Any]]:
    """Produce useful field/state findings instead of a single boolean."""
    findings: list[dict[str, Any]] = []
    for field, expected in (expectation.get("changedDimensions") or {}).items():
        actual = after.get(field)
        if actual != expected:
            findings.append({"code": "changed-dimension-mismatch", "field": field,
                             "expected": expected, "actual": actual})
    for field in expectation.get("invariants") or []:
        if before.get(field) != after.get(field):
            findings.append({"code": "invariant-violated", "field": field,
                             "expected": before.get(field), "actual": after.get(field)})
    for forbidden in expectation.get("forbiddenStates") or []:
        if isinstance(forbidden, dict) and all(after.get(key) == value for key, value in forbidden.items()):
            findings.append({"code": "forbidden-state-reached", "state": forbidden})
    return findings
