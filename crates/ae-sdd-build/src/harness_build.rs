use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::{
    ExecutionMode, HarnessInput, JobError, JobExecution, JobInput, NativeJobRequest,
    execute_native_job,
};

/// Direct-argv input for native harness generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessBuildRequest {
    /// Ordered declarative source files.
    pub source_files: Vec<PathBuf>,
    /// Generated harness entry.
    pub target_file: PathBuf,
    /// Human-readable harness title.
    pub title: String,
    /// Explicit containment roots.
    pub allowed_roots: Vec<PathBuf>,
    /// Whether to plan or apply the generated artifact.
    pub mode: ExecutionMode,
}

/// Generates a bounded harness with a content-derived idempotency key.
pub fn execute_harness_build(request: &HarnessBuildRequest) -> Result<JobExecution, JobError> {
    let mut digest = Sha256::new();
    digest.update(b"ae-sdd-harness-build/v1\0");
    digest.update(request.title.as_bytes());
    digest.update([0]);
    digest.update(request.target_file.to_string_lossy().as_bytes());
    for source in &request.source_files {
        digest.update([0]);
        digest.update(source.to_string_lossy().as_bytes());
        let bytes = std::fs::read(source).map_err(|source_error| JobError::Io {
            path: source.display().to_string(),
            source: source_error,
        })?;
        digest.update([0]);
        digest.update(bytes);
    }
    execute_native_job(&NativeJobRequest {
        schema_version: "ae-sdd-native-job/v1".to_owned(),
        entrypoint: "harness".to_owned(),
        actor: "cli:harness".to_owned(),
        reason: "generate the native host harness from declarative sources".to_owned(),
        idempotency_key: hex::encode(digest.finalize()),
        mode: request.mode,
        allowed_roots: request.allowed_roots.clone(),
        job: JobInput::Harness(HarnessInput {
            source_files: request.source_files.clone(),
            target_file: request.target_file.clone(),
            title: request.title.clone(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn direct_harness_request_is_content_idempotent() {
        let root =
            std::env::temp_dir().join(format!("ae-sdd-direct-harness-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture");
        fs::write(root.join("SKILL.md"), "skill\n").expect("skill");
        let request = HarnessBuildRequest {
            source_files: vec![root.join("SKILL.md")],
            target_file: root.join("harness/agent.md"),
            title: "ae-sdd Agent Harness".to_owned(),
            allowed_roots: vec![root.clone()],
            mode: ExecutionMode::Apply,
        };
        let first = execute_harness_build(&request).expect("first harness build");
        let mut planned_request = request.clone();
        planned_request.mode = ExecutionMode::DryRun;
        let planned = execute_harness_build(&planned_request).expect("replanned harness");
        assert_eq!(first.request_digest, planned.request_digest);
        assert_eq!(first.changes, planned.changes);
        assert_eq!(first.plan_digest, planned.plan_digest);
        let replay = execute_harness_build(&request).expect("harness replay");
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert!(root.join("harness/agent.md").is_file());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
