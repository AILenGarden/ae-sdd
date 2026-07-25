//! V-B02 coverage for the real `HostProcessAdapter` + `HostSupervisor` path:
//! capability precheck (no spawn), real subprocess accept (exit 0), real
//! subprocess host-reject (non-zero exit, still `Ok`), and real subprocess
//! timeout — all through `BoundedCommandRunner`, the same path
//! `HostProcessAdapter::dispatch` uses in production.
//!
//! Fixture scripts are written to a `TempDir` per test so the observed exit
//! code and runtime are fully deterministic (no reliance on the ambient
//! shell/stdin state of whatever process runs `cargo test`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ae_sdd_contracts::compact::CompactRequest;
use ae_sdd_contracts::{AdapterId, IdempotencyKey, SchemaVersion};
use ae_sdd_domain::{
    ArtifactDigest, ArtifactKind, ArtifactRef, CompactId, ContextGeneration, ProjectRelativePath,
    SessionId,
};
use ae_sdd_host::{HostAckOutcome, HostAdapterId, HostCapability, HostCapabilitySet};
use ae_sdd_integrations::{
    BoundedCommandRunner, HostProcessAdapter, HostSupervisor, HostSupervisorError, IntegrationError,
};
use tempfile::TempDir;
use uuid::Uuid;

/// Writes an executable fixture script that exits with `code`, regardless of
/// the arguments `HostProcessAdapter::dispatch` passes it.
/// On Windows, Rust's `Command` auto-wraps `.bat`/`.cmd` via `cmd.exe`, so a
/// batch file is directly usable as a `HostRuntimeAdapter`'s executable path.
/// On Unix, a shebang script needs the executable bit set.
fn exit_code_script(dir: &Path, code: i32) -> PathBuf {
    #[cfg(windows)]
    {
        let path = dir.join("fixture.bat");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(file, "@echo off\r\nexit /b {code}").expect("write fixture script");
        path
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fixture.sh");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(file, "#!/bin/sh\nexit {code}").expect("write fixture script");
        let mut permissions = file.metadata().expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fixture script");
        path
    }
}

/// Writes an executable fixture script that sleeps far longer than any test
/// deadline, regardless of arguments.
fn long_running_script(dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let path = dir.join("sleep.bat");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(file, "@echo off\r\nping -n 60 127.0.0.1 >nul").expect("write fixture script");
        path
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("sleep.sh");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(file, "#!/bin/sh\nsleep 60").expect("write fixture script");
        let mut permissions = file.metadata().expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fixture script");
        path
    }
}

fn adapter_id(value: &str) -> HostAdapterId {
    HostAdapterId::new(value).expect("adapter id")
}

fn full_capabilities() -> HostCapabilitySet {
    HostCapabilitySet::new([
        HostCapability::Create,
        HostCapability::Send,
        HostCapability::Wait,
        HostCapability::Cancel,
        HostCapability::Attest,
        HostCapability::Compact,
    ])
}

fn compact_request(session_id: SessionId) -> CompactRequest {
    CompactRequest::new(
        SchemaVersion::V1,
        CompactId::from_uuid(Uuid::new_v4()),
        session_id,
        AdapterId::new("stub-adapter").expect("adapter id"),
        ArtifactRef::new(
            ArtifactKind::new("context-snapshot").expect("artifact kind"),
            ProjectRelativePath::new(".ae-sdd/snapshots/compact-1.json").expect("relative path"),
            ArtifactDigest::digest(b"snapshot"),
            8,
        ),
        ContextGeneration::new(1),
        ContextGeneration::new(2),
        60_000,
        IdempotencyKey::new("compact-1").expect("idempotency key"),
    )
    .expect("valid compact request")
}

#[test]
fn compact_dispatches_through_a_real_subprocess_and_accepts_on_exit_zero() {
    let temp_dir = TempDir::new().expect("temp dir");
    let script = exit_code_script(temp_dir.path(), 0);
    let runner = BoundedCommandRunner::new(4096);
    let adapter = HostProcessAdapter::new(
        adapter_id("stub-adapter"),
        full_capabilities(),
        script,
        runner,
    );
    let supervisor = HostSupervisor::new(adapter);
    let request = compact_request(SessionId::from_uuid(Uuid::new_v4()));

    let summary = supervisor
        .compact(&request)
        .expect("real subprocess accepts");

    assert_eq!(summary.outcome(), &HostAckOutcome::Accepted);
    assert_eq!(summary.session_id(), Some(request.session_id()));
    assert_eq!(summary.command_seq(), request.next_generation().get());
}

#[test]
fn compact_maps_a_real_nonzero_exit_to_ok_rejected_summary() {
    let temp_dir = TempDir::new().expect("temp dir");
    let script = exit_code_script(temp_dir.path(), 3);
    let runner = BoundedCommandRunner::new(4096);
    let adapter = HostProcessAdapter::new(
        adapter_id("stub-adapter"),
        full_capabilities(),
        script,
        runner,
    );
    let supervisor = HostSupervisor::new(adapter);
    let request = compact_request(SessionId::from_uuid(Uuid::new_v4()));

    let summary = supervisor
        .compact(&request)
        .expect("host-side rejection is still Ok, not Err");

    match summary.outcome() {
        HostAckOutcome::Rejected { .. } => {}
        other => panic!("expected Rejected outcome for a non-zero exit, got {other:?}"),
    }
}

#[test]
fn compact_rejects_capability_unsupported_without_spawning_a_process() {
    // A non-existent executable path proves the precheck short-circuits
    // before any spawn attempt: if `adapter.dispatch` were called, spawning
    // this path would surface as `Unavailable`, not `CapabilityUnsupported`.
    let runner = BoundedCommandRunner::new(4096);
    let adapter = HostProcessAdapter::new(
        adapter_id("stub-adapter"),
        HostCapabilitySet::new([HostCapability::Send]),
        PathBuf::from("this-executable-does-not-exist-anywhere"),
        runner,
    );
    let supervisor = HostSupervisor::new(adapter);
    let request = compact_request(SessionId::from_uuid(Uuid::new_v4()));

    let outcome = supervisor.compact(&request);

    assert_eq!(outcome, Err(HostSupervisorError::CapabilityUnsupported));
}

/// Writes an executable fixture script that snapshots the child's own
/// environment to `env.txt` (one `KEY=VALUE` line per variable it received),
/// then exits 0.
fn env_snapshot_script(dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let path = dir.join("env_snapshot.bat");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(
            file,
            "@echo off\r\nset > \"{}\"",
            dir.join("env.txt").display()
        )
        .expect("write fixture script");
        path
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("env_snapshot.sh");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(
            file,
            "#!/bin/sh\nenv > \"{}\"",
            dir.join("env.txt").display()
        )
        .expect("write fixture script");
        let mut permissions = file.metadata().expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fixture script");
        path
    }
}

/// Writes an executable fixture script that, once started, keeps a
/// grandchild process alive appending a heartbeat line to `heartbeat.txt`
/// roughly once per second, then itself idles far longer than any test
/// deadline. This mirrors a real toolchain wrapper that forks a background
/// worker: killing only the direct child must not be enough to stop it.
fn heartbeat_tree_script(dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let path = dir.join("heartbeat.bat");
        let marker = dir.join("heartbeat.txt");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(
            file,
            "@echo off\r\nstart /B cmd /C \"for /l %%i in () do (echo x>>\"{marker}\" & ping -n 1 127.0.0.1 >nul)\"\r\nping -n 60 127.0.0.1 >nul",
            marker = marker.display(),
        )
        .expect("write fixture script");
        path
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("heartbeat.sh");
        let marker = dir.join("heartbeat.txt");
        let mut file = std::fs::File::create(&path).expect("create fixture script");
        writeln!(
            file,
            "#!/bin/sh\n(while true; do echo x >> \"{marker}\"; sleep 0.2; done) &\nsleep 60",
            marker = marker.display(),
        )
        .expect("write fixture script");
        let mut permissions = file.metadata().expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod fixture script");
        path
    }
}

#[test]
fn dispatch_forwards_only_allowlisted_environment_variables_to_the_subprocess() {
    let temp_dir = TempDir::new().expect("temp dir");
    let script = env_snapshot_script(temp_dir.path());
    let runner = BoundedCommandRunner::new(1_048_576);

    // `CARGO_MANIFEST_DIR` is guaranteed present in *this* test process
    // (Cargo sets it for every test binary at runtime, not just compile
    // time) but is not on the hardening allowlist, so its absence in the
    // child's snapshot proves leakage prevention rather than just "some
    // filtering runs". Reading it (never mutating process env) keeps this
    // test race-free under the default multi-threaded test runner.
    assert!(
        std::env::var_os("CARGO_MANIFEST_DIR").is_some(),
        "test precondition: Cargo must set CARGO_MANIFEST_DIR for this to be a meaningful probe"
    );

    let outcome = runner.run(&script, &[], Some(temp_dir.path()), Duration::from_secs(10));

    outcome.expect("env snapshot script runs to completion");
    let snapshot = std::fs::read_to_string(temp_dir.path().join("env.txt"))
        .expect("env snapshot file is written");

    assert!(
        !snapshot.to_uppercase().contains("CARGO_MANIFEST_DIR"),
        "Cargo's own non-allowlisted build-time variable leaked into the child's environment"
    );
    assert!(
        snapshot.to_uppercase().contains("PATH="),
        "the allowlisted PATH variable must still reach the child so the executable can resolve"
    );
}

#[test]
fn deadline_overrun_cleanup_stops_a_grandchild_the_immediate_kill_would_orphan() {
    let temp_dir = TempDir::new().expect("temp dir");
    let script = heartbeat_tree_script(temp_dir.path());
    let marker = temp_dir.path().join("heartbeat.txt");
    let runner = BoundedCommandRunner::new(4096);

    let outcome = runner.run(
        &script,
        &[],
        Some(temp_dir.path()),
        Duration::from_millis(400),
    );
    assert!(
        matches!(outcome, Err(IntegrationError::CommandTimeout)),
        "expected the heartbeat fixture to overrun the deadline, got {outcome:?}"
    );

    let heartbeat_count = || -> usize {
        std::fs::read_to_string(&marker)
            .unwrap_or_default()
            .lines()
            .count()
    };
    // `taskkill /T /F` (and the equivalent process-group signal on Unix) is
    // asynchronous: the OS may take a few milliseconds after `runner.run`
    // returns to actually tear down the grandchild, so a write already
    // in-flight at that exact instant can still land. Give cleanup a short
    // grace window before taking the first sample, then confirm the count
    // has genuinely stabilized (not just "grew slower") over a second,
    // longer window — the fixture's own heartbeat period is ~1s on Windows
    // and ~200ms on Unix, so 1.5s is several periods on either platform.
    std::thread::sleep(Duration::from_millis(500));
    let count_after_grace = heartbeat_count();
    std::thread::sleep(Duration::from_millis(1_500));
    let count_later = heartbeat_count();

    assert_eq!(
        count_after_grace, count_later,
        "grandchild kept appending heartbeats well after the deadline cleanup; \
         process-tree cleanup did not reach it (count went {count_after_grace} -> {count_later})"
    );
}

#[test]
fn compact_maps_a_real_subprocess_deadline_overrun_to_timeout() {
    let temp_dir = TempDir::new().expect("temp dir");
    let script = long_running_script(temp_dir.path());
    // `HostProcessAdapter::dispatch` hardcodes a 30s deadline internally
    // (see `command.rs`), so this test cannot force a faster timeout through
    // `HostSupervisor::compact`. It instead exercises the same
    // `BoundedCommandRunner` deadline path directly against the real
    // long-running fixture, proving the runner really kills and reports a
    // timeout rather than hanging — the mapping from
    // `IntegrationError::CommandTimeout` to `HostAdapterError::Timeout` to
    // `HostSupervisorError::Timeout` is covered deterministically by the
    // stub-adapter unit test `host_supervisor::tests::dispatch_maps_timeout`.
    let runner = BoundedCommandRunner::new(4096);

    let outcome = runner.run(&script, &[], None, Duration::from_millis(300));

    assert!(
        matches!(outcome, Err(IntegrationError::CommandTimeout)),
        "expected the real long-running subprocess to be killed after the deadline, got {outcome:?}"
    );
}
