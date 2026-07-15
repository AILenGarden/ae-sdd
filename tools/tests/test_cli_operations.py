from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest


CLI = Path(__file__).resolve().parent.parent / "bin" / "ae-sdd"


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


def run_cli(project: Path, *args: str) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(CLI.parent.parent)
    env["PYTHONIOENCODING"] = "utf-8"
    return subprocess.run(
        [sys.executable, str(CLI), *args],
        cwd=str(project),
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )


def test_ops_describe_is_machine_readable(project: Path) -> None:
    result = run_cli(project, "ops", "describe", "--json")

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["schemaVersion"] == "1"
    assert any(item["name"] == "state.transition" for item in payload["operations"])


def test_ops_next_returns_revision_lease_and_next_actions(project: Path) -> None:
    result = run_cli(
        project,
        "ops",
        "next",
        "--project",
        str(project),
        "--work-item",
        "WORK-001",
        "--story",
        "STORY-001",
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["revision"] == 0
    assert payload["leaseStatus"]["status"] == "absent"
    assert payload["nextActions"][0]["operation"] == "lease.acquire"


def test_ops_execute_request_file_acquires_lease(project: Path) -> None:
    request_path = project / "request.json"
    request_path.write_text(
        json.dumps(
            {
                "schemaVersion": "1",
                "operation": "lease.acquire",
                "project": str(project),
                "workItem": "WORK-001",
                "idempotencyKey": "acquire-a",
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
        ),
        encoding="utf-8",
    )

    result = run_cli(project, "ops", "execute", "--request-file", str(request_path), "--json")

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["ok"] is True
    assert payload["operation"] == "lease.acquire"
    assert payload["lease"]["fencingToken"] == 1


def test_ops_execute_failure_still_emits_one_json_object(project: Path) -> None:
    request_path = project / "request.json"
    request_path.write_text(
        json.dumps(
            {
                "schemaVersion": "1",
                "operation": "state.patch",
                "project": str(project),
                "workItem": "WORK-001",
                "parameters": {},
            }
        ),
        encoding="utf-8",
    )

    result = run_cli(project, "ops", "execute", "--request-file", str(request_path), "--json")

    assert result.returncode == 1
    payload = json.loads(result.stdout)
    assert payload["ok"] is False
    assert payload["error"]["code"] == "OPERATION_NOT_REGISTERED"
    assert result.stdout.count("{") >= 1
    assert result.stdout.strip().startswith("{")
    assert result.stdout.strip().endswith("}")


def test_lease_convenience_commands_share_typed_protocol(project: Path) -> None:
    acquired = run_cli(
        project,
        "lease",
        "acquire",
        "--project",
        str(project),
        "--work-item",
        "WORK-001",
        "--agent-id",
        "A",
        "--session-id",
        "session-A",
        "--host",
        "test-host",
        "--pid",
        "1",
        "--ttl-seconds",
        "300",
        "--idempotency-key",
        "acquire-a",
        "--json",
    )
    assert acquired.returncode == 0, acquired.stderr
    lease = json.loads(acquired.stdout)["lease"]

    status = run_cli(
        project,
        "lease",
        "status",
        "--project",
        str(project),
        "--work-item",
        "WORK-001",
        "--json",
    )
    assert status.returncode == 0, status.stderr
    assert json.loads(status.stdout)["leaseStatus"]["leaseId"] == lease["leaseId"]
