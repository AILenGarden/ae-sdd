use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::{
    CompileInput, DistributeInput, ExecutionMode, JobError, JobExecution, JobInput,
    NativeJobRequest, OfflineCommand, OfflineError, OfflineRequest, OfflineResult,
    execute_native_job, execute_offline,
};

/// Typed input for the native post-commit compile/verify/distribute chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCommitRequest {
    /// Repository root containing the source package.
    pub repository_root: PathBuf,
    /// Declarative source package to compile.
    pub source_directory: PathBuf,
    /// Native package output directory.
    pub package_directory: PathBuf,
    /// Explicit distribution targets.
    pub target_directories: Vec<PathBuf>,
    /// Explicit containment roots for every source and target.
    pub allowed_roots: Vec<PathBuf>,
    /// Full Git commit identity used for idempotency.
    pub commit_id: String,
}

/// Result of the native post-commit chain.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostCommitExecution {
    /// Compile transaction and receipt.
    pub compile: JobExecution,
    /// Read-only package verification result.
    pub verification: OfflineResult,
    /// Distribution transaction and receipt.
    pub distribute: JobExecution,
}

/// Executes compile, verification, and distribution without shell-generated JSON.
pub fn execute_post_commit(
    request: &PostCommitRequest,
) -> Result<PostCommitExecution, PostCommitError> {
    validate(request)?;
    let compile = execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "post-commit.compile".to_owned(),
        actor: "git:post-commit".to_owned(),
        reason: "compile declarative package after a functional commit".to_owned(),
        idempotency_key: format!("post-commit-{}-compile", request.commit_id),
        mode: ExecutionMode::Apply,
        allowed_roots: request.allowed_roots.clone(),
        job: JobInput::Compile(CompileInput {
            source_directory: request.source_directory.clone(),
            output_directory: request.package_directory.clone(),
            generated_configs: Vec::new(),
        }),
    })?;
    let verification = execute_offline(&OfflineRequest {
        schema_version: "ae-sdd-offline-build/v1".to_owned(),
        mode: ExecutionMode::DryRun,
        actor: "git:post-commit".to_owned(),
        reason: "verify the compiled package before distribution".to_owned(),
        idempotency_key: format!("post-commit-{}-verify", request.commit_id),
        command: OfflineCommand::RuntimeVerify {
            package_directory: request.package_directory.clone(),
        },
    })?;
    let distribute = execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "post-commit.distribute".to_owned(),
        actor: "git:post-commit".to_owned(),
        reason: "distribute the verified native package after compile".to_owned(),
        idempotency_key: format!("post-commit-{}-distribute", request.commit_id),
        mode: ExecutionMode::Apply,
        allowed_roots: request.allowed_roots.clone(),
        job: JobInput::Distribute(DistributeInput {
            package_directory: request.package_directory.clone(),
            target_directories: request.target_directories.clone(),
        }),
    })?;
    Ok(PostCommitExecution {
        compile,
        verification,
        distribute,
    })
}

fn validate(request: &PostCommitRequest) -> Result<(), PostCommitError> {
    if request.commit_id.len() < 7
        || request.commit_id.len() > 64
        || !request
            .commit_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || request.target_directories.is_empty()
        || request.allowed_roots.is_empty()
    {
        return Err(PostCommitError::InvalidInput);
    }
    let repository = canonical_directory(&request.repository_root)?;
    let source = canonical_directory(&request.source_directory)?;
    if !source.starts_with(&repository) {
        return Err(PostCommitError::RepositoryContainment(
            request.source_directory.display().to_string(),
        ));
    }
    let package_parent = existing_ancestor(&request.package_directory)?;
    if !package_parent.starts_with(&repository) {
        return Err(PostCommitError::RepositoryContainment(
            request.package_directory.display().to_string(),
        ));
    }
    Ok(())
}

fn canonical_directory(path: &Path) -> Result<PathBuf, PostCommitError> {
    let canonical = path.canonicalize().map_err(PostCommitError::Io)?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(PostCommitError::InvalidInput)
    }
}

fn existing_ancestor(path: &Path) -> Result<PathBuf, PostCommitError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(PostCommitError::Io)?
            .join(path)
    };
    let mut current = absolute.as_path();
    while !current.exists() {
        current = current.parent().ok_or(PostCommitError::InvalidInput)?;
    }
    current.canonicalize().map_err(PostCommitError::Io)
}

/// Native post-commit failure.
#[derive(Debug, Error)]
pub enum PostCommitError {
    /// The commit, target, or root set is empty or malformed.
    #[error("post-commit input is empty, malformed, or unbounded")]
    InvalidInput,
    /// Source or package path is outside the declared repository.
    #[error("post-commit source/package escapes the repository: {0}")]
    RepositoryContainment(String),
    /// Filesystem validation failed.
    #[error("post-commit filesystem validation failed: {0}")]
    Io(std::io::Error),
    /// Native transaction failed.
    #[error("post-commit native job failed: {0}")]
    Job(#[from] JobError),
    /// Package verification failed.
    #[error("post-commit package verification failed: {0}")]
    Verify(#[from] OfflineError),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn fixture() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("ae-sdd-post-commit-{}-{nonce}", std::process::id()));
        fs::create_dir_all(root.join("repo/source")).expect("source");
        fs::create_dir_all(root.join("home")).expect("home");
        fs::write(
            root.join("repo/source/SKILL.md"),
            "---\nname: fixture\n---\n",
        )
        .expect("skill");
        root
    }

    #[test]
    fn typed_post_commit_compiles_verifies_distributes_and_replays() {
        let root = fixture();
        let repo = root.join("repo");
        let home = root.join("home");
        let request = PostCommitRequest {
            repository_root: repo.clone(),
            source_directory: repo.join("source"),
            package_directory: repo.join("dist/ae-sdd"),
            target_directories: vec![home.join(".codex/skills/ae-sdd")],
            allowed_roots: vec![repo, home.clone()],
            commit_id: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        };
        let first = execute_post_commit(&request).expect("post-commit");
        assert_eq!(first.verification.payload["format"], "native");
        assert!(home.join(".codex/skills/ae-sdd/SKILL.md").is_file());
        let replay = execute_post_commit(&request).expect("post-commit replay");
        assert!(replay.compile.replayed);
        assert!(replay.distribute.replayed);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn typed_post_commit_rejects_source_outside_repository() {
        let root = fixture();
        let repo = root.join("repo");
        let home = root.join("home");
        let request = PostCommitRequest {
            repository_root: repo.clone(),
            source_directory: home.clone(),
            package_directory: repo.join("dist/ae-sdd"),
            target_directories: vec![home.join("target")],
            allowed_roots: vec![repo, home],
            commit_id: "0123456".to_owned(),
        };
        assert!(matches!(
            execute_post_commit(&request),
            Err(PostCommitError::RepositoryContainment(_))
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
