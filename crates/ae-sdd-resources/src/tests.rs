use std::fs;

use ae_sdd_contracts::{DocumentTxnId, SchemaVersion};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, InputFingerprint, InventoryGeneration, ProjectKey,
    ProjectRelativePath, WorkItemId,
};

use crate::{
    AssetsPath, DeterministicResourceResolver, DocumentFinalizeRequest, DocumentPlanner,
    DocumentSaveRequest, ResolutionDisposition, ResourceCandidate, ResourceIntent, ResourceLayer,
    ResourcePort, ResourceResolveError, ResourceResolveRequest,
};

fn artifact(kind: &str, path: &str, bytes: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new(kind).expect("valid artifact kind"),
        ProjectRelativePath::new(path).expect("valid fixture path"),
        ArtifactDigest::digest(bytes),
        bytes.len() as u64,
    )
}

fn resolve_request(
    candidates: Vec<ResourceCandidate>,
    override_authorized: bool,
) -> ResourceResolveRequest {
    ResourceResolveRequest::new(
        SchemaVersion::V1,
        ProjectKey::new("ae-sdd").expect("valid project key"),
        ArtifactKind::new("story").expect("valid resource kind"),
        ResourceIntent::Read,
        candidates,
        override_authorized,
        InventoryGeneration::new(7),
    )
    .expect("valid resolution request")
}

#[test]
fn resolver_canonicalizes_priority_trace_and_replay_digest() {
    let declared = ResourceCandidate::new(
        ResourceLayer::DeclaredOverride,
        artifact("story", "docs/override.story.md", b"override"),
    );
    let canonical = ResourceCandidate::new(
        ResourceLayer::Canonical,
        artifact(
            "story",
            "ae-sdd-doc/Story/STORY-AE-SDD-RESOURCE-CONTEXT-002.md",
            b"canonical",
        ),
    );
    let legacy = ResourceCandidate::new(
        ResourceLayer::Legacy,
        artifact("story", "docs/legacy/STORY-002.md", b"legacy"),
    );
    let resolver = DeterministicResourceResolver;

    let first = resolver
        .resolve(&resolve_request(
            vec![legacy.clone(), canonical.clone(), declared.clone()],
            true,
        ))
        .expect("declared override wins");
    let replay = resolver
        .resolve(&resolve_request(vec![canonical, declared, legacy], true))
        .expect("candidate order does not change resolution");

    assert_eq!(first.winner().path().as_str(), "docs/override.story.md");
    assert_eq!(first.source_layer(), ResourceLayer::DeclaredOverride);
    assert_eq!(first.resolution_digest(), replay.resolution_digest());
    assert_eq!(first.trace(), replay.trace());
    assert_eq!(
        first
            .trace()
            .iter()
            .map(|entry| (entry.layer(), entry.disposition()))
            .collect::<Vec<_>>(),
        vec![
            (
                ResourceLayer::DeclaredOverride,
                ResolutionDisposition::Winner
            ),
            (
                ResourceLayer::Canonical,
                ResolutionDisposition::LowerPriority
            ),
            (ResourceLayer::Legacy, ResolutionDisposition::LowerPriority),
        ]
    );
}

#[test]
fn resolver_rejects_undeclared_override_and_kind_conflict() {
    let override_candidate = ResourceCandidate::new(
        ResourceLayer::DeclaredOverride,
        artifact("story", "docs/override.story.md", b"override"),
    );
    assert_eq!(
        ResourceResolveRequest::new(
            SchemaVersion::V1,
            ProjectKey::new("ae-sdd").expect("valid project key"),
            ArtifactKind::new("story").expect("valid resource kind"),
            ResourceIntent::Read,
            vec![override_candidate],
            false,
            InventoryGeneration::new(7),
        ),
        Err(ResourceResolveError::OverrideNotAuthorized)
    );

    let wrong_kind = ResourceCandidate::new(
        ResourceLayer::Canonical,
        artifact("design-review", "ae-sdd-doc/DR/DR-002.md", b"dr"),
    );
    assert_eq!(
        ResourceResolveRequest::new(
            SchemaVersion::V1,
            ProjectKey::new("ae-sdd").expect("valid project key"),
            ArtifactKind::new("story").expect("valid resource kind"),
            ResourceIntent::Read,
            vec![wrong_kind],
            false,
            InventoryGeneration::new(7),
        ),
        Err(ResourceResolveError::CandidateKindMismatch)
    );
}

#[test]
fn document_planner_is_deterministic_and_does_not_write_project_files() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let target_on_disk = workspace.path().join("story.md");
    fs::write(&target_on_disk, b"before").expect("seed target");
    let target = ProjectRelativePath::new("story.md").expect("valid target");
    let staged = artifact("document-content", ".ae-sdd/staging/story.md", b"after");
    let request = DocumentSaveRequest::new(
        DocumentTxnId::new("txn.story.save.1").expect("valid transaction ID"),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid Work Item"),
        target,
        staged,
        Some(ArtifactDigest::digest(b"before")),
        InputFingerprint::digest(b"save inputs"),
    )
    .expect("valid save request");

    let first = DocumentPlanner::save(&request).expect("save plan");
    let replay = DocumentPlanner::save(&request).expect("replayed save plan");

    assert_eq!(first, replay);
    assert_eq!(first.plan_digest(), replay.plan_digest());
    assert_eq!(fs::read(target_on_disk).expect("read target"), b"before");
    assert_eq!(first.operations().len(), 1);
    assert_eq!(
        first.operations()[0].expected_before_digest(),
        Some(ArtifactDigest::digest(b"before"))
    );
}

#[test]
fn document_finalize_plan_binds_expected_digest() {
    let expected = ArtifactDigest::digest(b"final");
    let request = DocumentFinalizeRequest::new(
        DocumentTxnId::new("txn.story.finalize.1").expect("valid transaction ID"),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid Work Item"),
        ProjectRelativePath::new("ae-sdd-doc/Story/STORY-002.md").expect("valid target"),
        expected,
        InputFingerprint::digest(b"finalize inputs"),
    );

    let plan = DocumentPlanner::finalize(&request).expect("finalize plan");

    assert_eq!(
        plan.operations()[0].expected_before_digest(),
        Some(expected)
    );
}

#[test]
fn assets_path_is_canonical_and_legacy_layout_fixture_is_typed() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/resources/document-layout.v1.json"
    ))
    .expect("valid resource fixture");
    let project = ProjectKey::new(fixture["projectKey"].as_str().expect("fixture project key"))
        .expect("typed project key");
    let canonical = AssetsPath::for_project(&project).expect("canonical assets path");

    assert_eq!(
        canonical.as_path().as_str(),
        ".ae-sdd/assets/ae-sdd.assets.md"
    );
    assert_eq!(
        fixture["documents"][0]["legacyCandidates"][0]
            .as_str()
            .and_then(|value| ProjectRelativePath::new(value).ok())
            .expect("typed legacy path")
            .as_str(),
        "docs/STORY-RESOURCE-CONTEXT-002.md"
    );
}
