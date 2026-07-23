use std::{collections::BTreeSet, fs, path::Path};

use serde_json::Value;

use super::{
    key::safe_document_path,
    predicate::{ac_ids, story_document},
};

pub(super) fn plan_contract_complete(plan: &Value) -> bool {
    nonempty_string(plan.get("goal"))
        && nonempty_array(plan.get("changedPaths"))
        && plan
            .get("verification")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.len() >= 14
                    && items.iter().all(|item| {
                        ["id", "acId", "boundary", "command", "expected"]
                            .iter()
                            .all(|field| nonempty_string(item.get(*field)))
                    })
            })
        && nonempty_array(plan.get("risks"))
        && plan.get("approved").and_then(Value::as_bool) == Some(true)
}

pub(super) fn http_contract_valid(plan: &Value) -> bool {
    let http: Vec<_> = plan
        .get("verification")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| {
            item.get("boundary")
                .and_then(Value::as_str)
                .is_some_and(|value| value.to_ascii_lowercase().contains("http"))
        })
        .collect();
    http.is_empty()
        || http.iter().all(|item| {
            nonempty_string(item.get("command")) && nonempty_string(item.get("expected"))
        })
}

pub(super) fn plan_story_aligned(
    root: &Path,
    state: &Value,
    plan: Option<&Value>,
    story: Option<&str>,
) -> bool {
    let Some((plan, path)) = plan.zip(story_document(root, state, story)) else {
        return false;
    };
    if plan.get("approved").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    let story_acs = fs::read_to_string(path)
        .ok()
        .map_or_else(BTreeSet::new, |text| ac_ids(&text));
    let plan_acs: BTreeSet<_> = plan
        .get("verification")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("acId").and_then(Value::as_str).map(str::to_owned))
        .collect();
    !story_acs.is_empty() && story_acs.is_subset(&plan_acs)
}

pub(super) fn source_trace_complete(root: &Path, plan: &Value) -> bool {
    plan.get("sourceReads")
        .and_then(Value::as_array)
        .is_some_and(|reads| {
            !reads.is_empty()
                && reads.iter().filter_map(Value::as_str).any(|path| {
                    let candidate = Path::new(path);
                    (candidate.is_absolute() && candidate.is_file())
                        || root.join(candidate).is_file()
                })
        })
}

pub(super) fn structured_test_evidence(state: &Value, root: &Path) -> bool {
    let entries = state.get("evidence").and_then(Value::as_array);
    entries.is_some_and(|items| {
        !items.is_empty()
            && items.iter().all(|item| {
                item.is_object()
                    && (nonempty_string(item.get("digest"))
                        || nonempty_string(item.get("evidenceId")))
            })
    }) || (state.get("evidenceFinalized").and_then(Value::as_bool) == Some(true)
        && evidence_manifest_exists(root))
}

pub(super) fn structured_coding_result(state: &Value) -> bool {
    ["codingResult", "coding"]
        .iter()
        .filter_map(|field| state.get(*field))
        .any(|value| {
            value.is_object()
                && (nonempty_string(value.get("status")) || nonempty_array(value.get("artifacts")))
        })
}

pub(super) fn traceability_symmetric(state: &Value, plan: Option<&Value>) -> bool {
    let explicit = state
        .pointer("/traceability/symmetric")
        .and_then(Value::as_bool)
        == Some(true)
        && state
            .pointer("/traceability/links")
            .and_then(Value::as_array)
            .is_some_and(|links| !links.is_empty());
    let planned = plan.is_some_and(|value| {
        value
            .get("sourceReads")
            .and_then(Value::as_array)
            .is_some_and(|reads| {
                let values: Vec<_> = reads.iter().filter_map(Value::as_str).collect();
                ["/RA/", "/DR/", "/Story/"].iter().all(|kind| {
                    values
                        .iter()
                        .any(|path| path.replace('\\', "/").contains(kind))
                })
            })
            && value
                .get("verification")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    !items.is_empty()
                        && items.iter().all(|item| {
                            nonempty_string(item.get("acId")) && nonempty_string(item.get("id"))
                        })
                })
    });
    explicit || planned
}

pub(super) fn document_storage_compliant(root: &Path, state: &Value) -> bool {
    let mut paths: Vec<_> = state
        .get("documentPaths")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(_, value)| value.as_str())
        .collect();
    paths.extend(
        state
            .get("storyStates")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(_, story)| story.get("docPath").and_then(Value::as_str)),
    );
    !paths.is_empty() && paths.iter().all(|path| safe_document_path(root, path))
}

pub(super) fn path_compliance_recorded(state: &Value) -> bool {
    state.get("pathCompliance").is_some_and(|record| {
        nonempty_array(record.get("scannedPaths"))
            && record
                .get("findings")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
            && record
                .get("inputFingerprint")
                .and_then(Value::as_str)
                .is_some_and(valid_digest)
    })
}

pub(super) fn review_loop_passed(state: &Value) -> bool {
    [state.get("reviewLoop"), state.get("reviewSession")]
        .into_iter()
        .flatten()
        .any(|value| {
            structured_status(Some(value), "passed") || structured_status(Some(value), "completed")
        })
}

pub(super) fn reviewers_independent(state: &Value) -> bool {
    let reviewers = state
        .pointer("/review/reviewers")
        .or_else(|| state.get("activeReviewers"))
        .and_then(Value::as_array);
    reviewers.is_some_and(|items| {
        let sessions: BTreeSet<_> = items
            .iter()
            .filter(|item| {
                item.get("role")
                    .and_then(Value::as_str)
                    .is_none_or(|role| role.eq_ignore_ascii_case("reviewer"))
            })
            .filter_map(|item| item.get("sessionId").and_then(Value::as_str))
            .collect();
        !sessions.is_empty() && sessions.len() == items.len()
    })
}

pub(super) fn review_recorded(review: &Value) -> bool {
    review
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status != "pending")
        && review.get("findings").is_some_and(Value::is_array)
}

pub(super) fn review_depth_valid(review: &Value) -> bool {
    let findings = review.get("findings").and_then(Value::as_array);
    review_recorded(review)
        && findings.is_some_and(|items| {
            !items.is_empty()
                || (nonempty_string(review.get("zeroFindingsRationale"))
                    && nonempty_array(review.get("evidenceIds")))
        })
}

pub(super) fn automation_consensus(state: &Value) -> bool {
    if state
        .pointer("/automation/enabled")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return true;
    }
    state
        .get("reviewConsensus")
        .and_then(Value::as_object)
        .is_some_and(|points| {
            points.values().any(|value| {
                value.get("passed").and_then(Value::as_bool) == Some(true)
                    && nonempty_array(value.get("reviewers"))
            })
        })
        && reviewers_independent(state)
}

pub(super) fn context_complete(state: &Value, required: &[&str]) -> bool {
    let loaded = state
        .get("loadedContexts")
        .or_else(|| state.get("contextLoaded"))
        .and_then(Value::as_object);
    loaded.is_some_and(|items| {
        required.iter().all(|key| {
            items.get(*key).is_some_and(|value| {
                value.get("complete").and_then(Value::as_bool) == Some(true)
                    && (nonempty_string(value.get("digest"))
                        || nonempty_string(value.get("source")))
            })
        })
    })
}

pub(super) fn route_exempt(state: &Value) -> bool {
    state
        .get("scale")
        .and_then(Value::as_str)
        .is_some_and(|scale| {
            matches!(scale.to_ascii_lowercase().as_str(), "micro" | "small")
                || matches!(scale, "微" | "小")
        })
}

pub(super) fn structured_status(value: Option<&Value>, expected: &str) -> bool {
    value
        .and_then(|item| item.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case(expected))
}

pub(super) fn nonempty_object(value: &Value) -> bool {
    value.as_object().is_some_and(|object| !object.is_empty())
}

pub(super) fn nonempty_array(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|array| !array.is_empty())
}

fn evidence_manifest_exists(root: &Path) -> bool {
    fs::read_dir(root.join(".auto-engineering"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().join("evidence/manifest.json").is_file())
}

fn nonempty_string(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
