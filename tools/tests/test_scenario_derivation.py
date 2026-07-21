from __future__ import annotations

import sys
from pathlib import Path

import pytest


TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import http_scenario, scenario_derivation  # noqa: E402


def model(*capabilities: str) -> dict:
    return {
        "interfaceId": "orders-api",
        "acIds": ["AC-001"],
        "capabilities": list(capabilities),
        "action": {"method": "POST", "path": "/orders"},
        "states": {"before": ["absent"], "after": ["created"], "forbidden": [{"status": "PARTIAL"}]},
        "observations": [{"id": "order-detail", "kind": "public-query", "independentOfAction": True}],
        "dimensions": {"changed": ["status", "amount"], "invariants": ["tenantId"]},
        "relations": [{"kind": "equal", "left": "detail.id", "right": "command.id"}],
        "perturbations": {
            "field": ["null", "boundary"], "identity": ["other-tenant"],
            "replay": ["same-key"], "dependency-failure": ["timeout"],
            "order": ["reverse"], "boundary": ["empty"], "time": ["timeout"],
            "concurrency": ["same-version"],
        },
        "repeatability": {"command": "run-scenario orders-api", "isolation": "namespace",
                          "cleanup": "delete namespace"},
    }


@pytest.mark.parametrize("capability", sorted(scenario_derivation.CAPABILITY_RULES))
def test_derives_only_the_contract_capability(capability: str):
    scenarios = scenario_derivation.derive_scenarios(model(capability))
    assert [item["capability"] for item in scenarios] == [capability]
    assert scenarios[0]["detects"]
    assert scenarios[0]["rationale"]


def test_query_does_not_receive_irrelevant_command_or_update_cases():
    scenarios = scenario_derivation.derive_scenarios(model("query"))
    assert len(scenarios) == 1
    assert scenarios[0]["kind"] == "relation-observation"
    assert "command" not in scenarios[0]["capability"]
    assert "update" not in scenarios[0]["scenarioId"]


def test_manifest_is_valid_when_every_scenario_has_a_reason_and_oracle():
    manifest = scenario_derivation.build_manifest(model("command", "auth", "idempotent"))
    assert http_scenario.validate_manifest(manifest) == []


def test_rejects_status_only_scenario():
    manifest = scenario_derivation.build_manifest(model("command"))
    manifest["scenarios"][0]["assertions"] = {"status": 200}
    assert "status-only" in {item["code"] for item in http_scenario.validate_manifest(manifest)}


def test_rejects_same_path_self_proof():
    manifest = scenario_derivation.build_manifest(model("command"))
    manifest["derivation"]["observations"][0]["independentOfAction"] = False
    codes = {item["code"] for item in http_scenario.validate_manifest(manifest)}
    assert "independent-observation-required" in codes


def test_rejects_copied_irrelevant_capability():
    manifest = scenario_derivation.build_manifest(model("query"))
    manifest["scenarios"][0]["capability"] = "command"
    codes = {item["code"] for item in http_scenario.validate_manifest(manifest)}
    assert "irrelevant-capability" in codes
    assert "capability-uncovered" in codes


def test_rejects_duplicate_failure_mechanisms():
    source = model("command", "query")
    source["failureMechanisms"] = {"command": ["same-defect"], "query": ["same-defect"]}
    manifest = {"schemaVersion": 1, "interfaceId": "orders-api", "derivation": source,
                "scenarios": []}
    first = scenario_derivation.derive_scenarios({**source, "failureMechanisms": {"command": ["same-defect"], "query": ["other-defect"]}})
    first[1]["detects"] = ["same-defect"]
    manifest["scenarios"] = first
    assert "duplicate-failure-mechanism" in {
        item["code"] for item in http_scenario.validate_manifest(manifest)
    }


def test_merges_capabilities_that_share_one_failure_mechanism():
    source = model("command", "query")
    source["failureMechanisms"] = {"command": ["shared-defect"], "query": ["shared-defect"]}
    manifest = scenario_derivation.build_manifest(source)
    assert len(manifest["scenarios"]) == 1
    assert manifest["scenarios"][0]["capabilities"] == ["command", "query"]
    assert http_scenario.validate_manifest(manifest) == []


def test_rejects_perturbations_irrelevant_to_the_declared_capability():
    source = model("auth")
    source["perturbations"] = {"field": ["blank"]}
    codes = {item["code"] for item in scenario_derivation.validate_capability_model(source)}
    assert "applicable-perturbation-required" in codes


def test_manifest_rejects_duplicate_ids_and_malformed_observation_references():
    manifest = scenario_derivation.build_manifest(model("command", "query"))
    manifest["scenarios"][1]["scenarioId"] = manifest["scenarios"][0]["scenarioId"]
    manifest["scenarios"][0]["observe"] = [{"id": "not-a-string"}]
    codes = {item["code"] for item in http_scenario.validate_manifest(manifest)}
    assert "duplicate-id" in codes
    assert "independent-observation-required" in codes


def test_model_requires_repeatability_and_perturbation_reasoning():
    source = model("async")
    source["repeatability"]["cleanup"] = ""
    source["perturbations"] = {}
    codes = {item["code"] for item in scenario_derivation.validate_capability_model(source)}
    assert codes == {"required", "perturbation-required", "applicable-perturbation-required"}
