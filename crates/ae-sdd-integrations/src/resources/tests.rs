use std::fs;

use ae_sdd_contracts::{DocumentTxnId, SchemaVersion};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, InputFingerprint, InventoryGeneration, ProjectKey,
    ProjectRelativePath, WorkItemId,
};
use ae_sdd_resources::{
    AssetsPath, AssetsPort, AssetsQueryRequest, AssetsReadRequest, DocumentPort,
    DocumentReadRequest, DocumentSaveRequest, ResourceCandidate, ResourceIntent, ResourceLayer,
    ResourceResolveRequest,
};

use super::{FilesystemDocumentPort, FilesystemResourceError, parse_project_path};

fn artifact(kind: &str, path: &str, bytes: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new(kind).expect("valid artifact kind"),
        ProjectRelativePath::new(path).expect("valid path"),
        ArtifactDigest::digest(bytes),
        bytes.len() as u64,
    )
}

fn resolution(path: &str, bytes: &[u8]) -> ResourceResolveRequest {
    ResourceResolveRequest::new(
        SchemaVersion::V1,
        ProjectKey::new("ae-sdd").expect("valid project key"),
        ArtifactKind::new("story").expect("valid artifact kind"),
        ResourceIntent::Read,
        vec![ResourceCandidate::new(
            ResourceLayer::Canonical,
            artifact("story", path, bytes),
        )],
        false,
        InventoryGeneration::new(3),
    )
    .expect("valid resolution")
}

#[test]
fn resources_document_adapter_reads_bounded_content_and_checks_digest() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    fs::create_dir_all(workspace.path().join("ae-sdd-doc/Story")).expect("create docs");
    fs::write(
        workspace.path().join("ae-sdd-doc/Story/STORY-002.md"),
        b"story body",
    )
    .expect("write story");
    let port = FilesystemDocumentPort::new(workspace.path()).expect("contained project root");
    let resolved = port
        .resolve(&resolution("ae-sdd-doc/Story/STORY-002.md", b"story body"))
        .expect("resolve document");

    let document = port
        .read(
            &DocumentReadRequest::new(
                resolved.reference().clone(),
                Some(ArtifactDigest::digest(b"story body")),
                64,
            )
            .expect("bounded request"),
        )
        .expect("bounded read");

    assert_eq!(document.bytes(), b"story body");
    assert_eq!(document.reference(), resolved.reference());
}

#[test]
fn resources_document_adapter_resolve_checks_content_digest_not_only_length() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    fs::create_dir_all(workspace.path().join("ae-sdd-doc/Story")).expect("create docs");
    fs::write(
        workspace.path().join("ae-sdd-doc/Story/STORY-002.md"),
        b"story body",
    )
    .expect("write story");
    let port = FilesystemDocumentPort::new(workspace.path()).expect("contained project root");

    assert_eq!(
        port.resolve(&resolution("ae-sdd-doc/Story/STORY-002.md", b"story b0dy",)),
        Err(FilesystemResourceError::DigestMismatch)
    );
}

#[test]
fn resources_document_adapter_rejects_lexical_windows_escape_forms() {
    for value in [
        "../outside",
        "C:drive-relative",
        "C:/absolute",
        "//server/share/file",
        r"\\?\C:\device\file",
        "docs/file.md:secret",
        "docs/CON.txt",
        "docs/trailing. ",
    ] {
        assert_eq!(
            parse_project_path(value),
            Err(FilesystemResourceError::InvalidProjectPath),
            "accepted {value:?}"
        );
    }
}

#[test]
fn resources_document_adapter_returns_plan_without_mutating_target() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    fs::create_dir_all(workspace.path().join(".ae-sdd/staging")).expect("create staging");
    fs::write(workspace.path().join("target.md"), b"before").expect("seed target");
    fs::write(workspace.path().join(".ae-sdd/staging/target.md"), b"after")
        .expect("seed staged content");
    let port = FilesystemDocumentPort::new(workspace.path()).expect("contained project root");
    let request = DocumentSaveRequest::new(
        DocumentTxnId::new("txn.target.save.1").expect("valid transaction ID"),
        WorkItemId::new("PRD-AE-SDD-RUST-DAEMON-001").expect("valid Work Item"),
        ProjectRelativePath::new("target.md").expect("valid target"),
        artifact("document-content", ".ae-sdd/staging/target.md", b"after"),
        Some(ArtifactDigest::digest(b"before")),
        InputFingerprint::digest(b"save"),
    )
    .expect("valid save request");

    let plan = port.save(&request).expect("save plan");

    assert_eq!(plan.operations().len(), 1);
    assert_eq!(
        fs::read(workspace.path().join("target.md")).unwrap(),
        b"before"
    );
}

#[test]
fn resources_document_adapter_rejects_symlink_or_reparse_escape() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let outside = tempfile::tempdir().expect("outside directory");
    fs::write(outside.path().join("secret.md"), b"secret").expect("outside file");
    let link = workspace.path().join("escape.md");

    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(outside.path().join("secret.md"), &link);
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(outside.path().join("secret.md"), &link);

    if let Err(error) = linked {
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
            ) || error.raw_os_error() == Some(1314),
            "unexpected symlink setup failure: {error}"
        );
        assert!(super::metadata_flags_are_reparse_or_symlink(0x400, false));
        return;
    }

    let port = FilesystemDocumentPort::new(workspace.path()).expect("contained project root");
    let result = port.resolve(&resolution("escape.md", b"secret"));
    assert_eq!(result, Err(FilesystemResourceError::ContainmentDenied));
}

#[test]
fn resources_assets_adapter_is_canonical_bounded_and_content_addressed() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let assets_dir = workspace.path().join(".ae-sdd/assets");
    fs::create_dir_all(&assets_dir).expect("create assets");
    fs::write(
        assets_dir.join("ae-sdd.assets.md"),
        "---\nschemaVersion: ae-sdd-project-assets/v1\nprojectKey: ae-sdd\n---\n## §A Outline\nservice\n## §B Modules\nmodule\n## §C Fields\nfield\n## §D Components\ncomponent\n## §E API\napi\n## §F Keywords\nkeyword\n## §G Read API\nread\n",
    )
    .expect("write assets");

    let port = super::FilesystemAssetsPort::new(workspace.path()).expect("contained root");
    let path = AssetsPath::for_project(&ProjectKey::new("ae-sdd").unwrap()).unwrap();
    let document = port
        .read(&AssetsReadRequest::new(path.clone(), Some("A"), 1024).unwrap())
        .expect("bounded section read");
    assert!(document.content().contains("service"));
    assert_eq!(document.reference().path(), path.as_path());
    assert!(port.check(&path).unwrap().is_valid());
    let query = port
        .query(&AssetsQueryRequest::new(path, "service", 4, 512).unwrap())
        .expect("bounded query");
    assert_eq!(query.matches().len(), 1);
}

#[test]
fn resources_assets_adapter_never_returns_a_split_utf8_byte() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let assets_dir = workspace.path().join(".ae-sdd/assets");
    fs::create_dir_all(&assets_dir).expect("create assets");
    fs::write(
        assets_dir.join("ae-sdd.assets.md"),
        "---\nschemaVersion: ae-sdd-project-assets/v1\nprojectKey: ae-sdd\n---\n## §A Asset Outline\néclair\n",
    )
    .expect("write assets");

    let port = super::FilesystemAssetsPort::new(workspace.path()).expect("contained root");
    let path = AssetsPath::for_project(&ProjectKey::new("ae-sdd").unwrap()).unwrap();
    let document = port
        .read(&AssetsReadRequest::new(path, Some("A"), 22).unwrap())
        .expect("bounded section read");

    assert!(document.truncated());
    assert!(document.content().len() <= 22);
    assert!(!document.content().contains('�'));
}
