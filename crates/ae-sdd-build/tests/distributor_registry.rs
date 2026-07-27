//! Contract for reading the distributor registry.
//!
//! The registry replaced a hardcoded host list, so the cases that matter are the
//! ones where a host would vanish or be invented without anyone noticing.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{InstructionLanguage, SkipReason, resolve_registry};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ae-sdd-distributor-registry-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture root");
        Self { root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_registry(root: &Path, body: &str) -> PathBuf {
    let path = root.join("distributors.json");
    fs::write(&path, body).expect("registry fixture");
    path
}

#[test]
fn enabled_hosts_supply_package_and_instruction_targets() {
    let fixture = Fixture::new("resolve");
    let home = fixture.home();
    fs::create_dir_all(home.join(".codex/skills/ae-sdd")).expect("codex target");
    let registry = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":"~/.claude/CLAUDE.md","l2_language":"zh"},
                {"name":"codex","protocol":"copytree","target_path":"~/.codex/skills/ae-sdd",
                 "detect":"path_exists","detect_cli":null,"enabled":true,
                 "l2_global_file":"~/.codex/AGENTS.md","l2_language":"en"},
                {"name":"qoder","protocol":"copytree","target_path":"~/.qoder/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null}
        ]}"#,
    );

    let resolution = resolve_registry(&registry, &home).expect("registry resolves");

    let names: Vec<_> = resolution
        .hosts
        .iter()
        .map(|host| host.name.as_str())
        .collect();
    assert_eq!(names, ["claude", "codex", "qoder"]);
    assert!(resolution.skipped.is_empty());

    let claude = &resolution.hosts[0];
    assert_eq!(claude.package_target, home.join(".claude/skills/ae-sdd"));
    let (file, language) = claude
        .instruction_target
        .as_ref()
        .expect("claude declares an instruction file");
    assert_eq!(file, &home.join(".claude/CLAUDE.md"));
    assert_eq!(*language, InstructionLanguage::Zh);

    assert_eq!(
        resolution.hosts[1].instruction_target.as_ref().map(|t| t.1),
        Some(InstructionLanguage::En)
    );
    // A package-only host must never have an instruction file inferred from its
    // skill directory.
    assert!(resolution.hosts[2].instruction_target.is_none());
}

#[test]
fn a_disabled_or_undetected_host_is_named_rather_than_dropped() {
    let fixture = Fixture::new("skip");
    let home = fixture.home();
    fs::create_dir_all(&home).expect("home");
    let registry = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null},
                {"name":"retired","protocol":"copytree","target_path":"~/.retired/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":false,
                 "l2_global_file":null,"l2_language":null},
                {"name":"absent","protocol":"copytree","target_path":"~/.absent/skills/ae-sdd",
                 "detect":"path_exists","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null}
        ]}"#,
    );

    let resolution = resolve_registry(&registry, &home).expect("registry resolves");

    assert_eq!(resolution.hosts.len(), 1);
    assert_eq!(resolution.hosts[0].name, "claude");
    let skipped: Vec<_> = resolution
        .skipped
        .iter()
        .map(|host| (host.name.as_str(), host.reason))
        .collect();
    assert_eq!(
        skipped,
        [
            ("retired", SkipReason::Disabled),
            ("absent", SkipReason::NotDetected),
        ]
    );
    // The undetected host's directory must not be created as a side effect of
    // being asked about; that is how a phantom target appears.
    assert!(!home.join(".absent").exists());
}

#[test]
fn an_unimplemented_protocol_fails_closed() {
    let fixture = Fixture::new("protocol");
    let home = fixture.home();
    let registry = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"mounted","protocol":"harness_mount","target_path":"~/.mounted",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null}
        ]}"#,
    );

    let error = resolve_registry(&registry, &home).expect_err("harness_mount has no native path");
    assert!(
        error.to_string().contains("harness_mount"),
        "unexpected error: {error}"
    );
}

#[test]
fn incomplete_or_duplicated_declarations_fail_closed() {
    let fixture = Fixture::new("incomplete");
    let home = fixture.home();

    let missing_language = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":"~/.claude/CLAUDE.md","l2_language":null}
        ]}"#,
    );
    let error = resolve_registry(&missing_language, &home).expect_err("language is required");
    assert!(
        error.to_string().contains("l2Language"),
        "unexpected error: {error}"
    );

    let unknown_language = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":"~/.claude/CLAUDE.md","l2_language":"fr"}
        ]}"#,
    );
    let error = resolve_registry(&unknown_language, &home).expect_err("fr is not a managed slice");
    assert!(
        error.to_string().contains("fr"),
        "unexpected error: {error}"
    );

    let duplicated = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null},
                {"name":"claude","protocol":"copytree","target_path":"~/.other/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null}
        ]}"#,
    );
    let error = resolve_registry(&duplicated, &home).expect_err("duplicate host is ambiguous");
    assert!(
        error.to_string().contains("twice"),
        "unexpected error: {error}"
    );
}

#[test]
fn a_malformed_registry_is_reported_with_its_path() {
    let fixture = Fixture::new("malformed");
    let home = fixture.home();

    let missing = fixture.path().join("absent.json");
    let error = resolve_registry(&missing, &home).expect_err("missing registry");
    assert!(
        error.to_string().contains("absent.json"),
        "unexpected error: {error}"
    );

    let malformed = write_registry(fixture.path(), "{ not an array }");
    let error = resolve_registry(&malformed, &home).expect_err("malformed registry");
    assert!(
        error.to_string().contains("valid JSON"),
        "unexpected error: {error}"
    );
}

#[test]
fn an_unreadable_envelope_fails_closed_rather_than_resolving_no_host() {
    let fixture = Fixture::new("envelope");
    let home = fixture.home();

    // The Python loader falls back to seed defaults on a shape it cannot read,
    // which lets a registry look healthy while the hosts it declares are ignored.
    // The native reader refuses instead.
    let future_schema = write_registry(
        fixture.path(),
        r#"{"schema_version":2,"distributors":[
            {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
             "detect":"always","detect_cli":null,"enabled":true,
             "l2_global_file":null,"l2_language":null}
        ]}"#,
    );
    let error = resolve_registry(&future_schema, &home).expect_err("schema 2 is not understood");
    assert!(
        error.to_string().contains("schema version 2"),
        "unexpected error: {error}"
    );

    let renamed_key = write_registry(fixture.path(), r#"{"schema_version":1,"hosts":[]}"#);
    let error = resolve_registry(&renamed_key, &home).expect_err("distributors key is required");
    assert!(
        error.to_string().contains("valid JSON"),
        "unexpected error: {error}"
    );
}

#[test]
fn unknown_registry_fields_are_carried_without_rejecting_the_entry() {
    let fixture = Fixture::new("extra");
    let home = fixture.home();
    // The Python writer records provenance fields the native reader has no use
    // for; rejecting them would break every existing registry on disk.
    let registry = write_registry(
        fixture.path(),
        r#"{"schema_version":1,"distributors":[
                {"name":"claude","protocol":"copytree","target_path":"~/.claude/skills/ae-sdd",
                 "detect":"always","detect_cli":null,"enabled":true,
                 "l2_global_file":null,"l2_language":null,
                 "registered_at":"2026-07-23T03:28:45.402502Z","notes":"Claude Code"}
        ]}"#,
    );

    let resolution = resolve_registry(&registry, &home).expect("extra fields are tolerated");
    assert_eq!(resolution.hosts.len(), 1);
}
