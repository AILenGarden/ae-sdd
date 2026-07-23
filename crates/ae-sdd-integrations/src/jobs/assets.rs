use std::fs;
use std::path::PathBuf;

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Map, Value, json};

use super::common::{
    JobContext, MAX_ASSET_BYTES, digest, read_bounded, required_string, schema_error,
};

const REQUIRED_SECTIONS: [&str; 7] = ["§A", "§B", "§C", "§D", "§E", "§F", "§G"];

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let document = AssetDocument::load(context, arguments)?;
    match entrypoint {
        "assets.check" => Ok(document.check()),
        "assets.outline" => Ok(document.outline()),
        "assets.query" => document.query(arguments),
        "assets.read" => document.read_stage(arguments),
        "assets.section" => document.section(arguments),
        "assets.stats" => Ok(document.stats()),
        _ => unreachable!("assets entrypoint was classified by caller"),
    }
}

struct AssetDocument {
    project_key: String,
    path: PathBuf,
    bytes: Vec<u8>,
    text: String,
    sections: Vec<Section>,
}

#[derive(Clone)]
struct Section {
    name: String,
    heading: String,
    line: usize,
    content: String,
}

impl AssetDocument {
    fn load(context: &JobContext<'_>, arguments: &Value) -> RuntimeResult<Self> {
        let project_key = arguments
            .get("project")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&context.workspace.project_key)
            .to_owned();
        let path = resolve_asset_path(context, arguments, &project_key)?;
        let bytes = read_bounded(&path, MAX_ASSET_BYTES)?;
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| schema_error("project asset must be UTF-8 Markdown"))?;
        let sections = parse_sections(&text);
        Ok(Self {
            project_key,
            path,
            bytes,
            text,
            sections,
        })
    }

    fn check(&self) -> Value {
        let missing = REQUIRED_SECTIONS
            .iter()
            .filter(|required| {
                !self
                    .sections
                    .iter()
                    .any(|section| section.heading.contains(**required))
            })
            .copied()
            .collect::<Vec<_>>();
        json!({
            "outcome": if missing.is_empty() {"PASS"} else {"FAIL"},
            "projectKey":self.project_key,
            "assetFile":relative_display(&self.path),
            "exists":true,
            "missingSections":missing,
            "digest":digest(&self.bytes),
        })
    }

    fn outline(&self) -> Value {
        json!({
            "outcome":"PASS",
            "projectKey":self.project_key,
            "stats":self.stats_payload(),
        })
    }

    fn query(&self, arguments: &Value) -> RuntimeResult<Value> {
        let query = required_string(arguments, "query")?;
        let top = super::common::bounded_u64(arguments, "top", 20, 100)? as usize;
        let hits = self.search(query, top);
        Ok(json!({
            "outcome":"PASS",
            "projectKey":self.project_key,
            "query":query,
            "topN":top,
            "nHits":hits.len(),
            "hits":hits,
        }))
    }

    fn section(&self, arguments: &Value) -> RuntimeResult<Value> {
        let name = required_string(arguments, "name")?;
        let normalized = normalize_section(name);
        let section = self
            .sections
            .iter()
            .find(|section| {
                normalize_section(&section.name) == normalized
                    || normalize_section(&section.heading) == normalized
            })
            .ok_or_else(|| schema_error("requested asset section does not exist"))?;
        Ok(json!({
            "outcome":"PASS",
            "projectKey":self.project_key,
            "section":name,
            "line":section.line,
            "content":section.content,
        }))
    }

    fn read_stage(&self, arguments: &Value) -> RuntimeResult<Value> {
        let stage = required_string(arguments, "stage")?;
        let baseline_keys = stage_keys(stage)
            .ok_or_else(|| schema_error("stage is not in the native asset read vocabulary"))?;
        let extra = arguments
            .get("keys")
            .map(parse_keys)
            .transpose()?
            .unwrap_or_default();
        let mut baseline_hits = Map::new();
        for key in baseline_keys {
            baseline_hits.insert((*key).to_owned(), Value::Array(self.search(key, 3)));
        }
        let mut extra_hits = Map::new();
        for key in extra {
            extra_hits.insert(key.clone(), Value::Array(self.search(&key, 3)));
        }
        Ok(json!({
            "outcome":"PASS",
            "stage":stage,
            "projectKey":self.project_key,
            "indexReady":true,
            "baselineHits":baseline_hits,
            "extraHits":extra_hits,
            "sections":{},
        }))
    }

    fn stats(&self) -> Value {
        let mut payload = self.stats_payload();
        payload["outcome"] = Value::String("PASS".to_owned());
        payload["projectKey"] = Value::String(self.project_key.clone());
        payload["assetFile"] = Value::String(relative_display(&self.path));
        payload["digest"] = Value::String(digest(&self.bytes));
        payload
    }

    fn stats_payload(&self) -> Value {
        let tokens = self
            .text
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .count();
        let documents = self.sections.len().max(1);
        json!({
            "nDocs":documents,
            "nTokens":tokens,
            "nSections":self.sections.len(),
            "avgDocLen":tokens / documents,
            "sections":self.sections.iter().map(|section| section.heading.clone()).collect::<Vec<_>>(),
            "byteLength":self.bytes.len(),
            "cacheStatus":"native",
        })
    }

    fn search(&self, query: &str, top: usize) -> Vec<Value> {
        let tokens = query_tokens(query);
        if tokens.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for section in &self.sections {
            for (offset, line) in section.content.lines().enumerate() {
                let folded = line.to_lowercase();
                let matched = tokens
                    .iter()
                    .filter(|token| folded.contains(token.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                if matched.is_empty() {
                    continue;
                }
                let heading_folded = section.heading.to_lowercase();
                let heading_bonus = matched
                    .iter()
                    .filter(|token| heading_folded.contains(token.as_str()))
                    .count();
                hits.push((
                    matched.len() * 10 + heading_bonus * 5,
                    json!({
                        "section":section.heading,
                        "line":section.line + offset,
                        "score":matched.len() * 10 + heading_bonus * 5,
                        "matchedTokens":matched,
                        "snippet":truncate(line.trim(), 240),
                    }),
                ));
            }
        }
        hits.sort_by(|left, right| right.0.cmp(&left.0));
        hits.into_iter().take(top).map(|(_, value)| value).collect()
    }
}

fn resolve_asset_path(
    context: &JobContext<'_>,
    arguments: &Value,
    project_key: &str,
) -> RuntimeResult<PathBuf> {
    if let Some(path) = arguments
        .get("assetFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return context.existing_file(path);
    }
    let configured = read_config_asset_path(context)?;
    if let Some(path) = configured {
        if let Ok(resolved) = context.existing_file(&format!(".ae-sdd/{path}")) {
            return Ok(resolved);
        }
    }
    let direct = format!(".ae-sdd/assets/{project_key}.assets.md");
    if let Ok(resolved) = context.existing_file(&direct) {
        return Ok(resolved);
    }
    let assets_dir = context.root.join(".ae-sdd").join("assets");
    let mut candidates = fs::read_dir(assets_dir)
        .map_err(super::common::io_error)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".assets.md"))
        })
        .take(3)
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(schema_error(
            "project asset could not be resolved unambiguously",
        ));
    }
    context.existing_file(&candidates.remove(0).to_string_lossy())
}

fn read_config_asset_path(context: &JobContext<'_>) -> RuntimeResult<Option<String>> {
    let path = match context.project_file(".ae-sdd/config.yaml") {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    let bytes = read_bounded(&path, super::common::MAX_FILE_BYTES)?;
    let text = String::from_utf8(bytes).map_err(|_| schema_error("config.yaml is not UTF-8"))?;
    Ok(text.lines().find_map(|line| {
        line.strip_prefix("assetPath:")
            .map(str::trim)
            .map(trim_yaml_scalar)
            .filter(|value| !value.is_empty())
    }))
}

fn parse_sections(text: &str) -> Vec<Section> {
    let lines = text.lines().collect::<Vec<_>>();
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            let hashes = trimmed.chars().take_while(|character| *character == '#').count();
            (hashes > 0 && hashes <= 6 && trimmed.as_bytes().get(hashes) == Some(&b' '))
                .then(|| (index, trimmed[hashes + 1..].trim().to_owned()))
        })
        .collect::<Vec<_>>();
    headings
        .iter()
        .enumerate()
        .map(|(position, (start, heading))| {
            let end = headings
                .get(position + 1)
                .map_or(lines.len(), |(next, _)| *next);
            Section {
                name: heading
                    .split_whitespace()
                    .next()
                    .unwrap_or(heading)
                    .to_owned(),
                heading: heading.clone(),
                line: start + 1,
                content: lines[*start..end].join("\n"),
            }
        })
        .collect()
}

fn parse_keys(value: &Value) -> RuntimeResult<Vec<String>> {
    let mut keys = match value {
        Value::String(value) => value.split(',').map(str::trim).map(str::to_owned).collect(),
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .map(str::to_owned)
                    .ok_or_else(|| schema_error("keys must contain strings"))
            })
            .collect::<RuntimeResult<Vec<_>>>()?,
        _ => return Err(schema_error("keys must be a string or string array")),
    };
    keys.retain(|key| !key.is_empty());
    if keys.len() > 32 || keys.iter().any(|key| key.len() > 128) {
        return Err(schema_error("keys exceed the bounded asset query contract"));
    }
    Ok(keys)
}

fn stage_keys(stage: &str) -> Option<&'static [&'static str]> {
    match stage {
        "requirement-analysis" => Some(&["business", "constraint", "stakeholder", "risk"]),
        "dr-generate" => Some(&["architecture", "component", "interface", "data model"]),
        "story-generate" => Some(&["contract", "acceptance", "field", "flow"]),
        "testcase-generate" => Some(&["acceptance", "boundary", "error", "verification"]),
        "coding-process" | "coding" => Some(&["technology", "module", "path", "constraint"]),
        "test-running" => Some(&["test", "environment", "command", "evidence"]),
        "code-reviewed" => Some(&["security", "quality", "constraint", "review"]),
        _ => None,
    }
}

fn query_tokens(value: &str) -> Vec<String> {
    let folded = value.to_lowercase();
    let mut tokens = folded
        .split(|character: char| !character.is_alphanumeric() && character != '-' && character != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if tokens.is_empty() && !folded.trim().is_empty() {
        tokens.push(folded.trim().to_owned());
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn normalize_section(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('§')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn trim_yaml_scalar(value: &str) -> String {
    value.trim_matches(['\'', '"']).to_owned()
}

fn relative_display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
