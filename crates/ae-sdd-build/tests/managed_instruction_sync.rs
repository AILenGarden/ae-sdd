//! End-to-end evidence that the released Rust post-commit chain updates only the
//! managed anchor range of each host's global instruction file.
//!
//! Every scenario runs against a temporary repository and a temporary home so no
//! test can ever touch the developer's real user profile.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ae_sdd_build::{
    InstructionLanguage, ManagedInstructionStatus, ManagedInstructionTarget, PostCommitError,
    PostCommitRequest, execute_post_commit,
};
use sha2::{Digest, Sha256};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

const L2_SSOT: &str = concat!(
    "<!-- ae-sdd L2 conversation discipline SSOT -->\n",
    "\n",
    "<!-- SECTION:zh -->\n",
    "## 强制工作流\n",
    "\n",
    "- 中文条目\n",
    "<!-- /SECTION:zh -->\n",
    "\n",
    "<!-- SECTION:en -->\n",
    "## Mandatory Workflow\n",
    "\n",
    "- English item\n",
    "<!-- /SECTION:en -->\n",
);

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
            "ae-sdd-managed-l2-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("repo/source")).expect("source directory");
        fs::create_dir_all(root.join("home")).expect("home directory");
        fs::write(
            root.join("repo/source/SKILL.md"),
            "---\nname: managed-l2-fixture\n---\n",
        )
        .expect("skill entry");
        fs::write(root.join("repo/source/L2-DISCIPLINE.md"), L2_SSOT).expect("L2 SSOT");
        Self { root }
    }

    fn repo(&self) -> PathBuf {
        self.root.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn host_file(&self, relative: &str) -> PathBuf {
        let path = self.home().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("host directory");
        }
        path
    }

    fn request(&self, targets: Vec<ManagedInstructionTarget>) -> PostCommitRequest {
        self.request_with_commit(targets, COMMIT)
    }

    fn request_with_commit(
        &self,
        targets: Vec<ManagedInstructionTarget>,
        commit: &str,
    ) -> PostCommitRequest {
        let repo = self.repo();
        let home = self.home();
        PostCommitRequest {
            repository_root: repo.clone(),
            source_directory: repo.join("source"),
            package_directory: repo.join("dist/ae-sdd"),
            target_directories: vec![
                home.join(".claude/skills/ae-sdd"),
                home.join(".codex/skills/ae-sdd"),
                home.join(".zcode/skills/ae-sdd"),
                home.join(".harness/skills/ae-sdd"),
                home.join(".hermes/skills/ae-sdd"),
            ],
            allowed_roots: vec![repo, home],
            commit_id: commit.to_owned(),
            managed_instruction_targets: targets,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn target(host: &str, language: InstructionLanguage, file: &Path) -> ManagedInstructionTarget {
    ManagedInstructionTarget {
        host: host.to_owned(),
        language,
        target_file: file.to_path_buf(),
    }
}

fn anchored(prefix: &str, stale: &str, suffix: &str, newline: &str) -> String {
    [
        prefix,
        "",
        "<!-- BEGIN ae-sdd-l2-ssot @ deadbee @ legacy (hash=000000000000 v=0.9.0) -->",
        stale,
        "<!-- END ae-sdd-l2-ssot -->",
        "",
        suffix,
        "",
    ]
    .join(newline)
}

fn digest(path: &Path) -> String {
    let bytes = fs::read(path).expect("digest read");
    hex::encode(Sha256::digest(&bytes))
}

fn status_of(
    outcomes: &[ae_sdd_build::ManagedInstructionOutcome],
    host: &str,
) -> ManagedInstructionStatus {
    outcomes
        .iter()
        .find(|outcome| outcome.host == host)
        .map(|outcome| outcome.status)
        .unwrap_or_else(|| panic!("host {host} must be reported"))
}

#[test]
fn managed_instruction_sync_updates_anchored_hosts() {
    let fixture = Fixture::new("update");
    let codex = fixture.host_file(".codex/AGENTS.md");
    let claude = fixture.host_file(".claude/CLAUDE.md");
    let zcode = fixture.host_file(".zcode/AGENTS.md");
    fs::write(
        &codex,
        anchored(
            "# Codex Global",
            "## Stale english",
            "## Skill Source",
            "\n",
        ),
    )
    .expect("codex fixture");
    fs::write(
        &claude,
        anchored("# Claude 全局", "## 过期中文", "## 个人约定", "\n"),
    )
    .expect("claude fixture");
    fs::write(
        &zcode,
        anchored("# ZCode 全局", "## 过期中文", "## 个人约定", "\n"),
    )
    .expect("zcode fixture");

    let execution = execute_post_commit(&fixture.request(vec![
        target("codex", InstructionLanguage::En, &codex),
        target("claude", InstructionLanguage::Zh, &claude),
        target("zcode", InstructionLanguage::Zh, &zcode),
    ]))
    .expect("post-commit with managed targets");

    assert_eq!(
        execution
            .managed_instructions
            .iter()
            .map(|outcome| outcome.host.as_str())
            .collect::<Vec<_>>(),
        vec!["claude", "codex", "zcode"],
        "outcomes must be reported in stable host-name order"
    );
    for host in ["claude", "codex", "zcode"] {
        assert_eq!(
            status_of(&execution.managed_instructions, host),
            ManagedInstructionStatus::Updated
        );
    }

    let codex_text = fs::read_to_string(&codex).expect("codex text");
    assert!(codex_text.contains("## Mandatory Workflow"));
    assert!(!codex_text.contains("## 强制工作流"));
    assert!(!codex_text.contains("## Stale english"));

    for (path, label) in [(&claude, "claude"), (&zcode, "zcode")] {
        let text = fs::read_to_string(path).expect("chinese host text");
        assert!(text.contains("## 强制工作流"), "{label}");
        assert!(!text.contains("## Mandatory Workflow"), "{label}");
    }

    for outcome in &execution.managed_instructions {
        let job = outcome
            .job
            .as_ref()
            .expect("an update must carry a receipt");
        assert_eq!(job.entrypoint, "post-commit.managed-instructions");
        assert_eq!(job.job_kind.as_str(), "admin");
        assert!(job.receipt.is_some());
    }
}

#[test]
fn managed_instruction_sync_preserves_bytes_outside_the_anchor() {
    let fixture = Fixture::new("outside");
    let codex = fixture.host_file(".codex/AGENTS.md");
    let prefix = "# Codex Global Instructions\n\n## Personal preface\n\n- keep me";
    let suffix = "## Skill Source\n\n- runtime path: C:/Users/example/.codex/skills/ae-sdd/SKILL.md\n\n## Sync Discipline\n\n- keep this too";
    let original = anchored(prefix, "## Stale", suffix, "\n");
    fs::write(&codex, &original).expect("codex fixture");

    execute_post_commit(&fixture.request(vec![target("codex", InstructionLanguage::En, &codex)]))
        .expect("post-commit");

    let updated = fs::read_to_string(&codex).expect("codex text");
    let before = |text: &str| {
        text.split("<!-- BEGIN ae-sdd-l2-ssot")
            .next()
            .expect("prefix")
            .to_owned()
    };
    let after = |text: &str| {
        text.split("<!-- END ae-sdd-l2-ssot -->")
            .nth(1)
            .expect("suffix")
            .to_owned()
    };
    assert_eq!(before(&original), before(&updated));
    assert_eq!(after(&original), after(&updated));
}

#[test]
fn managed_instruction_sync_preserves_crlf_targets() {
    let fixture = Fixture::new("crlf");
    let codex = fixture.host_file(".codex/AGENTS.md");
    let original = anchored("# Codex", "## Stale", "## Tail", "\r\n");
    fs::write(&codex, &original).expect("codex fixture");

    execute_post_commit(&fixture.request(vec![target("codex", InstructionLanguage::En, &codex)]))
        .expect("post-commit");

    let updated = fs::read_to_string(&codex).expect("codex text");
    assert_eq!(
        updated.matches('\n').count(),
        updated.matches("\r\n").count(),
        "a CRLF target must keep every newline as CRLF"
    );
    assert!(updated.contains("## Mandatory Workflow"));
}

#[test]
fn managed_instruction_sync_skips_missing_and_unanchored_targets() {
    let fixture = Fixture::new("skip");
    let missing = fixture.home().join(".zcode/AGENTS.md");
    let unanchored = fixture.host_file(".claude/CLAUDE.md");
    let unanchored_text = "# Claude\n\n## Hand written only\n";
    fs::write(&unanchored, unanchored_text).expect("claude fixture");
    let before = digest(&unanchored);

    let execution = execute_post_commit(&fixture.request(vec![
        target("zcode", InstructionLanguage::Zh, &missing),
        target("claude", InstructionLanguage::Zh, &unanchored),
    ]))
    .expect("skips must not fail the chain");

    assert_eq!(
        status_of(&execution.managed_instructions, "zcode"),
        ManagedInstructionStatus::MissingTarget
    );
    assert_eq!(
        status_of(&execution.managed_instructions, "claude"),
        ManagedInstructionStatus::MissingAnchor
    );
    assert!(!missing.exists(), "a missing target must never be created");
    assert_eq!(digest(&unanchored), before);
    assert_eq!(
        fs::read_to_string(&unanchored).expect("claude text"),
        unanchored_text
    );
    for outcome in &execution.managed_instructions {
        assert!(outcome.job.is_none(), "a skip must not open a transaction");
    }
}

#[test]
fn managed_instruction_sync_fails_closed_on_malformed_anchor() {
    let fixture = Fixture::new("malformed");
    let codex = fixture.host_file(".codex/AGENTS.md");
    let original = concat!(
        "# Codex\n",
        "<!-- BEGIN ae-sdd-l2-ssot @ one -->\n",
        "first\n",
        "<!-- BEGIN ae-sdd-l2-ssot @ two -->\n",
        "second\n",
        "<!-- END ae-sdd-l2-ssot -->\n",
    );
    fs::write(&codex, original).expect("codex fixture");
    let before = digest(&codex);

    let error = execute_post_commit(&fixture.request(vec![target(
        "codex",
        InstructionLanguage::En,
        &codex,
    )]))
    .expect_err("a duplicated anchor must fail closed");
    assert!(matches!(
        error,
        PostCommitError::ManagedInstructions { ref host, .. } if host == "codex"
    ));
    assert_eq!(digest(&codex), before);
    assert_eq!(fs::read_to_string(&codex).expect("codex text"), original);
}

#[test]
fn managed_instruction_sync_is_idempotent_on_replay() {
    let fixture = Fixture::new("replay");
    let codex = fixture.host_file(".codex/AGENTS.md");
    fs::write(&codex, anchored("# Codex", "## Stale", "## Tail", "\n")).expect("codex fixture");
    let request = fixture.request(vec![target("codex", InstructionLanguage::En, &codex)]);

    let first = execute_post_commit(&request).expect("first run");
    assert_eq!(
        status_of(&first.managed_instructions, "codex"),
        ManagedInstructionStatus::Updated
    );
    let applied = digest(&codex);

    let second = execute_post_commit(&request).expect("replay");
    assert_eq!(
        status_of(&second.managed_instructions, "codex"),
        ManagedInstructionStatus::Unchanged
    );
    assert_eq!(digest(&codex), applied);
    assert!(
        second
            .managed_instructions
            .iter()
            .all(|outcome| outcome.job.is_none()),
        "an unchanged replay must not open a transaction"
    );
    let third = execute_post_commit(&request).expect("second replay");
    assert_eq!(digest(&codex), applied);
    assert_eq!(
        status_of(&third.managed_instructions, "codex"),
        ManagedInstructionStatus::Unchanged
    );
}

#[test]
fn managed_instruction_sync_keeps_package_distribution_independent() {
    let fixture = Fixture::new("independent");
    let missing_codex = fixture.home().join(".codex/AGENTS.md");
    let unanchored_claude = fixture.host_file(".claude/CLAUDE.md");
    fs::write(&unanchored_claude, "# Claude\n").expect("claude fixture");

    let execution = execute_post_commit(&fixture.request(vec![
        target("codex", InstructionLanguage::En, &missing_codex),
        target("claude", InstructionLanguage::Zh, &unanchored_claude),
    ]))
    .expect("all-skip must still succeed");

    for relative in [
        ".claude/skills/ae-sdd/SKILL.md",
        ".codex/skills/ae-sdd/SKILL.md",
        ".zcode/skills/ae-sdd/SKILL.md",
        ".harness/skills/ae-sdd/SKILL.md",
        ".hermes/skills/ae-sdd/SKILL.md",
    ] {
        assert!(
            fixture.home().join(relative).is_file(),
            "skill package distribution must stay independent of managed skips: {relative}"
        );
    }
    assert!(
        execution
            .managed_instructions
            .iter()
            .all(|outcome| matches!(
                outcome.status,
                ManagedInstructionStatus::MissingTarget | ManagedInstructionStatus::MissingAnchor
            ))
    );
}

#[test]
fn managed_instruction_sync_never_touches_harness_or_hermes_globals() {
    let fixture = Fixture::new("harness");
    let harness_global = fixture.host_file(".harness/AGENTS.md");
    let hermes_global = fixture.host_file(".hermes/AGENTS.md");
    let anchored_text = anchored("# Harness", "## Stale", "## Tail", "\n");
    fs::write(&harness_global, &anchored_text).expect("harness fixture");
    fs::write(&hermes_global, &anchored_text).expect("hermes fixture");
    let harness_before = digest(&harness_global);
    let hermes_before = digest(&hermes_global);

    let codex = fixture.host_file(".codex/AGENTS.md");
    fs::write(&codex, anchored("# Codex", "## Stale", "## Tail", "\n")).expect("codex fixture");

    let execution = execute_post_commit(&fixture.request(vec![target(
        "codex",
        InstructionLanguage::En,
        &codex,
    )]))
    .expect("post-commit");

    assert_eq!(execution.managed_instructions.len(), 1);
    assert_eq!(execution.managed_instructions[0].host, "codex");
    assert_eq!(digest(&harness_global), harness_before);
    assert_eq!(digest(&hermes_global), hermes_before);
    assert!(
        fixture
            .home()
            .join(".harness/skills/ae-sdd/SKILL.md")
            .is_file()
    );
    assert!(
        fixture
            .home()
            .join(".hermes/skills/ae-sdd/SKILL.md")
            .is_file()
    );
}

#[test]
fn managed_instruction_sync_rejects_targets_outside_allowed_roots() {
    let fixture = Fixture::new("containment");
    let outside_root = fixture.root.join("outside");
    fs::create_dir_all(&outside_root).expect("outside root");
    let outside = outside_root.join("AGENTS.md");
    fs::write(&outside, anchored("# Outside", "## Stale", "## Tail", "\n")).expect("outside file");
    let before = digest(&outside);

    let error = execute_post_commit(&fixture.request(vec![target(
        "codex",
        InstructionLanguage::En,
        &outside,
    )]))
    .expect_err("a target outside allowed roots must be rejected");
    assert!(
        matches!(error, PostCommitError::Job(_)),
        "containment must be enforced by the native transaction layer: {error:?}"
    );
    assert_eq!(digest(&outside), before);
}

#[test]
fn managed_instruction_cli_reports_every_host_status() {
    let fixture = Fixture::new("cli");
    let codex = fixture.host_file(".codex/AGENTS.md");
    let claude = fixture.host_file(".claude/CLAUDE.md");
    fs::write(&codex, anchored("# Codex", "## Stale", "## Tail", "\n")).expect("codex fixture");
    fs::write(&claude, "# Claude\n").expect("claude fixture");
    let zcode = fixture.home().join(".zcode/AGENTS.md");
    let repo = fixture.repo();
    let home = fixture.home();

    let run = |json: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ae-sdd-build"));
        command
            .arg("post-commit")
            .arg("--repository-root")
            .arg(&repo)
            .arg("--source")
            .arg(repo.join("source"))
            .arg("--package")
            .arg(repo.join("dist/ae-sdd"))
            .arg("--target")
            .arg(home.join(".codex/skills/ae-sdd"))
            .arg("--allowed-root")
            .arg(&repo)
            .arg("--allowed-root")
            .arg(&home)
            .args(["--commit", COMMIT])
            .arg("--codex-instructions")
            .arg(&codex)
            .arg("--claude-instructions")
            .arg(&claude)
            .arg("--zcode-instructions")
            .arg(&zcode);
        if json {
            command.arg("--json");
        }
        command.output().expect("post-commit CLI")
    };

    let human = run(false);
    assert!(human.status.success());
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("managed instructions:"), "{stdout}");
    assert!(stdout.contains("codex=updated"), "{stdout}");
    assert!(stdout.contains("claude=missing-anchor"), "{stdout}");
    assert!(stdout.contains("zcode=missing-target"), "{stdout}");

    let json = run(true);
    assert!(json.status.success());
    let execution: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("post-commit JSON");
    let managed = execution["managedInstructions"]
        .as_array()
        .expect("managedInstructions array");
    assert_eq!(managed.len(), 3);
    assert_eq!(managed[0]["host"], "claude");
    assert_eq!(managed[0]["status"], "missing-anchor");
    assert_eq!(managed[1]["host"], "codex");
    assert_eq!(managed[1]["status"], "unchanged");
    assert_eq!(managed[2]["host"], "zcode");
    assert_eq!(managed[2]["status"], "missing-target");
}
