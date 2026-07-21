from __future__ import annotations

import json
import sys
from pathlib import Path


TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import evidence, gates, scenario_derivation  # noqa: E402


def model() -> dict:
    return {
        "interfaceId": "job-api",
        "acIds": ["AC-001"],
        "capabilities": ["async", "idempotent"],
        "action": {"method": "POST", "path": "/jobs"},
        "states": {"before": ["absent"], "after": ["queued", "done"],
                   "forbidden": [{"status": "REGRESSED"}]},
        "observations": [{"id": "job-status", "kind": "public-query",
                          "independentOfAction": True}],
        "dimensions": {"changed": ["status"], "invariants": ["jobId"]},
        "relations": [{"kind": "monotonic", "path": "status"}],
        "perturbations": {"time": ["timeout"], "replay": ["same-key"],
                          "order": ["duplicate-event"], "dependency-failure": ["worker-down"],
                          "concurrency": ["parallel-submit"]},
        "repeatability": {"command": "run-scenario job-api", "isolation": "namespace",
                          "cleanup": "delete namespace"},
    }


def state(manifest_path: str | None) -> dict:
    item = {
        "id": "V-HTTP-1", "acId": "AC-001", "boundary": "http",
        "stages": ["local", "test-env"], "internalMocksAllowed": False,
        "command": "run-http",
    }
    if manifest_path is not None:
        item["scenarioManifest"] = manifest_path
    return {"executionPlan": {"goal": "x", "changedPaths": ["x"],
                              "verification": [item], "approved": True,
                              "scenarioPolicyVersion": 1}}


def write_manifest(project: Path) -> str:
    relative = "test-scenarios/job-api.json"
    path = project / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(scenario_derivation.build_manifest(model())), encoding="utf-8")
    return relative


def test_g_http_1_accepts_derived_manifest(tmp_path: Path):
    result = gates.check_g_http_1(tmp_path, state(write_manifest(tmp_path)), "STORY-1")
    assert result.pass_, result.message
    assert result.details["validated"][0]["scenarioCount"] == 2


def test_g_http_1_rejects_missing_manifest(tmp_path: Path):
    result = gates.check_g_http_1(tmp_path, state(None), "STORY-1")
    assert not result.pass_
    assert result.details["failures"][0]["reason"] == "scenario-manifest-path"


def test_g_http_1_rejects_shallow_manifest(tmp_path: Path):
    relative = write_manifest(tmp_path)
    path = tmp_path / relative
    manifest = json.loads(path.read_text(encoding="utf-8"))
    manifest["scenarios"][0]["assertions"] = {"status": 200}
    path.write_text(json.dumps(manifest), encoding="utf-8")
    result = gates.check_g_http_1(tmp_path, state(relative), "STORY-1")
    assert not result.pass_
    issues = result.details["failures"][0]["issues"]
    assert "status-only" in {item["code"] for item in issues}


def test_g_http_1_rejects_path_escape(tmp_path: Path):
    result = gates.check_g_http_1(tmp_path, state("../outside.json"), "STORY-1")
    assert not result.pass_
    assert result.details["failures"][0]["reason"] == "scenario-manifest-path"


def test_legacy_plan_remains_readable(tmp_path: Path):
    legacy = state(None)
    legacy["executionPlan"].pop("scenarioPolicyVersion")
    result = gates.check_g_http_1(tmp_path, legacy, "STORY-1")
    assert result.pass_
    assert result.details["profile"] == "legacy"


def test_g08_invokes_scenario_gate_for_new_policy(tmp_path: Path):
    result = gates.check_g08(tmp_path, state(None), "STORY-1")
    assert not result.pass_
    assert result.gate_id == "G-HTTP-1"


def record_stage(project: Path, stage: str, assertion_kinds: list[str]):
    artifact = project / f"{stage}.json"
    artifact.write_text(json.dumps({"result": "PASS"}), encoding="utf-8")
    return evidence.record(
        project, "STORY-1", kind=f"http-{stage}", command=f"run-{stage}",
        input_fingerprint="fp-1", toolchain_fingerprint="http:v1", exit_code=0,
        artifacts=[{"path": artifact.name}],
        summary={
            "stage": stage,
            "baseUrl": "http://127.0.0.1:8080" if stage == "local" else "https://test.example.com",
            "buildId": "build-1", "acIds": ["AC-001"], "internalMocks": False,
            "result": "PASS",
            "scenarioResults": [{
                "scenarioId": "job-api:async", "result": "PASS",
                "assertionKinds": assertion_kinds,
                "rerunCommand": "run-scenario job-api:async",
            }],
        },
        logical_key=f"http-{stage}",
    )


def test_http_evidence_requires_executed_scenario_depth(tmp_path: Path):
    record_stage(tmp_path, "local", ["state", "invariant"])
    record_stage(tmp_path, "test-env", ["state", "invariant"])
    ok, reason, details = evidence.validate_http_acceptance_manifest(
        tmp_path, "STORY-1", ["AC-001"], "fp-1", ["job-api:async"]
    )
    assert ok, (reason, details)


def test_http_evidence_rejects_status_only_scenario_result(tmp_path: Path):
    record_stage(tmp_path, "local", ["status"])
    record_stage(tmp_path, "test-env", ["status"])
    ok, reason, _ = evidence.validate_http_acceptance_manifest(
        tmp_path, "STORY-1", ["AC-001"], "fp-1", ["job-api:async"]
    )
    assert not ok
    assert reason == "http-evidence-scenario-depth"
