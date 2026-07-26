"""
test_resume_approved_plan.py — resume-approved-plan Python parity oracle。

Plan P0 Task 12 / Story STORY-AE-SDD-SLICE-SUPERVISOR-001 AC-006：Python oracle
只在 migration profile 下生成与 Rust CLI（bins/ae-sdd-cli resume-approved-plan）
相同的 request/response shape；不读取 Story/constraints/state/source 文件，不在
Python typed registry 注册执行入口，永不成为 canary/sole-writer fallback。
cursor 值绑定 tests/fixtures/execution-efficiency golden fixture。
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from lib import operations

TOOLS_ROOT = Path(__file__).resolve().parent.parent
REPO_ROOT = TOOLS_ROOT.parent
CLI = TOOLS_ROOT / "bin" / "ae-sdd"
FIXTURE_DIR = REPO_ROOT / "tests" / "fixtures" / "execution-efficiency" / "approved-resume"

# daemon `execution.resume` data payload 的冻结响应键
# （crates/ae-sdd-integrations/src/business.rs execution_resume_response）
RESPONSE_SHAPE = [
    "projectionKind",
    "contextRevision",
    "capsuleDigest",
    "capsule",
    "nextAction",
    "authorityRefreshCount",
]


@pytest.fixture
def cursor() -> dict:
    """golden fixture 绑定的已知 resume cursor。"""
    context = json.loads((FIXTURE_DIR / "context.json").read_text(encoding="utf-8"))
    state = json.loads((FIXTURE_DIR / "state.json").read_text(encoding="utf-8"))
    return {
        "knownCapsuleDigest": state["executionRuntime"]["capsuleDigest"],
        "knownContextRevision": context["contextRevision"],
    }


@pytest.fixture
def resume_request(cursor: dict) -> dict:
    return {
        "workspaceId": "11111111-1111-4111-8111-111111111111",
        "agentId": "kimi:instance",
        "sessionId": "22222222-2222-4222-8222-222222222222",
        "workItemId": "PRD-AE-SDD-EXECUTION-EFFICIENCY-001",
        **cursor,
    }


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


def test_builder_assembles_operation_execute_envelope_with_cursor_passthrough(
    resume_request: dict, cursor: dict
) -> None:
    envelope = operations.build_execution_resume_request(resume_request)

    assert envelope == {
        "operation": "execution.resume",
        "dryRun": False,
        "payload": cursor,
    }


def test_builder_without_cursor_sends_empty_payload_object(resume_request: dict) -> None:
    request = {k: v for k, v in resume_request.items() if not k.startswith("known")}

    envelope = operations.build_execution_resume_request(request)

    assert envelope["payload"] == {}
    assert envelope["operation"] == "execution.resume"
    assert envelope["dryRun"] is False


def test_builder_rejects_drift_before_daemon_contact(resume_request: dict) -> None:
    with pytest.raises(operations.OperationError) as unknown:
        operations.build_execution_resume_request({**resume_request, "storyPath": "x.md"})
    assert unknown.value.code == "OPERATION_SCHEMA_INVALID"

    with pytest.raises(operations.OperationError):
        operations.build_execution_resume_request(
            {**resume_request, "knownContextRevision": "4"}
        )

    with pytest.raises(operations.OperationError):
        operations.build_execution_resume_request(
            {**resume_request, "knownContextRevision": -1}
        )

    for field in ("workspaceId", "agentId", "sessionId", "workItemId"):
        request = {k: v for k, v in resume_request.items() if k != field}
        with pytest.raises(operations.OperationError) as missing:
            operations.build_execution_resume_request(request)
        assert missing.value.code == "OPERATION_SCHEMA_INVALID", field


def test_response_shape_matches_frozen_daemon_projection() -> None:
    assert operations.execution_resume_response_shape() == RESPONSE_SHAPE


def test_python_registry_cannot_execute_execution_resume(tmp_path: Path) -> None:
    """oracle 不是 fallback：Python typed registry 没有 execution.resume 执行入口。"""
    registry = operations.OperationRegistry(tmp_path)

    with pytest.raises(operations.OperationError) as exc:
        registry.execute(
            {
                "schemaVersion": operations.SCHEMA_VERSION,
                "operation": "execution.resume",
                "project": str(tmp_path),
                "workItem": "WORK-001",
                "parameters": {},
            }
        )

    assert exc.value.code == "OPERATION_NOT_REGISTERED"


def test_cli_emits_migration_oracle_shape(
    tmp_path: Path, resume_request: dict, cursor: dict
) -> None:
    result = run_cli(
        tmp_path,
        "resume-approved-plan",
        "--request",
        json.dumps(resume_request),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert set(payload) == {"schemaVersion", "oracle", "operationExecute", "responseShape"}
    assert payload["schemaVersion"] == operations.SCHEMA_VERSION
    assert payload["oracle"] == "migration"
    assert payload["operationExecute"] == {
        "operation": "execution.resume",
        "dryRun": False,
        "payload": cursor,
    }
    assert payload["responseShape"] == RESPONSE_SHAPE


def test_cli_rejects_malformed_and_drifting_requests(
    tmp_path: Path, resume_request: dict
) -> None:
    bad_json = run_cli(tmp_path, "resume-approved-plan", "--request", "not json", "--json")
    assert bad_json.returncode == 1
    assert json.loads(bad_json.stdout)["error"]["code"] == "OPERATION_REQUEST_INVALID_JSON"

    drifted = run_cli(
        tmp_path,
        "resume-approved-plan",
        "--request",
        json.dumps({**resume_request, "storyPath": "ae-sdd-doc/Story/x.md"}),
        "--json",
    )
    assert drifted.returncode == 1
    assert json.loads(drifted.stdout)["error"]["code"] == "OPERATION_SCHEMA_INVALID"

    missing = dict(resume_request)
    del missing["sessionId"]
    incomplete = run_cli(
        tmp_path, "resume-approved-plan", "--request", json.dumps(missing), "--json"
    )
    assert incomplete.returncode == 1
    assert json.loads(incomplete.stdout)["error"]["code"] == "OPERATION_SCHEMA_INVALID"


def test_oracle_never_reads_or_writes_project_files(
    tmp_path: Path, resume_request: dict
) -> None:
    """migration profile 只读 oracle：项目树在调用前后逐字节一致。"""
    before = sorted(str(path) for path in tmp_path.rglob("*"))

    result = run_cli(
        tmp_path,
        "resume-approved-plan",
        "--request",
        json.dumps(resume_request),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    after = sorted(str(path) for path in tmp_path.rglob("*"))
    assert before == after, "oracle 不得在项目内创建、读取依赖或修改任何文件"
