"""Contracts for real-HTTP, local-then-test-environment acceptance evidence."""
from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


TOOLS_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = TOOLS_DIR.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import evidence, gates, verification_plan  # noqa: E402


STORY_ID = "STORY-HTTP-001"
MASTER_SOURCE = REPO_ROOT / "source"


def _scanner_module():
    path = REPO_ROOT / "scripts" / "test_authenticity_scan.py"
    spec = importlib.util.spec_from_file_location("test_authenticity_scan_http_policy", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _write(project: Path, relative: str, content: str) -> Path:
    path = project / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


def _http_plan(*, boundary: str = "http", stages=None, internal_mocks=False) -> dict:
    return {
        "goal": "Implement HTTP AC",
        "changedPaths": ["src/test/java/example/ApiIT.java"],
        "verification": [{
            "id": "V-HTTP-1",
            "acId": "AC-001",
            "boundary": boundary,
            "stages": ["local", "test-env"] if stages is None else stages,
            "internalMocksAllowed": internal_mocks,
            "command": "pytest-http",
        }],
        "approved": True,
    }


def _record_http(
    project: Path,
    *,
    stage: str,
    input_fingerprint: str,
    build_id: str = "build-123",
    base_url: str | None = None,
    ac_ids=None,
    internal_mocks: bool = False,
    kind: str | None = None,
    logical_key: str | None = None,
) -> dict:
    artifact_label = (logical_key or stage).replace(":", "-")
    artifact = _write(
        project,
        f".auto-engineering/{STORY_ID}/evidence/{artifact_label}.json",
        json.dumps({"stage": stage, "status": "PASS"}),
    )
    return evidence.record(
        project,
        STORY_ID,
        kind=kind or f"http-{stage}",
        command=f"run-http-{stage}",
        input_fingerprint=input_fingerprint,
        toolchain_fingerprint="http-acceptance:v1",
        exit_code=0,
        artifacts=[{"path": artifact.relative_to(project).as_posix()}],
        summary={
            "stage": stage,
            "baseUrl": base_url or (
                "http://127.0.0.1:8080" if stage == "local"
                else "https://test-api.example.internal"
            ),
            "buildId": build_id,
            "acIds": ["AC-001"] if ac_ids is None else ac_ids,
            "internalMocks": internal_mocks,
            "result": "PASS",
        },
        logical_key=logical_key or f"http-{stage}",
    )


def _record_authenticity(project: Path, plan: dict, changed_paths: list[str]) -> None:
    command = "test_authenticity_scan.py"
    toolchain = "test-authenticity:v1"
    report = _write(
        project,
        f".auto-engineering/{STORY_ID}/evidence/g09-report.json",
        json.dumps({
            "status": "PASS",
            "storyId": STORY_ID,
            "scope": sorted(changed_paths),
            "commandHash": evidence.command_hash(command),
            "toolchainFingerprint": toolchain,
        }),
    )
    evidence.record(
        project,
        STORY_ID,
        kind="test-authenticity",
        command=command,
        input_fingerprint=plan["planFingerprint"],
        toolchain_fingerprint=toolchain,
        exit_code=0,
        artifacts=[{"path": report.relative_to(project).as_posix()}],
        summary={
            "gate": "G-09",
            "storyId": STORY_ID,
            "status": "PASS",
            "changedPaths": sorted(changed_paths),
            "scope": sorted(changed_paths),
            "commandHash": evidence.command_hash(command),
            "toolchainFingerprint": toolchain,
            "report": report.relative_to(project).as_posix(),
        },
    )


class TestExecutionPlanHttpContract:
    @pytest.mark.parametrize(
        ("mutate", "expected_issue"),
        [
            (lambda plan: plan["verification"][0].pop("stages"), "stages"),
            (lambda plan: plan["verification"][0].update(stages=["test-env", "local"]), "stages"),
            (lambda plan: plan["verification"][0].update(internalMocksAllowed=True), "internalMocksAllowed"),
            (lambda plan: plan["verification"][0].update(command=""), "command"),
        ],
    )
    def test_g08_rejects_incomplete_http_verification(self, tmp_path: Path, mutate, expected_issue: str):
        state = {"executionPlan": _http_plan()}
        mutate(state["executionPlan"])

        result = gates.check_g08(tmp_path, state, STORY_ID)

        assert not result.pass_
        assert result.details.get("reason") == "http-verification-contract"
        assert expected_issue in json.dumps(result.details.get("issues"), ensure_ascii=False)

    def test_g08_accepts_complete_http_verification(self, tmp_path: Path):
        result = gates.check_g08(tmp_path, {"executionPlan": _http_plan()}, STORY_ID)
        assert result.pass_, result.message

    def test_g14_rejects_interface_ac_downgraded_to_non_http(self, tmp_path: Path):
        _write(
            tmp_path,
            f"design/{STORY_ID}.md",
            (
                f"# {STORY_ID}\n\n"
                "| AC ID | Given | When | Then | 测试层级 | 验证边界 |\n"
                "| --- | --- | --- | --- | --- | --- |\n"
                "| AC-001 | service running | call API | 200 | 接口 | http |\n"
            ),
        )
        state = {"executionPlan": _http_plan(boundary="unit")}

        result = gates.check_g14(tmp_path, state, STORY_ID)

        assert not result.pass_
        assert result.details.get("reason") == "http-ac-boundary-mismatch"
        assert result.details.get("httpAcs") == ["AC-001"]


class TestHttpAuthenticityScanner:
    def test_mockmvc_is_a_blocker(self, tmp_path: Path):
        _write(
            tmp_path,
            "src/test/java/example/ControllerTest.java",
            """
            import org.springframework.test.web.servlet.MockMvc;
            class ControllerTest { MockMvc mvc; }
            """,
        )

        findings, _ = _scanner_module().scan_java_tests(tmp_path)

        assert "mock-http-boundary" in {item.rule for item in findings if item.severity == "BLOCKER"}

    def test_webmvctest_is_a_blocker(self, tmp_path: Path):
        _write(
            tmp_path,
            "src/test/java/example/ControllerTest.java",
            "@WebMvcTest(UserController.class) class ControllerTest {}\n",
        )

        findings, _ = _scanner_module().scan_java_tests(tmp_path)

        assert "mock-http-boundary" in {item.rule for item in findings if item.severity == "BLOCKER"}

    def test_random_port_with_internal_mockbean_is_a_blocker(self, tmp_path: Path):
        _write(
            tmp_path,
            "src/test/java/example/ApiIT.java",
            """
            @SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
            class ApiIT {
              @MockBean private UserService userService;
              @Test void call() { assertEquals(200, 200); }
            }
            """,
        )

        findings, _ = _scanner_module().scan_java_tests(tmp_path)

        assert "http-internal-mock" in {item.rule for item in findings if item.severity == "BLOCKER"}

    @pytest.mark.parametrize(
        "declaration",
        [
            "@Mock private UserService userService;",
            "@Mock private UserServiceImpl userService;",
            "@Mock private UserRepositoryAdapter userRepository;",
            "@Mock private UserMapperImpl userMapper;",
            "@Mock private UserDAO userDao;",
            "@Mock private OrderUseCaseImpl orderUseCase;",
            "@Mock private AdminControllerAdvice controllerAdvice;",
            "@Spy private UserService userService;",
            "private UserService userService = Mockito.mock(UserService.class);",
            "private UserService userService = mock(UserService.class);",
            "private UserService userService = Mockito.spy(new UserService());",
            "private UserService userService = spy(new UserService());",
        ],
    )
    def test_random_port_with_internal_mockito_double_is_a_blocker(
        self, tmp_path: Path, declaration: str
    ):
        _write(
            tmp_path,
            "src/test/java/example/ApiIT.java",
            f"""
            @SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
            class ApiIT {{
              {declaration}
              @Test void call() {{ assertEquals(200, 200); }}
            }}
            """,
        )

        findings, _ = _scanner_module().scan_java_tests(tmp_path)

        assert "http-internal-mock" in {item.rule for item in findings if item.severity == "BLOCKER"}

    def test_real_http_without_internal_mock_passes_http_rules(self, tmp_path: Path):
        _write(
            tmp_path,
            "src/test/java/example/ApiIT.java",
            """
            @SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
            class ApiIT {
              @Autowired TestRestTemplate http;
              @Test void call() { assertEquals(200, http.getForEntity("/health", String.class).getStatusCodeValue()); }
            }
            """,
        )

        findings, _ = _scanner_module().scan_java_tests(tmp_path)
        blocker_rules = {item.rule for item in findings if item.severity == "BLOCKER"}

        assert "mock-http-boundary" not in blocker_rules
        assert "http-internal-mock" not in blocker_rules

    @pytest.mark.parametrize("external_type", ["ExternalPaymentClient", "ExternalPaymentServiceClient"])
    def test_external_stub_is_not_classified_as_internal_mock(
        self, tmp_path: Path, external_type: str
    ):
        _write(
            tmp_path,
            "src/test/java/example/ApiIT.java",
            f"""
            @SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
            class ApiIT {{
              @MockBean private {external_type} externalPaymentClient;
              @Test void call() {{ assertEquals(200, 200); }}
            }}
            """,
        )

        findings, _ = _scanner_module().scan_java_tests(tmp_path)

        assert "http-internal-mock" not in {item.rule for item in findings if item.severity == "BLOCKER"}


class TestHttpAcceptanceEvidence:
    def test_local_only_is_incomplete(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1")

        ok, reason, details = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-missing-stage"
        assert details["missingStages"] == ["test-env"]

    def test_valid_local_then_test_env_pair_passes(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1")
        _record_http(tmp_path, stage="test-env", input_fingerprint="fp-1")

        ok, reason, details = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert ok, (reason, details)
        assert reason == "verified"
        assert details["buildId"] == "build-123"

    def test_build_mismatch_fails(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1", build_id="build-a")
        _record_http(tmp_path, stage="test-env", input_fingerprint="fp-1", build_id="build-b")

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-build-mismatch"

    @pytest.mark.parametrize(
        ("stage", "base_url"),
        [
            ("local", "https://test-api.example.internal"),
            ("test-env", "http://127.0.0.1:8080"),
            ("test-env", "https://user:password@test-api.example.internal"),
            ("test-env", "https://test-api.example.internal?token=secret"),
            ("test-env", "http://0.0.0.0:8080"),
            ("test-env", "http://[::]:8080"),
            ("test-env", "http://169.254.10.20:8080"),
        ],
    )
    def test_stage_url_policy_fails_closed(self, tmp_path: Path, stage: str, base_url: str):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1")
        _record_http(tmp_path, stage="test-env", input_fingerprint="fp-1")
        manifest = evidence.load_manifest(tmp_path, STORY_ID)
        target = next(entry for entry in manifest["entries"] if entry["kind"] == f"http-{stage}")
        target["summary"]["baseUrl"] = base_url
        evidence.save_manifest(tmp_path, STORY_ID, manifest)

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-url"

    def test_internal_mock_attestation_fails(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1", internal_mocks=True)
        _record_http(tmp_path, stage="test-env", input_fingerprint="fp-1")

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-internal-mock"

    def test_missing_ac_coverage_fails(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1", ac_ids=["AC-001"])
        _record_http(tmp_path, stage="test-env", input_fingerprint="fp-1", ac_ids=["AC-001"])

        ok, reason, details = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001", "AC-002"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-missing-ac"
        assert details["candidates"][0]["missingLocalAcs"] == ["AC-002"]
        assert details["candidates"][0]["missingTestEnvAcs"] == ["AC-002"]

    def test_test_environment_must_follow_local_stage(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1")
        _record_http(tmp_path, stage="test-env", input_fingerprint="fp-1")
        manifest = evidence.load_manifest(tmp_path, STORY_ID)
        next(entry for entry in manifest["entries"] if entry["kind"] == "http-local")[
            "startedAt"
        ] = "2026-07-17T10:00:00Z"
        next(entry for entry in manifest["entries"] if entry["kind"] == "http-test-env")[
            "startedAt"
        ] = "2026-07-17T09:00:00Z"
        evidence.save_manifest(tmp_path, STORY_ID, manifest)

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-order"

    def test_multiple_entries_can_aggregate_ac_coverage(self, tmp_path: Path):
        for stage in ("local", "test-env"):
            _record_http(
                tmp_path, stage=stage, input_fingerprint="fp-1", ac_ids=["AC-001"],
                logical_key=f"http-{stage}:ac-1",
            )
            _record_http(
                tmp_path, stage=stage, input_fingerprint="fp-1", ac_ids=["AC-002"],
                logical_key=f"http-{stage}:ac-2",
            )

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001", "AC-002"], "fp-1"
        )

        assert ok, reason

    def test_stale_active_entry_does_not_poison_current_pair(self, tmp_path: Path):
        _record_http(
            tmp_path, stage="local", input_fingerprint="old-fp",
            logical_key="http-local:old",
        )
        _record_http(
            tmp_path, stage="test-env", input_fingerprint="old-fp",
            logical_key="http-test-env:old",
        )
        _record_http(
            tmp_path, stage="local", input_fingerprint="fp-1",
            logical_key="http-local:current",
        )
        _record_http(
            tmp_path, stage="test-env", input_fingerprint="fp-1",
            logical_key="http-test-env:current",
        )

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert ok, reason

    def test_supplemental_evidence_does_not_satisfy_test_env(self, tmp_path: Path):
        _record_http(tmp_path, stage="local", input_fingerprint="fp-1")
        _record_http(
            tmp_path,
            stage="test-env",
            input_fingerprint="fp-1",
            kind="http-external-supplemental",
        )

        ok, reason, _ = evidence.validate_http_acceptance_manifest(
            tmp_path, STORY_ID, ["AC-001"], "fp-1"
        )

        assert not ok
        assert reason == "http-evidence-missing-stage"

    def test_g09_requires_test_environment_evidence_for_http_plan(self, tmp_path: Path):
        changed = "src/test/java/example/ApiIT.java"
        _write(
            tmp_path,
            changed,
            "class ApiIT { @Test void value() { assertEquals(2, 1 + 1); } }\n",
        )
        plan = verification_plan.build_plan(tmp_path, STORY_ID, [changed])
        _record_authenticity(tmp_path, plan, [changed])
        _record_http(tmp_path, stage="local", input_fingerprint=plan["planFingerprint"])
        state = {
            "phase": "test-running",
            "entryNode": "STORY",
            "verificationPlan": plan,
            "executionPlan": _http_plan(),
        }

        result = gates.check_g09(tmp_path, state, STORY_ID, master_source=MASTER_SOURCE)

        assert not result.pass_
        assert result.details.get("evidenceReason") == "http-evidence-missing-stage"

    def test_g09_accepts_complete_local_then_test_environment_evidence(self, tmp_path: Path):
        changed = "src/test/java/example/ApiIT.java"
        _write(
            tmp_path,
            changed,
            "class ApiIT { @Test void value() { assertEquals(2, 1 + 1); } }\n",
        )
        plan = verification_plan.build_plan(tmp_path, STORY_ID, [changed])
        _record_authenticity(tmp_path, plan, [changed])
        _record_http(tmp_path, stage="local", input_fingerprint=plan["planFingerprint"])
        _record_http(tmp_path, stage="test-env", input_fingerprint=plan["planFingerprint"])
        state = {
            "phase": "test-running",
            "entryNode": "STORY",
            "verificationPlan": plan,
            "executionPlan": _http_plan(),
        }

        result = gates.check_g09(tmp_path, state, STORY_ID, master_source=MASTER_SOURCE)

        assert result.pass_, result.message
        assert result.details.get("httpEvidence", {}).get("buildId") == "build-123"


def test_source_contract_rejects_mock_http_acceptance_language():
    strategy = (REPO_ROOT / "source/standards/testing/be-testcase-strategy.md").read_text(encoding="utf-8")
    constraints = (REPO_ROOT / "constraints/testing.md").read_text(encoding="utf-8")
    distributed_constraints = (
        REPO_ROOT / "source/standards/constraints/testing.md"
    ).read_text(encoding="utf-8")

    assert "Service 层用 `@MockBean` 隔离即可" not in strategy
    assert "Service 使用 `@MockBean`" not in constraints
    assert "Service 层用 @MockBean 隔离" not in distributed_constraints
    assert "MockMvc 仅框架过老时降级" not in distributed_constraints
    for text in (strategy, constraints, distributed_constraints):
        assert "test-env" in text
        assert "internalMocksAllowed=false" in text


def test_main_fallback_uses_evidence_and_state_review_instead_of_reports():
    fallback = (REPO_ROOT / "source/skill-fallbacks/SKILL.full.md").read_text(encoding="utf-8")

    assert "| 测试报告 |" not in fallback
    assert "| Coding报告 |" not in fallback
    assert "| CodeReview报告 |" not in fallback
    assert "TEST_REPORT 已生成" not in fallback
    assert "codeReviewReport存在" not in fallback
    assert "state.review.status/findings" in fallback
    assert "immutable evidence" in fallback
