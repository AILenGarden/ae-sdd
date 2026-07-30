use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use ae_sdd_domain::StoryId;
use ae_sdd_runtime::{RuntimeError, RuntimeResult};
use serde_json::Value;

use super::{
    contracts::{
        automation_enabled, context_complete, document_storage_compliant, http_contract_valid,
        nonempty_object, path_compliance_recorded, plan_contract_complete, plan_story_aligned,
        route_exempt, source_trace_complete, structured_coding_result, structured_status,
        structured_test_evidence, traceability_symmetric,
    },
    key::{
        GateContext, LocatedState, ReviewAuthorityDenial, active_story, safe_document_path,
        workspace_inputs,
    },
};

/// One predicate outcome plus the structured reason an authoritative Review
/// predicate refused. `denial` is only populated when `satisfied` is false.
pub(super) struct PredicateVerdict {
    pub(super) satisfied: bool,
    pub(super) denial: Option<ReviewAuthorityDenial>,
}

impl PredicateVerdict {
    const fn plain(satisfied: bool) -> Self {
        Self {
            satisfied,
            denial: None,
        }
    }

    /// Converts a Review authority denial into a failing predicate. A validator
    /// error is never allowed to surface as PASS.
    fn review(denial: Option<ReviewAuthorityDenial>) -> Self {
        Self {
            satisfied: denial.is_none(),
            denial,
        }
    }
}

pub(super) fn predicate_value(
    predicate: &str,
    context: &GateContext,
    located: &LocatedState,
) -> RuntimeResult<PredicateVerdict> {
    let root = context.root.as_path();
    let state = &located.value;
    let work_item = context.work_item_id.as_str();
    let story = active_story(state, work_item).map(|id| id.to_string());
    let plan = state.get("executionPlan").filter(|value| value.is_object());
    let value = match predicate {
        "project.assets.complete" => project_assets_complete(root),
        "document.dr.exists" => document_exists(root, state, story.as_deref(), "DR"),
        "document.story.exists" => document_exists(root, state, story.as_deref(), "Story"),
        "review.story.passed" => {
            structured_status(state.get("storyReview"), "passed")
                || route_story_committed(root, state, work_item)
        }
        "document.testcase.exists" => {
            verification_matrix(root, state, story.as_deref())
                || document_exists(root, state, story.as_deref(), "TestCase")
        }
        "document.task.exists" => document_exists(root, state, story.as_deref(), "Task"),
        "review.task.passed" => structured_status(state.get("taskReview"), "passed"),
        "coding_plan.exists" => plan.is_some_and(nonempty_object),
        // Beyond the plan contract itself, an active Story forces Story-AC
        // coverage: every AC the Story declares must appear in the plan
        // verification matrix. Story-less routes (micro/small scale) skip the
        // coverage check so they are not blocked by a document they never had.
        "coding_plan.fourteen_gates.complete" => {
            plan.is_some_and(plan_contract_complete)
                && (story.is_none() || plan_story_aligned(root, state, plan, story.as_deref()))
        }
        "http.scenario_manifest.valid" => plan.is_some_and(http_contract_valid),
        "test.evidence.exists" => structured_test_evidence(state, root),
        "coding.result.exists" => structured_coding_result(state),
        "review.findings.recorded" => {
            return Ok(PredicateVerdict::review(
                context.review_authority_denial(located),
            ));
        }
        "traceability.full_chain.symmetric" => traceability_symmetric(state, plan),
        "coding_plan.story.aligned" => plan_story_aligned(root, state, plan, story.as_deref()),
        "coding_plan.source_trace.complete" => {
            plan.is_some_and(|value| source_trace_complete(root, value))
        }
        "document.storage.compliant" => document_storage_compliant(root, state),
        "source.output_paths.compliant" => path_compliance_recorded(state),
        "document.ra.exists_or_exempt" => {
            document_exists(root, state, story.as_deref(), "RA") || route_exempt(state)
        }
        "ra.dimensions.complete" => {
            ra_text(root, state, story.as_deref()).is_some_and(|text| ra_dimensions_complete(&text))
        }
        "ra.derivatives.complete" => ra_text(root, state, story.as_deref())
            .is_some_and(|text| ra_derivatives_complete(&text)),
        "memory.configuration_path.consistent" => memory_paths_consistent(root),
        "review.loop.exit_satisfied" | "review.independence.valid" | "review.depth.valid" => {
            return Ok(PredicateVerdict::review(
                context.review_authority_denial(located),
            ));
        }
        "review.automation_consensus.valid_or_exempt" => {
            if !automation_enabled(state) {
                return Ok(PredicateVerdict::plain(true));
            }
            return Ok(PredicateVerdict::review(
                context.review_authority_denial(located),
            ));
        }
        "context.dr.complete" => dr_context_complete(root, state, work_item),
        "context.story.complete" => story_context_complete(root, state, work_item),
        "context.testcase.complete" => context_complete(state, &["story", "constraints", "assets"]),
        "context.task.complete" => {
            route_exempt(state) || context_complete(state, &["story", "constraints"])
        }
        _ => {
            return Err(RuntimeError::new(
                ae_sdd_protocol::StableErrorCode::GateError,
                "Gate predicate is not implemented",
            ));
        }
    };
    Ok(PredicateVerdict::plain(value))
}

fn project_assets_complete(root: &Path) -> bool {
    let count = fs::read_dir(root.join("constraints"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count();
    count >= 5
        && ["Cargo.toml", "pyproject.toml", "package.json"]
            .iter()
            .any(|name| root.join(name).is_file())
}

fn dr_context_complete(root: &Path, state: &Value, work_item: &str) -> bool {
    project_manifest_exists(root)
        && indexed_constraints_complete(root)
        && standards_complete(root)
        && bound_document_exists(root, state, "RA")
        && (is_route_state(state, work_item) || bound_document_exists(root, state, "PRD"))
}

fn project_manifest_exists(root: &Path) -> bool {
    ["Cargo.toml", "pyproject.toml", "package.json"]
        .iter()
        .any(|name| safe_document_path(root, name))
}

fn indexed_constraints_complete(root: &Path) -> bool {
    let directory = root.join("constraints");
    if !safe_document_path(root, "constraints/README.md") {
        return false;
    }
    let Ok(index) = fs::read_to_string(directory.join("README.md")) else {
        return false;
    };
    let indexed: Vec<_> = index
        .split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|entry| {
            let path = entry.strip_prefix("constraints/").unwrap_or(entry);
            (path.ends_with(".md") && !path.contains(['/', '\\']) && path != "README.md")
                .then_some(path)
        })
        .collect();
    !indexed.is_empty()
        && indexed
            .iter()
            .all(|path| safe_document_path(root, &format!("constraints/{path}")))
}

fn standards_complete(root: &Path) -> bool {
    fs::read_dir(root.join("source/standards"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| safe_document_path(root, &entry.path().to_string_lossy()))
}

fn bound_document_exists(root: &Path, state: &Value, kind: &str) -> bool {
    let directory = match kind {
        "STORY" => "Story",
        _ => kind,
    };
    state
        .pointer(&format!("/documentPaths/{kind}"))
        .and_then(Value::as_str)
        .is_some_and(|value| {
            let path = Path::new(value);
            let mut components = path.components();
            !path.is_absolute()
                && components
                    .next()
                    .is_some_and(|part| part.as_os_str() == "ae-sdd-doc")
                && components
                    .next()
                    .is_some_and(|part| part.as_os_str() == directory)
                && components.all(|part| matches!(part, std::path::Component::Normal(_)))
                && path.extension().is_some_and(|extension| extension == "md")
                && canonical_document_in_kind(root, path, directory)
        })
}

fn canonical_document_in_kind(root: &Path, path: &Path, kind: &str) -> bool {
    let canonical_root = root.canonicalize();
    let canonical_kind = root.join("ae-sdd-doc").join(kind).canonicalize();
    let canonical_document = root.join(path).canonicalize();
    canonical_root.is_ok_and(|root| {
        canonical_kind.is_ok_and(|kind| {
            kind.starts_with(&root)
                && canonical_document
                    .is_ok_and(|document| document.starts_with(kind) && document.is_file())
        })
    })
}

fn is_route_state(state: &Value, work_item: &str) -> bool {
    state.get("entryNode").and_then(Value::as_str) == Some("ROUTE")
        && state.get("stateMachineName").and_then(Value::as_str) == Some(work_item)
        && work_item.strip_prefix("ROUTE-").is_some_and(|suffix| {
            suffix.len() == 8
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn route_story_committed(root: &Path, state: &Value, work_item: &str) -> bool {
    is_route_state(state, work_item)
        && route_document_committed(state, "STORY")
        && bound_document_exists(root, state, "STORY")
        && bound_story_authority(state)
}

fn bound_story_authority(state: &Value) -> bool {
    let Some(story_id) = state.get("activeStory").and_then(Value::as_str) else {
        return false;
    };
    story_id.starts_with("STORY-")
        && StoryId::new(story_id.to_owned()).is_ok()
        && state
            .pointer(&format!("/storyStates/{story_id}/docPath"))
            .and_then(Value::as_str)
            == state
                .pointer("/documentPaths/STORY")
                .and_then(Value::as_str)
}

fn story_context_complete(root: &Path, state: &Value, work_item: &str) -> bool {
    if !is_route_state(state, work_item) {
        return context_complete(state, &["constraints", "assets", "sourceTrace"]);
    }
    let upstream_complete = route_document_committed(state, "RA")
        && bound_document_exists(root, state, "RA")
        && match state.get("selectedDesign").and_then(Value::as_str) {
            Some("DR") => {
                route_document_committed(state, "DR") && bound_document_exists(root, state, "DR")
            }
            Some("STORY") => true,
            _ => false,
        };
    project_manifest_exists(root)
        && indexed_constraints_complete(root)
        && standards_complete(root)
        && upstream_complete
}

fn route_document_committed(state: &Value, kind: &str) -> bool {
    state
        .pointer(&format!("/routeDocuments/{kind}"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn document_exists(root: &Path, state: &Value, story: Option<&str>, kind: &str) -> bool {
    if kind == "Story"
        && story
            .and_then(|id| state.pointer(&format!("/storyStates/{id}/docPath")))
            .and_then(Value::as_str)
            .is_some_and(|path| safe_document_path(root, path))
    {
        return true;
    }
    let needle = kind.to_ascii_lowercase();
    state
        .get("documentPaths")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(_, value)| value.as_str())
        .any(|path| path.to_ascii_lowercase().contains(&needle) && safe_document_path(root, path))
        || workspace_inputs(root).is_ok_and(|files| {
            files.into_iter().any(|(path, _)| {
                let lower = path.to_ascii_lowercase();
                lower.ends_with(".md")
                    && (lower.contains(&format!("ae-sdd-doc/{needle}/"))
                        || lower.contains(&format!("/{needle}/")))
                    && story.is_none_or(|id| {
                        lower.contains(&id.to_ascii_lowercase()) || kind == "RA" || kind == "DR"
                    })
            })
        })
}

fn verification_matrix(root: &Path, state: &Value, story: Option<&str>) -> bool {
    story_document(root, state, story)
        .and_then(|path| fs::read_to_string(path).ok())
        .is_some_and(|text| {
            text.contains("verification") || text.contains("验证矩阵") || text.contains("AC-")
        })
}

pub(super) fn story_document(root: &Path, state: &Value, story: Option<&str>) -> Option<PathBuf> {
    let story = story?;
    state
        .pointer(&format!("/storyStates/{story}/docPath"))
        .and_then(Value::as_str)
        .filter(|value| safe_document_path(root, value))
        // `safe_document_path` validates `root.join(value)`, so the resolved path
        // must be returned too. Returning the raw relative `docPath` would make
        // callers read it against the process working directory, which silently
        // yields no document and lets a Gate fail for the wrong reason.
        .map(|value| {
            let path = Path::new(value);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        })
        .or_else(|| {
            workspace_inputs(root)
                .ok()?
                .into_iter()
                .find_map(|(relative, absolute)| {
                    (relative.to_ascii_lowercase().contains("ae-sdd-doc/story/")
                        && relative
                            .to_ascii_lowercase()
                            .contains(&story.to_ascii_lowercase()))
                    .then_some(absolute)
                })
        })
}

fn ra_text(root: &Path, state: &Value, story: Option<&str>) -> Option<String> {
    workspace_inputs(root)
        .ok()?
        .into_iter()
        .find_map(|(relative, absolute)| {
            let lower = relative.to_ascii_lowercase();
            (lower.contains("ae-sdd-doc/ra/")
                && story.is_none_or(|id| {
                    lower.contains(&id.trim_start_matches("STORY-").to_ascii_lowercase())
                }))
            .then(|| fs::read_to_string(absolute).ok())
            .flatten()
        })
        .or_else(|| document_exists(root, state, story, "RA").then(String::new))
}

fn ra_dimensions_complete(text: &str) -> bool {
    text.lines()
        .filter(|line| line.trim_start().starts_with("##"))
        .count()
        >= 8
        && text.contains("RequirementAnalysisModel")
}

fn ra_derivatives_complete(text: &str) -> bool {
    text.contains("RA-G")
        && text.contains("RequirementAnalysisModel")
        && text.matches("##").count() >= 10
}

fn memory_paths_consistent(root: &Path) -> bool {
    let config = root.join(".ae-sdd/config.json");
    if !config.is_file() {
        return true;
    }
    fs::read(config)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| {
            value
                .get("memoryPath")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .is_some_and(|path| !Path::new(&path).is_absolute() && root.join(path).exists())
}

/// Extracts the AC ids a document declares. Accepts every `AC-` token whose
/// remainder carries at least one digit, so both numeric (`AC-1`, `AC-001`)
/// and descriptive (`AC-NAME-01`) conventions are honored; pure-letter prose
/// such as `AC-DC` is not an id and stays excluded.
pub(super) fn ac_ids(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
        .filter(|token| {
            token.strip_prefix("AC-").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().any(|c| c.is_ascii_digit())
            })
        })
        .map(str::to_owned)
        .collect()
}
