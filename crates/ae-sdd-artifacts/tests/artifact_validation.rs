use std::fs;

use ae_sdd_artifacts::{
    ArtifactStoreError, ArtifactValidator, FsArtifactStore, InMemoryArtifactStore, WorkspaceRoot,
};
use ae_sdd_domain::{ArtifactDigest, ArtifactKind, ArtifactRef, ProjectRelativePath};

fn reference(path: &str, bytes: &[u8]) -> ArtifactRef {
    ArtifactRef::new(
        ArtifactKind::new("story").expect("kind is valid"),
        ProjectRelativePath::new(path).expect("path is valid"),
        ArtifactDigest::digest(bytes),
        u64::try_from(bytes.len()).expect("fixture length fits u64"),
    )
}

#[test]
fn valid_artifact_requires_scope_length_and_digest() {
    let store = InMemoryArtifactStore::default();
    let bytes = b"bounded child result";
    let artifact = reference("ae-sdd-doc/Story/STORY-1.md", bytes);
    store.insert(artifact.path().clone(), bytes.to_vec());
    let validator = ArtifactValidator::new(store, 1024);

    let validated = validator
        .validate(
            &artifact,
            &[ProjectRelativePath::new("ae-sdd-doc/Story").expect("scope is valid")],
        )
        .expect("matching artifact validates");

    assert_eq!(validated.bytes(), bytes);
}

#[test]
fn out_of_scope_and_digest_mismatch_fail_closed() {
    let store = InMemoryArtifactStore::default();
    let bytes = b"content";
    let artifact = reference("private/result.json", bytes);
    store.insert(artifact.path().clone(), bytes.to_vec());
    let validator = ArtifactValidator::new(store, 1024);

    assert_eq!(
        validator
            .validate(
                &artifact,
                &[ProjectRelativePath::new("public").expect("scope is valid")]
            )
            .expect_err("scope escape is rejected"),
        ArtifactStoreError::OutsideGrant
    );

    let wrong = ArtifactRef::new(
        artifact.kind().clone(),
        artifact.path().clone(),
        ArtifactDigest::digest(b"different"),
        artifact.byte_length(),
    );
    assert!(matches!(
        validator.validate(
            &wrong,
            &[ProjectRelativePath::new("private").expect("scope is valid")]
        ),
        Err(ArtifactStoreError::DigestMismatch { .. })
    ));
}

#[test]
fn filesystem_adapter_reads_only_canonical_workspace_files() {
    let temp = tempfile::tempdir().expect("temp directory is created");
    fs::create_dir_all(temp.path().join("docs")).expect("docs directory is created");
    fs::write(temp.path().join("docs/story.md"), b"story").expect("fixture is written");
    let root = WorkspaceRoot::open(temp.path()).expect("workspace root opens");
    let store = FsArtifactStore::new(root);
    let artifact = reference("docs/story.md", b"story");

    let validated = ArtifactValidator::new(store, 1024)
        .validate(
            &artifact,
            &[ProjectRelativePath::new("docs").expect("scope is valid")],
        )
        .expect("filesystem artifact validates");

    assert_eq!(validated.bytes(), b"story");
}

#[cfg(unix)]
#[test]
fn filesystem_adapter_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = tempfile::tempdir().expect("workspace is created");
    let outside = tempfile::tempdir().expect("outside directory is created");
    fs::write(outside.path().join("secret"), b"secret").expect("secret fixture is written");
    symlink(outside.path(), workspace.path().join("escaped")).expect("symlink is created");
    let store =
        FsArtifactStore::new(WorkspaceRoot::open(workspace.path()).expect("workspace root opens"));
    let artifact = reference("escaped/secret", b"secret");

    assert_eq!(
        ArtifactValidator::new(store, 1024)
            .validate(
                &artifact,
                &[ProjectRelativePath::new("escaped").expect("scope is valid")]
            )
            .expect_err("symlink escape is rejected"),
        ArtifactStoreError::OutsideWorkspace
    );
}
