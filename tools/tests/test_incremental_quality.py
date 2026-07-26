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
    snapshot = project / entry["artifacts"][0]["snapshotPath"]
    snapshot.write_text("tampered", encoding="utf-8")
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


def test_evidence_record_keeps_immutable_snapshot_and_supersedes_same_logical_key():
    project = Path(tempfile.mkdtemp())
    artifact = project / "result.json"
    artifact.write_text("first", encoding="utf-8")
    first = evidence.record(
        project, "STORY-001", kind="test", command="mvn test",
        input_fingerprint="i1", toolchain_fingerprint="java8", exit_code=0,
        artifacts=[{"path": "result.json", "sha256": evidence.artifact_hash(artifact)}],
        logical_key="test:mvn-test",
    )
    artifact.write_text("second", encoding="utf-8")
    second = evidence.record(
        project, "STORY-001", kind="test", command="mvn test",
        input_fingerprint="i2", toolchain_fingerprint="java8", exit_code=0,
        artifacts=[{"path": "result.json", "sha256": evidence.artifact_hash(artifact)}],
        logical_key="test:mvn-test",
    )

    manifest = evidence.load_manifest(project, "STORY-001")
    assert manifest["entries"][0]["status"] == "superseded"
    assert manifest["entries"][1]["status"] == "active"
    assert manifest["entries"][0]["artifacts"][0]["snapshotPath"] != manifest["entries"][1]["artifacts"][0]["snapshotPath"]
    snapshot = project / manifest["entries"][1]["artifacts"][0]["snapshotPath"]
    assert snapshot.read_text(encoding="utf-8") == "second"
    assert evidence.finalize_manifest(project, "STORY-001")[1]["entries"][1]["evidenceId"] == second["evidenceId"]
    assert first["evidenceId"] != second["evidenceId"]


def test_evidence_finalize_validates_snapshot_after_source_changes():
    project = Path(tempfile.mkdtemp())
    artifact = project / "result.xml"
    artifact.write_text("stable", encoding="utf-8")
    evidence.record(
        project, "STORY-001", kind="test", command="mvn test",
        input_fingerprint="i1", toolchain_fingerprint="java8", exit_code=0,
        artifacts=[{"path": "result.xml", "sha256": evidence.artifact_hash(artifact)}],
    )
    artifact.write_text("changed-after-record", encoding="utf-8")
    path, manifest = evidence.finalize_manifest(project, "STORY-001")
    assert path.is_file()
    assert manifest["entries"][0]["artifacts"][0]["path"] == "result.xml"


def test_legacy_evidence_manifest_remains_readable_without_rewrite_until_finalize():
    project = Path(tempfile.mkdtemp())
    artifact = project / "legacy.txt"
    artifact.write_text("legacy", encoding="utf-8")
    manifest_path = evidence.manifest_path(project, "STORY-001")
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    legacy = {"schemaVersion": 1, "storyId": "STORY-001", "entries": [{
        "evidenceId": "ev-legacy", "kind": "test", "commandHash": evidence.command_hash("mvn test"),
        "inputFingerprint": "i1", "toolchainFingerprint": "java8", "exitCode": 0,
        "reusable": True, "artifacts": [{"path": "legacy.txt", "sha256": evidence.artifact_hash(artifact)}],
    }]}
    manifest_path.write_text(json.dumps(legacy), encoding="utf-8")
    assert evidence.load_manifest(project, "STORY-001")["entries"][0].get("status") is None
    evidence.finalize_manifest(project, "STORY-001")
    assert evidence.load_manifest(project, "STORY-001")["entries"][0].get("status") is None


def test_verification_plan_does_not_schedule_maven_for_docs_only():
    plan = verification_plan.build_plan(Path.cwd(), "STORY-001", ["ae-sdd-doc/Story/STORY-001.md"])
    assert plan["changeClass"] == ["documentation"]
    assert "Maven/full-story-regression" in plan["notRequired"]


def test_verification_plan_exposes_evidence_input_and_next_action():
    project = Path(tempfile.mkdtemp())
    changed = project / "src" / "A.java"
    changed.parent.mkdir(parents=True, exist_ok=True)
    changed.write_text("class A {}", encoding="utf-8")
    plan = verification_plan.build_plan(project, "STORY-001", ["src/A.java"], work_item="WI-001")
    assert plan["evidenceInputFingerprint"] == plan["inputFingerprint"]
    assert plan["nextActions"]
    assert plan["nextActions"][0]["inputFingerprint"] == plan["evidenceInputFingerprint"]


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


# --- Append-only evidence ledger parity with the Rust contract ---------------

GOLDEN_EVENT_DIGEST = "f413824b8d196be69690e917fe65ef5291e6dc49d68dea750cd306172b355e56"


def _golden_event():
    return evidence.make_event(
        1,
        "ev-golden-00000001",
        "recorded",
        "tests/golden",
        "1" * 64,
        [{
            "kind": "evidence-entry",
            "path": ".auto-engineering/STORY-GOLDEN/evidence/entries/entry.json",
            "digest": "2" * 64,
            "byteLength": 128,
        }],
        None,
    )


def test_evidence_ledger_golden_event_digest_matches_the_rust_contract():
    event = _golden_event()
    assert event["eventDigest"] == GOLDEN_EVENT_DIGEST
    line = evidence.canonical_event_line(event)
    assert line.startswith('{"artifactRefs":[{"byteLength":128,')
    assert '"previousEventDigest":null' in line
    parsed = evidence.parse_ledger(line + "\n")
    assert parsed == [event]


def test_evidence_ledger_chain_links_events_and_detects_tampering():
    first = _golden_event()
    second = evidence.make_event(2, "ev-second", "superseded", "tests/golden", "3" * 64, [],
                                 first["eventDigest"])
    third = evidence.make_event(3, "ev-third", "recorded", "tests/golden", "3" * 64, [],
                                second["eventDigest"])
    assert [event["sequence"] for event in evidence.verify_ledger([first, second, third])] == [1, 2, 3]

    tampered_digest = dict(second, logicalKey="tests/rewritten")
    try:
        evidence.verify_ledger([first, tampered_digest, third])
        assert False, "a rewritten historical event must fail verification"
    except ValueError:
        pass
    broken_link = dict(third, previousEventDigest="0" * 64)
    try:
        evidence.verify_ledger([first, second, broken_link])
        assert False, "a broken chain link must fail verification"
    except ValueError:
        pass
    try:
        evidence.verify_ledger([second])
        assert False, "a non-contiguous sequence must fail verification"
    except ValueError:
        pass
    try:
        evidence.verify_ledger([dict(first, previousEventDigest="0" * 64)])
        assert False, "a genesis event must not reference a previous digest"
    except ValueError:
        pass
    try:
        evidence.parse_ledger(evidence.canonical_event_line(first) + " \n")
        assert False, "a non-canonical ledger line must fail verification"
    except ValueError:
        pass


def test_evidence_ledger_projection_supersede_finalize_and_rebuild():
    first_entry = {
        "evidenceId": "ev-1", "kind": "test", "logicalKey": "tests/core",
        "inputFingerprint": "1" * 64, "exitCode": 0, "reusable": True, "artifacts": [],
    }
    second_entry = dict(first_entry, evidenceId="ev-3", inputFingerprint="3" * 64)
    recorded = evidence.make_event(1, "ev-1", "recorded", "tests/core", "1" * 64, [], None)
    superseded = evidence.make_event(2, "ev-2", "superseded", "tests/core", "3" * 64, [],
                                     recorded["eventDigest"])
    replacement = evidence.make_event(3, "ev-3", "recorded", "tests/core", "3" * 64, [],
                                      superseded["eventDigest"])
    finalized = evidence.make_event(4, "ev-4", "finalized", "", "4" * 64, [],
                                    replacement["eventDigest"])
    events = evidence.verify_ledger([recorded, superseded, replacement, finalized])
    payloads = {"ev-1": first_entry, "ev-3": second_entry}

    entries = evidence.project_entries(events, payloads)
    assert [entry["evidenceId"] for entry in entries] == ["ev-1", "ev-3"]
    assert entries[0]["status"] == "superseded"
    assert entries[0]["supersededBy"] == "ev-3"
    assert entries[1]["status"] == "active"
    assert evidence.project_entries(events, payloads) == entries, "projection is deterministic"

    manifest = evidence.rebuild_manifest("STORY-001", entries)
    assert manifest["contentHash"] == evidence.manifest_content_hash(manifest)
    assert evidence.rebuild_manifest("STORY-001", entries) == manifest, "rebuild is byte-stable"

    dangling = evidence.make_event(1, "ev-x", "superseded", "tests/missing", "5" * 64, [], None)
    try:
        evidence.project_entries([dangling], {})
        assert False, "a supersede without an active entry must fail closed"
    except ValueError:
        pass


def test_evidence_ledger_projection_preserves_legacy_residue_verbatim():
    legacy_entry = {"evidenceId": "ev-legacy", "kind": "test", "inputFingerprint": "i1",
                    "exitCode": 0, "reusable": True, "artifacts": []}
    recorded = evidence.make_event(1, "ev-1", "recorded", "tests/core", "1" * 64, [], None)
    entries = evidence.project_entries([recorded], {"ev-1": {"evidenceId": "ev-1"}},
                                       residue=[legacy_entry])
    assert entries[0] == legacy_entry
    assert "status" not in entries[0]
    assert entries[1]["status"] == "active"
