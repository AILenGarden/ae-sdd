use std::fs;
use std::path::PathBuf;

use ae_sdd_build::{SourceSlimError, SourceSlimMode, SourceSlimRequest, execute_source_slim};
use tempfile::TempDir;

fn source_fixture() -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("temporary source root");
    let source_root = temp.path().join("source");
    fs::create_dir_all(source_root.join("skills/phase1-design")).expect("skill directory");
    fs::create_dir_all(source_root.join("skill-fallbacks/skills/phase1-design"))
        .expect("fallback directory");
    fs::create_dir_all(source_root.join("templates/skill")).expect("template directory");
    fs::write(
        source_root.join("templates/skill/source-skill-slim-entry-template.md"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../source/templates/skill/source-skill-slim-entry-template.md"
        )),
    )
    .expect("source slim template");
    (temp, source_root)
}

fn write_fixture_entry(source_root: &std::path::Path) -> PathBuf {
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    fs::write(
        source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md"),
        "---\nname: example\ndescription: A source fallback used by the test.\n---\n\n# Example Skill\n\nUse when the route needs a test fixture.\n\n## Workflow\n\nRoute through Requirement Analysis first.\n\n`ae-sdd flow next`\n",
    )
    .expect("fallback");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("stale slim entry");
    skill
}

#[test]
fn refresh_renders_from_fallback_and_validate_requires_exact_bytes() {
    let (_temp, source_root) = source_fixture();
    let skill = write_fixture_entry(&source_root);
    let relative = PathBuf::from("skills/phase1-design/example-skill.md");

    let refreshed = execute_source_slim(&SourceSlimRequest {
        source_root: source_root.clone(),
        skills: vec![relative.clone()],
        mode: SourceSlimMode::Refresh,
    })
    .expect("refresh succeeds");
    assert_eq!(refreshed.entries.len(), 1);
    assert!(refreshed.entries[0].changed);

    let rendered = fs::read_to_string(&skill).expect("rendered slim entry");
    assert!(rendered.contains("source_slim_schema: ae-sdd-source-slim/v2"));
    assert!(rendered.contains("# Example Skill Source SKILL Slim Entry"));
    assert!(
        rendered.contains(
            "This source SKILL has been slimmed by the standard source-slimming pipeline."
        )
    );
    assert!(rendered.contains(
        "Refresh from the fallback with `ae-sdd-build source-slim --source source --skill skills/phase1-design/example-skill.md --refresh`."
    ));
    assert!(rendered.contains(
        "Validate canonical rendered bytes with `ae-sdd-build source-slim --source source --skill skills/phase1-design/example-skill.md --validate`."
    ));
    assert!(
        rendered.contains(
            "1. Read the full source or the recorded fallback as the only semantic input."
        )
    );
    assert!(!rendered.contains("# stale entry"));

    let validated = execute_source_slim(&SourceSlimRequest {
        source_root: source_root.clone(),
        skills: vec![relative],
        mode: SourceSlimMode::Validate,
    })
    .expect("freshly rendered entry validates");
    assert!(!validated.entries[0].changed);

    fs::write(&skill, format!("{rendered}\n")).expect("tamper rendered entry");
    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Validate,
    })
    .expect_err("byte drift must fail validation");
    assert!(matches!(error, SourceSlimError::RenderedMismatch { .. }));
}

#[test]
fn refresh_rejects_a_skill_path_that_escapes_the_source_root() {
    let (_temp, source_root) = source_fixture();

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("../outside.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("path traversal is rejected");
    assert!(matches!(error, SourceSlimError::InvalidSkillPath { .. }));
}

#[test]
fn refresh_rejects_a_fallback_that_escapes_the_source_root() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: ../outside.md\n---\n\n# stale entry\n",
    )
    .expect("slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("fallback traversal is rejected");
    assert!(matches!(error, SourceSlimError::InvalidFallbackPath { .. }));
}

#[test]
fn refresh_rejects_a_fallback_path_with_a_windows_ads_separator() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md:alternate\n---\n\n# stale entry\n",
    )
    .expect("slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("an ADS separator is not a portable fallback path");

    assert!(matches!(error, SourceSlimError::InvalidFallbackPath { .. }));
}

#[test]
fn refresh_preflights_all_entries_before_writing_any_entry() {
    let (_temp, source_root) = source_fixture();
    let valid_skill = write_fixture_entry(&source_root);
    let original = fs::read_to_string(&valid_skill).expect("stale entry");
    let invalid_skill = source_root.join("skills/phase1-design/invalid-skill.md");
    fs::write(
        &invalid_skill,
        "---\nname: invalid\nsource_slimmed: true\n---\n\n# invalid entry\n",
    )
    .expect("invalid slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![
            PathBuf::from("skills/phase1-design/example-skill.md"),
            PathBuf::from("skills/phase1-design/invalid-skill.md"),
        ],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("an invalid later entry rejects the refresh");

    assert!(matches!(error, SourceSlimError::MissingFallback { .. }));
    assert_eq!(
        fs::read_to_string(valid_skill).expect("valid entry remains readable"),
        original,
        "refresh must finish validation before writing any selected entry"
    );
}

#[test]
fn validate_uses_canonical_utf8_content_across_line_endings() {
    let (_temp, source_root) = source_fixture();
    let skill = write_fixture_entry(&source_root);
    let fallback = source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md");
    let fallback_text = fs::read_to_string(&fallback).expect("fallback");
    fs::write(&fallback, fallback_text.replace('\n', "\r\n")).expect("CRLF fallback");
    let relative = PathBuf::from("skills/phase1-design/example-skill.md");

    execute_source_slim(&SourceSlimRequest {
        source_root: source_root.clone(),
        skills: vec![relative.clone()],
        mode: SourceSlimMode::Refresh,
    })
    .expect("refresh canonicalizes renderer input");

    let rendered = fs::read_to_string(&skill).expect("rendered entry");
    fs::write(&skill, rendered.replace('\n', "\r\n")).expect("CRLF rendered entry");

    execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![relative],
        mode: SourceSlimMode::Validate,
    })
    .expect("equivalent CRLF entry validates against canonical rendered bytes");
}

#[test]
fn refresh_rejects_a_fallback_outside_skill_fallbacks() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    let fallback = source_root.join("other/full.md");
    fs::create_dir_all(fallback.parent().expect("fallback parent")).expect("fallback directory");
    fs::write(
        &fallback,
        "---\nname: other\n---\n\n# Other fallback\n\nComplete source.\n",
    )
    .expect("other fallback");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: other/full.md\n---\n\n# stale entry\n",
    )
    .expect("slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("fallbacks outside skill-fallbacks are not semantic sources");

    assert!(matches!(
        error,
        SourceSlimError::UnsupportedFallbackPath { .. }
    ));
}

#[cfg(windows)]
#[test]
fn refresh_rejects_a_fallback_symlink_inside_skill_fallbacks() {
    use std::os::windows::fs::symlink_file;

    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    let actual_fallback =
        source_root.join("skill-fallbacks/skills/phase1-design/actual-example-skill.full.md");
    fs::write(
        &actual_fallback,
        "---\nname: other\n---\n\n# Other fallback\n\nComplete source.\n",
    )
    .expect("other fallback");
    let declared_fallback =
        source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md");
    if let Err(error) = symlink_file(&actual_fallback, &declared_fallback) {
        if error.raw_os_error() == Some(1314) {
            eprintln!(
                "skipping symlink containment test: Windows symlink privilege is unavailable"
            );
            return;
        }
        panic!("fallback symlink: {error}");
    }
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a fallback symlink cannot alias another fallback");

    assert!(matches!(
        error,
        SourceSlimError::FallbackContainsLink { .. }
    ));
}

#[cfg(windows)]
#[test]
fn refresh_rejects_a_source_entry_symlink_inside_skills() {
    use std::os::windows::fs::symlink_file;

    let (_temp, source_root) = source_fixture();
    let full_fallback =
        source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md");
    fs::write(
        &full_fallback,
        "---\nname: example\n---\n\n# Complete source\n\nFull semantics.\n",
    )
    .expect("full fallback");
    let redirected_entry = source_root.join("skills/phase1-design/redirected-entry.md");
    fs::write(
        &redirected_entry,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("redirected slim entry");
    let source_entry = source_root.join("skills/phase1-design/example-skill.md");
    if let Err(error) = symlink_file(&redirected_entry, &source_entry) {
        if error.raw_os_error() == Some(1314) {
            eprintln!(
                "skipping symlink containment test: Windows symlink privilege is unavailable"
            );
            return;
        }
        panic!("source entry symlink: {error}");
    }

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a source entry symlink cannot alias another skills entry");

    assert!(matches!(
        error,
        SourceSlimError::SourceEntryContainsLink { .. }
    ));
}

#[test]
fn refresh_rejects_a_self_referential_fallback() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skills/phase1-design/example-skill.md\n---\n\n# stale entry\n",
    )
    .expect("self-referential slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("an entry cannot use itself as its fallback");

    assert!(matches!(
        error,
        SourceSlimError::SelfReferentialFallback { .. }
    ));
}

#[test]
fn refresh_rejects_an_already_slimmed_fallback() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    let fallback = source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md");
    fs::write(
        &fallback,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# Slim fallback\n",
    )
    .expect("slimmed fallback");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("slim entry");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a slimmed fallback cannot be re-slimmed");

    assert!(matches!(error, SourceSlimError::SlimmedFallback { .. }));
}

#[test]
fn refresh_uses_the_source_template_as_the_rendering_authority() {
    let (_temp, source_root) = source_fixture();
    let skill = write_fixture_entry(&source_root);
    let template = source_root.join("templates/skill/source-skill-slim-entry-template.md");
    let source = fs::read_to_string(&template).expect("template");
    fs::write(
        template,
        source.replace(
            "This source SKILL has been slimmed by the standard source-slimming pipeline.",
            "Template authority marker.",
        ),
    )
    .expect("updated template");

    execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect("refresh succeeds");

    assert!(
        fs::read_to_string(skill)
            .expect("rendered entry")
            .contains("Template authority marker."),
        "the generated entry must contain the source template's static content"
    );
}

#[test]
fn refresh_rejects_a_template_without_a_required_slot() {
    let (_temp, source_root) = source_fixture();
    write_fixture_entry(&source_root);
    let template = source_root.join("templates/skill/source-skill-slim-entry-template.md");
    let source = fs::read_to_string(&template).expect("template");
    fs::write(&template, source.replace("{title}", "title")).expect("template without title slot");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("template slot loss must fail closed");

    assert!(matches!(error, SourceSlimError::TemplateInvalid { .. }));
}

#[test]
fn refresh_rejects_a_template_without_the_slimming_sop_section() {
    let (_temp, source_root) = source_fixture();
    write_fixture_entry(&source_root);
    let template = source_root.join("templates/skill/source-skill-slim-entry-template.md");
    let source = fs::read_to_string(&template).expect("template");
    fs::write(
        &template,
        source.replace("## Source Slimming SOP", "## Removed SOP"),
    )
    .expect("template without SOP");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("template structure loss must fail closed");

    assert!(matches!(error, SourceSlimError::TemplateInvalid { .. }));
}

#[test]
fn refresh_rejects_a_template_with_a_duplicate_semantic_row_marker() {
    let (_temp, source_root) = source_fixture();
    let skill = write_fixture_entry(&source_root);
    let template = source_root.join("templates/skill/source-skill-slim-entry-template.md");
    let source = fs::read_to_string(&template).expect("template");
    let marker = "| identity_trigger | {detected evidence} | {design docs} | {fallback rule} |";
    fs::write(
        &template,
        source.replace(marker, &format!("{marker}\n{marker}")),
    )
    .expect("template with duplicate semantic row marker");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a duplicate template row marker must fail closed");

    assert!(matches!(error, SourceSlimError::TemplateInvalid { .. }));
    assert!(
        fs::read_to_string(skill)
            .expect("stale entry")
            .contains("# stale entry"),
        "invalid templates must not rewrite the source entry"
    );
}

#[test]
fn refresh_rejects_a_row_field_placeholder_outside_its_template_row() {
    let (_temp, source_root) = source_fixture();
    write_fixture_entry(&source_root);
    let template = source_root.join("templates/skill/source-skill-slim-entry-template.md");
    let source = fs::read_to_string(&template).expect("template");
    fs::write(
        &template,
        source.replace("## Load Contract", "{level}\n## Load Contract"),
    )
    .expect("template with orphaned row field");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a template must not emit an unexpanded placeholder");

    assert!(matches!(error, SourceSlimError::TemplateInvalid { .. }));
}

#[test]
fn refresh_rejects_duplicate_frontmatter_keys() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("duplicate slim entry");
    fs::write(
        source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md"),
        "---\nname: example\n---\n\n# Example\n",
    )
    .expect("fallback");

    assert!(
        execute_source_slim(&SourceSlimRequest {
            source_root,
            skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
            mode: SourceSlimMode::Refresh,
        })
        .is_err(),
        "duplicate metadata must fail closed instead of using a last-key-wins value"
    );
}

#[test]
fn refresh_preserves_non_generated_source_frontmatter() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    fs::write(
        source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md"),
        "---\nname: example\nsource_identity: durable-contract\ndescription: preserves semantic metadata\n---\n\n# Example\n",
    )
    .expect("fallback");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("stale entry");

    execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect("refresh succeeds");

    assert!(
        fs::read_to_string(skill)
            .expect("rendered entry")
            .contains("source_identity: durable-contract"),
        "only generated source-slim metadata may be removed"
    );
}

#[test]
fn refresh_handles_utf8_summaries_and_counts_trailing_newlines_as_no_extra_line() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("skills/phase1-design/example-skill.md");
    let paragraph = "测".repeat(600);
    let fallback = format!("---\nname: example\n---\n\n{paragraph}\n");
    let expected_lines = fallback.lines().count();
    fs::write(
        source_root.join("skill-fallbacks/skills/phase1-design/example-skill.full.md"),
        &fallback,
    )
    .expect("fallback");
    fs::write(
        &skill,
        "---\nname: example\nsource_slimmed: true\nsource_fallback: skill-fallbacks/skills/phase1-design/example-skill.full.md\n---\n\n# stale entry\n",
    )
    .expect("stale entry");

    execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect("UTF-8 content must not panic");

    let rendered = fs::read_to_string(skill).expect("rendered entry");
    assert!(rendered.contains(&format!("source_original_lines: {expected_lines}")));
}

#[test]
fn refresh_bootstraps_the_root_skill_only_from_its_fixed_fallback() {
    let (_temp, source_root) = source_fixture();
    let skill = source_root.join("SKILL.md");
    fs::write(&skill, "---\nname: root\n---\n\n# Existing root contract\n").expect("root entry");
    fs::write(
        source_root.join("skill-fallbacks/SKILL.full.md"),
        "---\nname: root\ndescription: Root fallback\n---\n\n# Root contract\n",
    )
    .expect("root fallback");

    execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("SKILL.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect("root bootstrap succeeds");

    let rendered = fs::read_to_string(skill).expect("rendered root entry");
    assert!(rendered.contains("source_slimmed: true"));
    assert!(rendered.contains("source_fallback: skill-fallbacks/SKILL.full.md"));
}

#[test]
fn refresh_rejects_a_catalog_fallback_that_differs_from_slim_metadata() {
    let (_temp, source_root) = source_fixture();
    write_fixture_entry(&source_root);
    let catalog = source_root.join("standards/runtime/methodology-catalog.v1.json");
    fs::create_dir_all(catalog.parent().expect("catalog parent")).expect("catalog directory");
    fs::write(
        catalog,
        r#"{
  "entries": [
    {
      "compactRef": "skills/phase1-design/example-skill.md",
      "fallbackRef": "skill-fallbacks/skills/phase1-design/other-skill.full.md"
    }
  ]
}"#,
    )
    .expect("catalog");

    assert!(
        execute_source_slim(&SourceSlimRequest {
            source_root,
            skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
            mode: SourceSlimMode::Refresh,
        })
        .is_err(),
        "runtime catalog fallback must be bound to the slim entry fallback"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn refresh_rejects_a_symlinked_optional_catalog_instead_of_treating_it_as_absent() {
    let (_temp, source_root) = source_fixture();
    write_fixture_entry(&source_root);
    let catalog = source_root.join("standards/runtime/methodology-catalog.v1.json");
    fs::create_dir_all(catalog.parent().expect("catalog parent")).expect("catalog directory");
    let external_catalog = source_root
        .parent()
        .expect("source parent")
        .join("external-catalog.json");
    fs::write(&external_catalog, "{\"entries\": []}\n").expect("external catalog");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&external_catalog, &catalog).expect("catalog symlink");
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_file(&external_catalog, &catalog) {
        if error.raw_os_error() == Some(1314) {
            eprintln!("skipping catalog symlink test: symlink privilege is unavailable");
            return;
        }
        panic!("catalog symlink: {error}");
    }

    let error = execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a linked optional catalog must fail closed");

    assert!(matches!(
        error,
        SourceSlimError::SupportingPathContainsLink { .. }
    ));
}

#[cfg(unix)]
#[test]
fn refresh_preserves_existing_source_entry_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let (_temp, source_root) = source_fixture();
    let skill = write_fixture_entry(&source_root);
    fs::set_permissions(&skill, fs::Permissions::from_mode(0o640)).expect("source permissions");

    execute_source_slim(&SourceSlimRequest {
        source_root,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect("refresh succeeds");

    assert_eq!(
        fs::metadata(skill)
            .expect("source metadata")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
}

#[cfg(unix)]
#[test]
fn refresh_rejects_a_symlinked_source_root() {
    use std::os::unix::fs::symlink;

    let (_temp, source_root) = source_fixture();
    let redirected = source_root
        .parent()
        .expect("source parent")
        .join("source-link");
    symlink(&source_root, &redirected).expect("source root symlink");

    let error = execute_source_slim(&SourceSlimRequest {
        source_root: redirected,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a symlinked source root is rejected");

    assert!(matches!(
        error,
        SourceSlimError::SourceRootContainsLink { .. }
    ));
}

#[cfg(windows)]
#[test]
fn refresh_rejects_a_reparse_point_source_root() {
    use std::os::windows::fs::symlink_dir;

    let (_temp, source_root) = source_fixture();
    let redirected = source_root
        .parent()
        .expect("source parent")
        .join("source-link");
    if let Err(error) = symlink_dir(&source_root, &redirected) {
        if error.raw_os_error() == Some(1314) {
            eprintln!("skipping reparse-point test: symlink privilege is unavailable");
            return;
        }
        panic!("source root reparse point: {error}");
    }

    let error = execute_source_slim(&SourceSlimRequest {
        source_root: redirected,
        skills: vec![PathBuf::from("skills/phase1-design/example-skill.md")],
        mode: SourceSlimMode::Refresh,
    })
    .expect_err("a reparse-point source root is rejected");

    assert!(matches!(
        error,
        SourceSlimError::SourceRootContainsLink { .. }
    ));
}
