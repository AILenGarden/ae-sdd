use super::*;

const SSOT: &str = concat!(
    "# Header\n",
    "\n",
    "<!-- SECTION:zh -->\n",
    "## 中文纪律\n",
    "\n",
    "- 中文条目\n",
    "<!-- /SECTION:zh -->\n",
    "\n",
    "<!-- SECTION:en -->\n",
    "## English discipline\n",
    "\n",
    "- English item\n",
    "<!-- /SECTION:en -->\n",
);

const REVISION: &str = "0123456";

fn anchored_target(newline: &str) -> String {
    [
        "# Codex Global Instructions",
        "",
        "<!-- BEGIN ae-sdd-l2-ssot @ deadbee @ legacy (hash=000000000000 v=0.9.0) -->",
        "## Stale discipline",
        "<!-- END ae-sdd-l2-ssot -->",
        "",
        "## Skill Source",
        "",
        "- personal note",
        "",
    ]
    .join(newline)
}

fn render(target: &str, language: InstructionLanguage) -> ManagedInstructionPlan {
    render_managed_instruction(&ManagedInstructionRenderRequest {
        source: SSOT,
        target,
        language,
        revision: REVISION,
    })
    .expect("render must succeed for a well-formed fixture")
}

fn rendered_contents(plan: &ManagedInstructionPlan) -> &str {
    match plan {
        ManagedInstructionPlan::Updated { contents, .. } => contents,
        other => panic!("expected an update, received {other:?}"),
    }
}

#[test]
fn render_selects_the_requested_language_body_exactly() {
    let target = anchored_target("\n");
    let english = render(&target, InstructionLanguage::En);
    let chinese = render(&target, InstructionLanguage::Zh);
    let english = rendered_contents(&english);
    let chinese = rendered_contents(&chinese);

    assert!(english.contains("## English discipline"));
    assert!(!english.contains("## 中文纪律"));
    assert!(chinese.contains("## 中文纪律"));
    assert!(!chinese.contains("## English discipline"));
    // Section markers themselves never reach the host file.
    assert!(!english.contains("SECTION:en"));
    assert!(!chinese.contains("SECTION:zh"));
}

#[test]
fn render_replaces_only_the_anchor_span() {
    let target = anchored_target("\n");
    let plan = render(&target, InstructionLanguage::En);
    let contents = rendered_contents(&plan);

    let original_prefix = "# Codex Global Instructions\n\n";
    let original_suffix = "\n## Skill Source\n\n- personal note\n";
    assert!(contents.starts_with(original_prefix));
    assert!(contents.ends_with(original_suffix));
    assert!(!contents.contains("## Stale discipline"));
    assert!(!contents.contains("hash=000000000000"));
    assert_eq!(contents.matches("BEGIN ae-sdd-l2-ssot").count(), 1);
    assert_eq!(contents.matches("END ae-sdd-l2-ssot").count(), 1);
}

#[test]
fn render_preserves_lf_and_crlf_conventions() {
    let lf = anchored_target("\n");
    let crlf = anchored_target("\r\n");

    let lf_plan = render(&lf, InstructionLanguage::En);
    let lf_contents = rendered_contents(&lf_plan);
    assert!(!lf_contents.contains('\r'));

    let crlf_plan = render(&crlf, InstructionLanguage::En);
    let crlf_contents = rendered_contents(&crlf_plan);
    assert_eq!(
        crlf_contents.matches('\n').count(),
        crlf_contents.matches("\r\n").count(),
        "every newline in a CRLF target must stay CRLF"
    );
    assert!(crlf_contents.starts_with("# Codex Global Instructions\r\n"));
    assert!(crlf_contents.ends_with("\r\n- personal note\r\n"));
}

#[test]
fn render_reports_missing_anchor_without_proposing_a_change() {
    let target = "# Codex Global Instructions\n\n## Hand written\n";
    let plan = render_managed_instruction(&ManagedInstructionRenderRequest {
        source: SSOT,
        target,
        language: InstructionLanguage::En,
        revision: REVISION,
    })
    .expect("a target without anchors is a skip, not an error");
    assert_eq!(plan, ManagedInstructionPlan::MissingAnchor);
}

#[test]
fn render_rejects_malformed_anchors() {
    let cases = [
        (
            "unterminated",
            "# Head\n<!-- BEGIN ae-sdd-l2-ssot @ a -->\nbody\n",
            ManagedInstructionError::AnchorUnterminated,
        ),
        (
            "orphan-end",
            "# Head\n<!-- END ae-sdd-l2-ssot -->\n",
            ManagedInstructionError::AnchorUnterminated,
        ),
        (
            "duplicate-begin",
            concat!(
                "<!-- BEGIN ae-sdd-l2-ssot @ a -->\n",
                "one\n",
                "<!-- BEGIN ae-sdd-l2-ssot @ b -->\n",
                "two\n",
                "<!-- END ae-sdd-l2-ssot -->\n",
            ),
            ManagedInstructionError::AnchorDuplicated,
        ),
        (
            "duplicate-end",
            concat!(
                "<!-- BEGIN ae-sdd-l2-ssot @ a -->\n",
                "one\n",
                "<!-- END ae-sdd-l2-ssot -->\n",
                "<!-- END ae-sdd-l2-ssot -->\n",
            ),
            ManagedInstructionError::AnchorDuplicated,
        ),
        (
            "reversed",
            concat!(
                "<!-- END ae-sdd-l2-ssot -->\n",
                "body\n",
                "<!-- BEGIN ae-sdd-l2-ssot @ a -->\n",
            ),
            ManagedInstructionError::AnchorReversed,
        ),
    ];
    for (label, target, expected) in cases {
        let error = render_managed_instruction(&ManagedInstructionRenderRequest {
            source: SSOT,
            target,
            language: InstructionLanguage::En,
            revision: REVISION,
        })
        .expect_err(label);
        assert_eq!(error, expected, "case {label}");
    }
}

#[test]
fn render_rejects_malformed_source_sections() {
    let cases = [
        (
            "missing",
            "# Head\n<!-- SECTION:zh -->\nbody\n<!-- /SECTION:zh -->\n",
            ManagedInstructionError::SectionMissing("en"),
        ),
        (
            "unterminated",
            "# Head\n<!-- SECTION:en -->\nbody\n",
            ManagedInstructionError::SectionUnterminated("en"),
        ),
        (
            "duplicated",
            concat!(
                "<!-- SECTION:en -->\none\n<!-- /SECTION:en -->\n",
                "<!-- SECTION:en -->\ntwo\n<!-- /SECTION:en -->\n",
            ),
            ManagedInstructionError::SectionDuplicated("en"),
        ),
        (
            "empty",
            "<!-- SECTION:en -->\n\n\n<!-- /SECTION:en -->\n",
            ManagedInstructionError::SectionEmpty("en"),
        ),
    ];
    let target = anchored_target("\n");
    for (label, source, expected) in cases {
        let error = render_managed_instruction(&ManagedInstructionRenderRequest {
            source,
            target: &target,
            language: InstructionLanguage::En,
            revision: REVISION,
        })
        .expect_err(label);
        assert_eq!(error, expected, "case {label}");
    }
}

#[test]
fn render_is_deterministic_for_identical_inputs() {
    let target = anchored_target("\n");
    let first = render(&target, InstructionLanguage::En);
    let second = render(&target, InstructionLanguage::En);
    assert_eq!(first, second);
    let contents = rendered_contents(&first);
    assert!(
        contents.contains(&format!(
            "<!-- BEGIN {MANAGED_ANCHOR} @ {REVISION} @ {MANAGED_ADAPTER_LABEL} (hash="
        )),
        "the audit header must record the caller-supplied revision and adapter"
    );
    // A wall-clock field would break replay-stable receipts.
    assert!(!contents.contains('Z'), "{contents}");
}

#[test]
fn render_reports_unchanged_for_byte_identical_content() {
    let target = anchored_target("\n");
    let plan = render(&target, InstructionLanguage::En);
    let updated = rendered_contents(&plan).to_owned();
    let replay = render(&updated, InstructionLanguage::En);
    match replay {
        ManagedInstructionPlan::Unchanged { content_hash } => {
            assert_eq!(content_hash.len(), 12);
        }
        other => panic!("a byte-identical target must be unchanged, received {other:?}"),
    }
}

#[test]
fn render_hashes_the_language_body_not_the_whole_source() {
    let target = anchored_target("\n");
    let english = render(&target, InstructionLanguage::En);
    let chinese = render(&target, InstructionLanguage::Zh);
    let (english_hash, chinese_hash) = match (&english, &chinese) {
        (
            ManagedInstructionPlan::Updated {
                content_hash: left, ..
            },
            ManagedInstructionPlan::Updated {
                content_hash: right,
                ..
            },
        ) => (left, right),
        other => panic!("expected two updates, received {other:?}"),
    };
    assert_ne!(english_hash, chinese_hash);
    assert_eq!(english_hash.len(), 12);
    assert!(english_hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
}
