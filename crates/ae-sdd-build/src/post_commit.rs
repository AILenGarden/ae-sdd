use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::{
    AdminChange, CompileInput, DistributeInput, ExecutionMode, InitInput, JobError, JobExecution,
    JobInput, ManagedInstructionError, ManagedInstructionPlan, ManagedInstructionRenderRequest,
    ManagedInstructionTarget, NativeJobRequest, OfflineCommand, OfflineError, OfflineRequest,
    OfflineResult, PermissionClass, execute_native_job, execute_offline,
    render_managed_instruction,
};

/// Relative path of the L2 discipline SSOT inside the compiled package.
const L2_DISCIPLINE_PATH: &str = "L2-DISCIPLINE.md";

/// Bounded read ceiling for the SSOT and every managed target file.
const MAX_MANAGED_FILE_BYTES: u64 = 1024 * 1024;

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
    /// Explicit managed global instruction targets. An empty list disables the
    /// managed L2 stage entirely; targets are never inferred from skill
    /// distribution directories.
    pub managed_instruction_targets: Vec<ManagedInstructionTarget>,
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
    /// Per-host managed L2 instruction outcome in stable host-name order.
    pub managed_instructions: Vec<ManagedInstructionOutcome>,
}

/// Reported result of one managed global instruction file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedInstructionStatus {
    /// The anchor range was rewritten through a native Admin transaction.
    Updated,
    /// The rendered file was byte-identical, so nothing was written.
    Unchanged,
    /// The target file does not exist; it is never created.
    MissingTarget,
    /// The target exists without a complete anchor pair; it is never modified.
    MissingAnchor,
}

/// Managed instruction outcome for a single host.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedInstructionOutcome {
    /// Stable host name.
    pub host: String,
    /// Target file path as reported to the caller.
    pub target_file: String,
    /// Reported status.
    pub status: ManagedInstructionStatus,
    /// Short digest of the rendered language body, absent when nothing rendered.
    pub content_hash: Option<String>,
    /// Native transaction receipt, present only for an applied update.
    pub job: Option<JobExecution>,
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
    let managed_instructions = sync_managed_instructions(request)?;
    Ok(PostCommitExecution {
        compile,
        verification,
        distribute,
        managed_instructions,
    })
}

/// Refreshes every explicitly declared managed global instruction file.
///
/// Runs after skill distribution so a managed skip or failure can never mask a
/// successful package delivery. Each changed host is applied through its own
/// native `Admin` transaction: per-file atomicity and rollback are preserved,
/// while cross-host atomicity is intentionally not claimed.
fn sync_managed_instructions(
    request: &PostCommitRequest,
) -> Result<Vec<ManagedInstructionOutcome>, PostCommitError> {
    if request.managed_instruction_targets.is_empty() {
        return Ok(Vec::new());
    }
    let source = read_bounded_utf8(&request.package_directory.join(L2_DISCIPLINE_PATH))?;
    let mut targets: Vec<&ManagedInstructionTarget> =
        request.managed_instruction_targets.iter().collect();
    targets.sort_by(|left, right| left.host.cmp(&right.host));

    let mut outcomes = Vec::with_capacity(targets.len());
    for target in targets {
        outcomes.push(sync_managed_target(request, &source, target)?);
    }
    Ok(outcomes)
}

fn sync_managed_target(
    request: &PostCommitRequest,
    source: &str,
    target: &ManagedInstructionTarget,
) -> Result<ManagedInstructionOutcome, PostCommitError> {
    let target_file = display_path(&target.target_file);
    let metadata = fs::symlink_metadata(&target.target_file);
    match metadata {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ManagedInstructionOutcome {
                host: target.host.clone(),
                target_file,
                status: ManagedInstructionStatus::MissingTarget,
                content_hash: None,
                job: None,
            });
        }
        Err(error) => return Err(PostCommitError::Io(error)),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(PostCommitError::ManagedInstructionTarget(target_file));
        }
        Ok(_) => {}
    }

    let current = read_bounded_utf8(&target.target_file)?;
    let plan = render_managed_instruction(&ManagedInstructionRenderRequest {
        source,
        target: &current,
        language: target.language,
        revision: &request.commit_id,
    })
    .map_err(|source| PostCommitError::ManagedInstructions {
        host: target.host.clone(),
        source,
    })?;

    match plan {
        ManagedInstructionPlan::MissingAnchor => Ok(ManagedInstructionOutcome {
            host: target.host.clone(),
            target_file,
            status: ManagedInstructionStatus::MissingAnchor,
            content_hash: None,
            job: None,
        }),
        ManagedInstructionPlan::Unchanged { content_hash } => Ok(ManagedInstructionOutcome {
            host: target.host.clone(),
            target_file,
            status: ManagedInstructionStatus::Unchanged,
            content_hash: Some(content_hash),
            job: None,
        }),
        ManagedInstructionPlan::Updated {
            contents,
            content_hash,
        } => {
            let job = apply_managed_target(request, target, contents)?;
            Ok(ManagedInstructionOutcome {
                host: target.host.clone(),
                target_file,
                status: ManagedInstructionStatus::Updated,
                content_hash: Some(content_hash),
                job: Some(job),
            })
        }
    }
}

fn apply_managed_target(
    request: &PostCommitRequest,
    target: &ManagedInstructionTarget,
    contents: String,
) -> Result<JobExecution, PostCommitError> {
    let parent = target
        .target_file
        .parent()
        .ok_or_else(|| {
            PostCommitError::ManagedInstructionTarget(display_path(&target.target_file))
        })?
        .to_path_buf();
    let file_name = target
        .target_file
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| {
            PostCommitError::ManagedInstructionTarget(display_path(&target.target_file))
        })?;
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "post-commit.managed-instructions".to_owned(),
        actor: "git:post-commit".to_owned(),
        reason: format!("refresh ae-sdd managed instructions for {}", target.host),
        idempotency_key: format!(
            "post-commit-{}-managed-instructions-{}",
            request.commit_id, target.host
        ),
        mode: ExecutionMode::Apply,
        allowed_roots: request.allowed_roots.clone(),
        job: JobInput::Admin(InitInput {
            project_root: parent,
            changes: vec![AdminChange {
                relative_path: file_name,
                contents,
                permission: PermissionClass::PrivateFile,
            }],
        }),
    })
    .map_err(PostCommitError::Job)
}

fn read_bounded_utf8(path: &Path) -> Result<String, PostCommitError> {
    let metadata = fs::symlink_metadata(path).map_err(PostCommitError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PostCommitError::ManagedInstructionTarget(display_path(
            path,
        )));
    }
    if metadata.len() > MAX_MANAGED_FILE_BYTES {
        return Err(PostCommitError::ManagedInstructionTarget(display_path(
            path,
        )));
    }
    let bytes = fs::read(path).map_err(PostCommitError::Io)?;
    String::from_utf8(bytes)
        .map_err(|_| PostCommitError::ManagedInstructionTarget(display_path(path)))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
    /// The managed L2 source or a host anchor is malformed.
    #[error("post-commit managed instruction rendering failed for {host}: {source}")]
    ManagedInstructions {
        /// Host whose managed file could not be rendered.
        host: String,
        /// Underlying rendering failure.
        source: ManagedInstructionError,
    },
    /// A managed target is a symlink, not a regular file, not UTF-8, or is
    /// larger than the bounded read ceiling.
    #[error("post-commit managed instruction target is not an accepted regular file: {0}")]
    ManagedInstructionTarget(String),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::InstructionLanguage;

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
            managed_instruction_targets: Vec::new(),
        };
        let first = execute_post_commit(&request).expect("post-commit");
        assert_eq!(first.verification.payload["format"], "native");
        assert!(home.join(".codex/skills/ae-sdd/SKILL.md").is_file());
        assert!(first.managed_instructions.is_empty());
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
            managed_instruction_targets: Vec::new(),
        };
        assert!(matches!(
            execute_post_commit(&request),
            Err(PostCommitError::RepositoryContainment(_))
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    const L2_FIXTURE: &str = concat!(
        "<!-- header -->\n",
        "<!-- SECTION:zh -->\n",
        "## 中文纪律\n",
        "<!-- /SECTION:zh -->\n",
        "<!-- SECTION:en -->\n",
        "## English discipline\n",
        "<!-- /SECTION:en -->\n",
    );

    fn managed_fixture() -> PathBuf {
        let root = fixture();
        fs::write(root.join("repo/source/L2-DISCIPLINE.md"), L2_FIXTURE).expect("L2 source");
        root
    }

    fn managed_request(
        root: &Path,
        targets: Vec<ManagedInstructionTarget>,
        commit: &str,
    ) -> PostCommitRequest {
        let repo = root.join("repo");
        let home = root.join("home");
        PostCommitRequest {
            repository_root: repo.clone(),
            source_directory: repo.join("source"),
            package_directory: repo.join("dist/ae-sdd"),
            target_directories: vec![home.join(".codex/skills/ae-sdd")],
            allowed_roots: vec![repo, home],
            commit_id: commit.to_owned(),
            managed_instruction_targets: targets,
        }
    }

    fn anchored(stale: &str) -> String {
        format!(
            "# Global\n\n<!-- BEGIN ae-sdd-l2-ssot @ old -->\n{stale}\n<!-- END ae-sdd-l2-ssot -->\n\n## Personal\n"
        )
    }

    #[test]
    fn typed_post_commit_updates_skips_and_replays_managed_instructions() {
        let root = managed_fixture();
        let home = root.join("home");
        let codex = home.join(".codex/AGENTS.md");
        let claude = home.join(".claude/CLAUDE.md");
        let zcode = home.join(".zcode/AGENTS.md");
        fs::create_dir_all(codex.parent().expect("codex parent")).expect("codex dir");
        fs::create_dir_all(claude.parent().expect("claude parent")).expect("claude dir");
        fs::create_dir_all(zcode.parent().expect("zcode parent")).expect("zcode dir");
        fs::write(&codex, anchored("## Stale")).expect("codex file");
        fs::write(&claude, "# Claude\n\n## Hand written\n").expect("claude file");
        // zcode file intentionally absent

        let targets = vec![
            ManagedInstructionTarget {
                host: "codex".to_owned(),
                language: InstructionLanguage::En,
                target_file: codex.clone(),
            },
            ManagedInstructionTarget {
                host: "claude".to_owned(),
                language: InstructionLanguage::Zh,
                target_file: claude.clone(),
            },
            ManagedInstructionTarget {
                host: "zcode".to_owned(),
                language: InstructionLanguage::Zh,
                target_file: zcode.clone(),
            },
        ];
        let request = managed_request(&root, targets, "0123456789abcdef0123456789abcdef01234567");
        let execution = execute_post_commit(&request).expect("post-commit");

        let statuses: Vec<(&str, ManagedInstructionStatus)> = execution
            .managed_instructions
            .iter()
            .map(|outcome| (outcome.host.as_str(), outcome.status))
            .collect();
        assert_eq!(
            statuses,
            vec![
                ("claude", ManagedInstructionStatus::MissingAnchor),
                ("codex", ManagedInstructionStatus::Updated),
                ("zcode", ManagedInstructionStatus::MissingTarget),
            ],
            "hosts must be reported in stable name order"
        );
        let codex_text = fs::read_to_string(&codex).expect("codex text");
        assert!(codex_text.contains("## English discipline"));
        assert!(codex_text.starts_with("# Global\n\n"));
        assert!(codex_text.ends_with("\n## Personal\n"));
        assert_eq!(
            fs::read_to_string(&claude).expect("claude text"),
            "# Claude\n\n## Hand written\n",
            "an unanchored target must stay byte-identical"
        );
        assert!(!zcode.exists(), "a missing target must never be created");

        let replay = execute_post_commit(&request).expect("post-commit replay");
        assert_eq!(
            replay
                .managed_instructions
                .iter()
                .find(|outcome| outcome.host == "codex")
                .map(|outcome| outcome.status),
            Some(ManagedInstructionStatus::Unchanged),
            "a byte-identical replay must not rewrite the target"
        );
        assert_eq!(
            fs::read_to_string(&codex).expect("codex replay"),
            codex_text
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn typed_post_commit_fails_closed_on_malformed_anchor_without_touching_the_file() {
        let root = managed_fixture();
        let home = root.join("home");
        let codex = home.join(".codex/AGENTS.md");
        fs::create_dir_all(codex.parent().expect("codex parent")).expect("codex dir");
        let original = "# Global\n<!-- BEGIN ae-sdd-l2-ssot @ old -->\nbody\n";
        fs::write(&codex, original).expect("codex file");

        let request = managed_request(
            &root,
            vec![ManagedInstructionTarget {
                host: "codex".to_owned(),
                language: InstructionLanguage::En,
                target_file: codex.clone(),
            }],
            "abcdef0123456789abcdef0123456789abcdef01",
        );
        let error = execute_post_commit(&request).expect_err("malformed anchor must fail closed");
        assert!(matches!(
            error,
            PostCommitError::ManagedInstructions { ref host, .. } if host == "codex"
        ));
        assert_eq!(fs::read_to_string(&codex).expect("codex text"), original);
        // Skill distribution completed before the managed stage failed.
        assert!(
            home.join(".codex/skills/ae-sdd/SKILL.md").is_file(),
            "a managed failure must not roll back package distribution"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
