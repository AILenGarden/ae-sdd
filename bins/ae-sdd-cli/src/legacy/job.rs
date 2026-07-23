use ae_sdd_protocol::{RequestParams, RpcMethod};
use serde_json::{Map, Value, json};

use super::{LegacyArgumentError, LegacyCommandRoute, LegacyRpcAdapter, LegacyTarget};

const JOB_SUBMIT_RPC_DEADLINE_MS: u64 = 30_000;
const POSITIONALS_FIELD: &str = "legacyPositionals";
const MAX_TEXT_BYTES: usize = 64 * 1024;

/// Validate command-specific legacy job arguments and build the strict
/// `job.submit` payload without weakening the registered workspace identity.
pub fn adapt_job_submission(
    route: &LegacyCommandRoute,
    entrypoint: &str,
    params: &mut RequestParams<Value>,
    now_unix_ms: u64,
) -> Result<(), LegacyArgumentError> {
    verify_route(route, entrypoint)?;
    let background_budget_ms = params.deadline_ms;
    let deadline_unix_ms = now_unix_ms
        .checked_add(background_budget_ms)
        .ok_or_else(|| error("background job deadline overflow"))?;
    let arguments = normalize_arguments(entrypoint, params.payload.clone())?;
    params.deadline_ms = background_budget_ms.min(JOB_SUBMIT_RPC_DEADLINE_MS);
    params.payload = json!({
        "entrypoint":entrypoint,
        "arguments":arguments,
        "deadlineUnixMs":deadline_unix_ms,
    });
    Ok(())
}

fn verify_route(route: &LegacyCommandRoute, entrypoint: &str) -> Result<(), LegacyArgumentError> {
    match &route.target {
        LegacyTarget::Rpc {
            method: RpcMethod::JobSubmit,
            adapter:
                LegacyRpcAdapter::JobSubmission {
                    entrypoint: frozen, ..
                },
        } if frozen == entrypoint => Ok(()),
        _ => Err(error(
            "job adapter entrypoint differs from the frozen legacy route",
        )),
    }
}

fn normalize_arguments(entrypoint: &str, payload: Value) -> Result<Value, LegacyArgumentError> {
    let mut object = payload
        .as_object()
        .cloned()
        .ok_or_else(|| error("legacy job business payload must be an object"))?;
    let mut positionals = take_positionals(&mut object)?;
    match entrypoint {
        "assets.check" => finish(object, positionals, &["assetFile", "project"], |value| {
            strings(value, &["assetFile", "project"])
        }),
        "assets.outline" | "assets.stats" => {
            finish(object, positionals, &["assetFile", "project"], |value| {
                strings(value, &["assetFile", "project"])
            })
        }
        "assets.query" => {
            positional(&mut object, &mut positionals, "query")?;
            finish(
                object,
                positionals,
                &["assetFile", "project", "query", "top"],
                |value| {
                    required_string(value, "query")?;
                    strings(value, &["assetFile", "project"])?;
                    bounded_integer(value, "top", 100)
                },
            )
        }
        "assets.read" => {
            positional(&mut object, &mut positionals, "stage")?;
            finish(
                object,
                positionals,
                &["assetFile", "keys", "project", "stage"],
                |value| {
                    required_string(value, "stage")?;
                    strings(value, &["assetFile", "project"])?;
                    string_or_array(value, "keys", 32)
                },
            )
        }
        "assets.section" => {
            positional(&mut object, &mut positionals, "name")?;
            finish(
                object,
                positionals,
                &["assetFile", "name", "project"],
                |value| {
                    required_string(value, "name")?;
                    strings(value, &["assetFile", "project"])
                },
            )
        }
        "automation.status" | "db.audit" => finish(object, positionals, &[], |_| Ok(())),
        "gate.doc-storage" => finish(
            object,
            positionals,
            &["intent", "path", "project"],
            |value| {
                required_string(value, "path")?;
                strings(value, &["intent", "project"])
            },
        ),
        "iteration-check" => finish(object, positionals, &["project"], |value| {
            strings(value, &["project"])
        }),
        "memory.create" => finish(
            object,
            positionals,
            &[
                "contextJson",
                "entityId",
                "entityType",
                "phase",
                "project",
                "sources",
                "story",
                "task",
            ],
            |value| {
                strings(
                    value,
                    &[
                        "entityId",
                        "entityType",
                        "phase",
                        "project",
                        "story",
                        "task",
                    ],
                )?;
                if value
                    .get("contextJson")
                    .is_some_and(|context| !context.is_string() && !context.is_object())
                {
                    return Err(error("contextJson must be text or a JSON object"));
                }
                string_or_array(value, "sources", 16)
            },
        ),
        "memory.read" | "memory.clean" | "memory.clean-all" | "memory.summarize" => finish(
            object,
            positionals,
            &[
                "entityId",
                "entityType",
                "phase",
                "project",
                "story",
                "task",
            ],
            |value| {
                strings(
                    value,
                    &[
                        "entityId",
                        "entityType",
                        "phase",
                        "project",
                        "story",
                        "task",
                    ],
                )
            },
        ),
        "memory.update" => finish(
            object,
            positionals,
            &[
                "content",
                "contentFile",
                "entityId",
                "entityType",
                "phase",
                "project",
                "slice",
                "story",
                "task",
            ],
            |value| {
                required_string(value, "slice")?;
                strings(
                    value,
                    &[
                        "content",
                        "contentFile",
                        "entityId",
                        "entityType",
                        "phase",
                        "project",
                        "story",
                        "task",
                    ],
                )?;
                at_most_one(value, "content", "contentFile")
            },
        ),
        "memory.common" => {
            positional(&mut object, &mut positionals, "commonAction")?;
            finish(
                object,
                positionals,
                &[
                    "commonAction",
                    "content",
                    "contentFile",
                    "entityId",
                    "entityType",
                    "phase",
                    "project",
                    "story",
                    "task",
                ],
                |value| {
                    let action = required_string(value, "commonAction")?;
                    if !matches!(action, "read" | "update" | "clean") {
                        return Err(error("commonAction must be read, update, or clean"));
                    }
                    strings(
                        value,
                        &[
                            "content",
                            "contentFile",
                            "entityId",
                            "entityType",
                            "phase",
                            "project",
                            "story",
                            "task",
                        ],
                    )?;
                    at_most_one(value, "content", "contentFile")
                },
            )
        }
        "memory.search" => finish(
            object,
            positionals,
            &[
                "entityId",
                "entityType",
                "limit",
                "phase",
                "project",
                "query",
                "story",
                "task",
            ],
            |value| {
                required_string(value, "query")?;
                strings(
                    value,
                    &[
                        "entityId",
                        "entityType",
                        "phase",
                        "project",
                        "story",
                        "task",
                    ],
                )?;
                bounded_integer(value, "limit", 100)
            },
        ),
        "update-check" => finish(object, positionals, &["affected", "only"], |value| {
            strings(value, &["affected", "only"])?;
            at_most_one(value, "affected", "only")
        }),
        "baseline.inspect" => finish(object, positionals, &["gate"], |value| {
            strings(value, &["gate"])
        }),
        "baseline.diff" => finish(
            object,
            positionals,
            &[
                "gate",
                "report",
                "reportFile",
                "rulesetFingerprint",
                "touched",
            ],
            |value| {
                strings(value, &["gate", "reportFile", "rulesetFingerprint"])?;
                exactly_one(value, "report", "reportFile")?;
                if let Some(report) = value.get("report")
                    && !report.is_object()
                {
                    return Err(error("baseline report must be a JSON object"));
                }
                string_or_array(value, "touched", 10_000)
            },
        ),
        "classify" => finish(object, positionals, &["file", "text"], |value| {
            exactly_one(value, "file", "text")?;
            strings(value, &["file", "text"])
        }),
        "db.profiles" => finish(object, positionals, &["init"], |value| {
            false_only(value, "init")
        }),
        "db.query" => finish(
            object,
            positionals,
            &["limit", "profile", "sql", "sqlFile", "write"],
            |value| {
                required_string(value, "profile")?;
                strings(value, &["sql", "sqlFile"])?;
                exactly_one(value, "sql", "sqlFile")?;
                bounded_integer(value, "limit", 1_000)?;
                false_only(value, "write")
            },
        ),
        "db.explain" => finish(
            object,
            positionals,
            &["limit", "profile", "sql", "sqlFile"],
            |value| {
                required_string(value, "profile")?;
                strings(value, &["sql", "sqlFile"])?;
                exactly_one(value, "sql", "sqlFile")?;
                bounded_integer(value, "limit", 1_000)
            },
        ),
        "evidence.lookup" => finish(
            object,
            positionals,
            &[
                "command",
                "inputFingerprint",
                "story",
                "toolchainFingerprint",
            ],
            |value| {
                for field in [
                    "command",
                    "inputFingerprint",
                    "story",
                    "toolchainFingerprint",
                ] {
                    required_string(value, field)?;
                }
                Ok(())
            },
        ),
        "git.status" => finish(object, positionals, &[], |_| Ok(())),
        "git.diff" => finish(object, positionals, &["base", "head", "stat"], |value| {
            strings(value, &["base", "head"])?;
            boolean(value, "stat")
        }),
        "git.log" => finish(object, positionals, &["limit", "path"], |value| {
            strings(value, &["path"])?;
            bounded_integer(value, "limit", 100)
        }),
        "git.blame" => finish(object, positionals, &["end", "file", "start"], |value| {
            required_string(value, "file")?;
            bounded_integer(value, "start", u64::MAX)?;
            bounded_integer(value, "end", u64::MAX)?;
            if value.contains_key("start") != value.contains_key("end") {
                return Err(error("git blame start and end must be supplied together"));
            }
            let start = value.get("start").and_then(Value::as_u64);
            let end = value.get("end").and_then(Value::as_u64);
            if let (Some(start), Some(end)) = (start, end)
                && (end < start || end - start > 10_000)
            {
                return Err(error("git blame line range is invalid or exceeds 10000"));
            }
            Ok(())
        }),
        "git.impact" => finish(
            object,
            positionals,
            &["base", "file", "files", "head"],
            |value| {
                strings(value, &["base", "head"])?;
                at_most_one(value, "file", "files")?;
                string_or_array(value, "file", 10_000)?;
                string_or_array(value, "files", 10_000)
            },
        ),
        "perf.doctor" | "perf.report" => finish(object, positionals, &["last", "limit"], |value| {
            bounded_integer(value, "last", 10_000)?;
            bounded_integer(value, "limit", 100)
        }),
        "plugin.list" | "plugin.validate" => finish(object, positionals, &[], |_| Ok(())),
        "plugin.trace" => {
            positional(&mut object, &mut positionals, "target")?;
            finish(object, positionals, &["target"], |value| {
                required_string(value, "target").map(|_| ())
            })
        }
        _ => Err(error(format!(
            "job entrypoint {entrypoint} has no read-only CLI schema"
        ))),
    }
}

fn finish<F>(
    object: Map<String, Value>,
    positionals: Vec<String>,
    allowed: &[&str],
    validate: F,
) -> Result<Value, LegacyArgumentError>
where
    F: FnOnce(&Map<String, Value>) -> Result<(), LegacyArgumentError>,
{
    if !positionals.is_empty() {
        return Err(error("too many positional arguments for legacy job"));
    }
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(error(format!("unknown legacy job field {field}")));
    }
    validate(&object)?;
    Ok(Value::Object(object))
}

fn take_positionals(object: &mut Map<String, Value>) -> Result<Vec<String>, LegacyArgumentError> {
    let Some(value) = object.remove(POSITIONALS_FIELD) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| error("internal legacy positionals must be an array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| error("legacy positional must be a string"))
        })
        .collect()
}

fn positional(
    object: &mut Map<String, Value>,
    positionals: &mut Vec<String>,
    field: &str,
) -> Result<(), LegacyArgumentError> {
    if positionals.is_empty() {
        return Ok(());
    }
    if object.contains_key(field) {
        return Err(error(format!(
            "{field} was supplied as both positional and named argument"
        )));
    }
    object.insert(field.to_owned(), Value::String(positionals.remove(0)));
    Ok(())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, LegacyArgumentError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= MAX_TEXT_BYTES)
        .ok_or_else(|| error(format!("{field} is required and must be bounded text")))?;
    Ok(value)
}

fn strings(object: &Map<String, Value>, fields: &[&str]) -> Result<(), LegacyArgumentError> {
    for field in fields {
        if object.contains_key(*field) {
            required_string(object, field)?;
        }
    }
    Ok(())
}

fn bounded_integer(
    object: &Map<String, Value>,
    field: &str,
    max: u64,
) -> Result<(), LegacyArgumentError> {
    if let Some(value) = object.get(field)
        && !value
            .as_u64()
            .is_some_and(|number| number > 0 && number <= max)
    {
        return Err(error(format!("{field} must be an integer from 1 to {max}")));
    }
    Ok(())
}

fn boolean(object: &Map<String, Value>, field: &str) -> Result<(), LegacyArgumentError> {
    if object.get(field).is_some_and(|value| !value.is_boolean()) {
        return Err(error(format!("{field} must be a boolean")));
    }
    Ok(())
}

fn false_only(object: &Map<String, Value>, field: &str) -> Result<(), LegacyArgumentError> {
    boolean(object, field)?;
    if object.get(field).and_then(Value::as_bool) == Some(true) {
        return Err(error(format!(
            "mutating --{field} is not available through read-only job.submit"
        )));
    }
    Ok(())
}

fn string_or_array(
    object: &Map<String, Value>,
    field: &str,
    max_items: usize,
) -> Result<(), LegacyArgumentError> {
    let Some(value) = object.get(field) else {
        return Ok(());
    };
    match value {
        Value::String(value) if !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES => Ok(()),
        Value::Array(values)
            if !values.is_empty()
                && values.len() <= max_items
                && values.iter().all(|value| {
                    value
                        .as_str()
                        .is_some_and(|text| !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES)
                }) =>
        {
            Ok(())
        }
        _ => Err(error(format!(
            "{field} must be bounded text or a bounded string array"
        ))),
    }
}

fn exactly_one(
    object: &Map<String, Value>,
    left: &str,
    right: &str,
) -> Result<(), LegacyArgumentError> {
    if object.contains_key(left) == object.contains_key(right) {
        Err(error(format!(
            "exactly one of {left} or {right} is required"
        )))
    } else {
        Ok(())
    }
}

fn at_most_one(
    object: &Map<String, Value>,
    left: &str,
    right: &str,
) -> Result<(), LegacyArgumentError> {
    if object.contains_key(left) && object.contains_key(right) {
        Err(error(format!("{left} and {right} cannot be combined")))
    } else {
        Ok(())
    }
}

fn error(message: impl Into<String>) -> LegacyArgumentError {
    LegacyArgumentError::new(message)
}
