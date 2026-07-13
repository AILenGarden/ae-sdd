import json
import sys
import tempfile
from datetime import datetime, timedelta, timezone
from pathlib import Path

TOOLS_DIR = Path(__file__).resolve().parent.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from lib import baseline, document_storage, evidence, verification_plan, work_item_context


def _finding(rule="AP-1", path="src/A.java", severity="BLOCKER"):
    return {"rule": rule, "path": path, "line": 10, "severity": severity, "message": "x"}


def test_baseline_requires_approval_and_detects_delta():
    project = Path(tempfile.mkdtemp())
    try:
        try:
            baseline.create(project, "G-CODE-1", [_finding()], created_by="u",
                            scanner_version="1", ruleset_fingerprint="r", project_fingerprint="p")
            assert False, "approval should be required"
        except PermissionError:
            pass
        payload = baseline.create(project, "G-CODE-1", [_finding()], created_by="u",
                                  scanner_version="1", ruleset_fingerprint="r", project_fingerprint="p",
                                  require_user_approval=True)
        loaded, error = baseline.load(project)
        assert error is None and loaded["contentHash"] == payload["contentHash"]
        result = baseline.compare(loaded, [_finding(), _finding("AP-2", "src/B.java")], ruleset_fingerprint="r")
        assert result["status"] == "BLOCK_NEW_FINDINGS"
        assert len(result["new"]) == 1
    finally:
        pass


def test_baseline_touched_debt_is_not_silently_inherited():
    project = Path(tempfile.mkdtemp())
    payload = baseline.create(
        project, "G-CODE-1", [_finding()], created_by="u", scanner_version="1",
        ruleset_fingerprint="r", project_fingerprint="p", require_user_approval=True,
    )
    result = baseline.compare(payload, [_finding()], ruleset_fingerprint="r", touched_paths=["src/A.java"])
    assert result["status"] == "BLOCK_TOUCHED_DEBT"
    assert len(result["touchedDebt"]) == 1


def test_evidence_cache_requires_matching_input_command_toolchain_and_artifact():
    project = Path(tempfile.mkdtemp())
    artifact = project / "result.xml"
    artifact.write_text("ok", encoding="utf-8")
    entry = evidence.record(
        project, "STORY-001", kind="test", command="mvn test",
        input_fingerprint="i1", toolchain_fingerprint="java8", exit_code=0,
        artifacts=[{"path": str(artifact), "sha256": evidence.artifact_hash(artifact)}],
    )
    assert evidence.find_reusable(project, "STORY-001", input_fingerprint="i1",
                                  command="mvn test", toolchain_fingerprint="java8")["evidenceId"] == entry["evidenceId"]
    assert evidence.find_reusable(project, "STORY-001", input_fingerprint="i2",
                                  command="mvn test", toolchain_fingerprint="java8") is None
    artifact.write_text("tampered", encoding="utf-8")
    assert evidence.find_reusable(project, "STORY-001", input_fingerprint="i1",
                                  command="mvn test", toolchain_fingerprint="java8") is None


def test_evidence_freshness_window_expires():
    project = Path(tempfile.mkdtemp())
    artifact = project / "result.xml"
    artifact.write_text("ok", encoding="utf-8")
    entry = evidence.record(
        project, "STORY-001", kind="test", command="mvn test",
        input_fingerprint="i1", toolchain_fingerprint="java8", exit_code=0,
        artifacts=[{"path": str(artifact), "sha256": evidence.artifact_hash(artifact)}],
        freshness_window_seconds=1,
    )
    entry["startedAt"] = (datetime.now(timezone.utc) - timedelta(seconds=5)).strftime("%Y-%m-%dT%H:%M:%SZ")
    assert evidence.is_reusable(entry, input_fingerprint="i1", command="mvn test", toolchain_fingerprint="java8") is False


def test_verification_plan_does_not_schedule_maven_for_docs_only():
    plan = verification_plan.build_plan(Path.cwd(), "STORY-001", ["ae-sdd-doc/Story/STORY-001.md"])
    assert plan["changeClass"] == ["documentation"]
    assert "Maven/full-story-regression" in plan["notRequired"]


def test_document_alias_is_pointer_registry_not_duplicate_body():
    ade_sdd = Path(tempfile.mkdtemp()) / ".ae-sdd"
    ade_sdd.mkdir(parents=True)
    canonical = ade_sdd.parent / "ae-sdd-doc" / "Story" / "STORY-001.md"
    canonical.parent.mkdir(parents=True)
    canonical.write_text("# canonical\n", encoding="utf-8")
    alias = ade_sdd.parent / "legacy-story.md"
    alias.write_text("See canonical: ae-sdd-doc/Story/STORY-001.md\n", encoding="utf-8")
    document_storage.register_alias(ade_sdd, str(alias), str(canonical))
    assert document_storage.resolve_alias(ade_sdd, str(alias)) == canonical
    document_storage.assert_no_duplicate_canonical(ade_sdd, str(alias), str(canonical))


def test_document_candidates_are_ambiguous_instead_of_mtime_selected():
    root = Path(tempfile.mkdtemp())
    first, second = root / "a.md", root / "b.md"
    first.write_text("a", encoding="utf-8")
    second.write_text("b", encoding="utf-8")
    try:
        document_storage.resolve_candidates([first, second])
        assert False, "ambiguity must be explicit"
    except document_storage.DocStorageError as exc:
        assert exc.code == "E013"


def test_artifact_invalidation_uses_input_fingerprint_edges():
    plan = work_item_context.invalidation_plan(
        [{"fingerprint": "sha256:story-new"}],
        [
            {"artifact": "CodingPlan", "inputArtifacts": [{"fingerprint": "sha256:story-new"}]},
            {"artifact": "MavenEvidence", "inputArtifacts": [{"fingerprint": "sha256:implementation"}]},
        ],
    )
    assert [item["artifact"] for item in plan["invalidated"]] == ["CodingPlan"]
    assert [item["artifact"] for item in plan["retained"]] == ["MavenEvidence"]
