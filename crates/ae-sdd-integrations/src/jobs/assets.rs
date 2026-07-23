use std::collections::{BTreeMap, BTreeSet};
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
    sections: Vec<Section>,
    docs: Vec<IndexedDoc>,
}

#[derive(Clone)]
struct Section {
    name: String,
    heading: String,
    line: usize,
    content: String,
}

struct IndexedDoc {
    section: String,
    line: usize,
    text: String,
    tokens: Vec<String>,
    term_frequency: BTreeMap<String, usize>,
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
        let docs = index_documents(&text, &sections);
        Ok(Self {
            project_key,
            path,
            bytes,
            sections,
            docs,
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
            let hits = self.search(key, 5);
            if !hits.is_empty() {
                baseline_hits.insert((*key).to_owned(), Value::Array(hits));
            }
        }
        let mut extra_hits = Map::new();
        for key in extra {
            let hits = self.search(&key, 5);
            if !hits.is_empty() {
                extra_hits.insert(key.clone(), Value::Array(hits));
            }
        }
        let mut sections = Map::new();
        for name in stage_sections(stage).unwrap_or_default() {
            if let Some(section) = self.find_section(name) {
                sections.insert((*name).to_owned(), Value::String(section.content.clone()));
            }
        }
        Ok(json!({
            "outcome":"PASS",
            "stage":stage,
            "projectKey":self.project_key,
            "indexReady":true,
            "baselineHits":baseline_hits,
            "extraHits":extra_hits,
            "sections":sections,
            "stats":self.stats_payload(),
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
        let token_count = self
            .docs
            .iter()
            .flat_map(|doc| doc.tokens.iter())
            .collect::<BTreeSet<_>>()
            .len();
        let documents = self.docs.len();
        let average = if documents == 0 {
            0.0
        } else {
            self.docs.iter().map(|doc| doc.tokens.len()).sum::<usize>() as f64 / documents as f64
        };
        json!({
            "nDocs":documents,
            "nTokens":token_count,
            "nSections":self.sections.len(),
            "avgDocLen":round(average, 2),
            "sections":self.sections.iter().map(|section| section.name.clone()).collect::<Vec<_>>(),
            "byteLength":self.bytes.len(),
            "cacheStatus":"native",
        })
    }

    fn search(&self, query: &str, top: usize) -> Vec<Value> {
        let tokens = query_tokens(query);
        if tokens.is_empty() || self.docs.is_empty() {
            return Vec::new();
        }
        let average = self.docs.iter().map(|doc| doc.tokens.len()).sum::<usize>() as f64
            / self.docs.len() as f64;
        let mut scores = BTreeMap::<usize, f64>::new();
        let mut matched = BTreeMap::<usize, BTreeSet<String>>::new();
        for token in tokens {
            let postings = self
                .docs
                .iter()
                .enumerate()
                .filter_map(|(index, doc)| {
                    doc.term_frequency
                        .get(&token)
                        .copied()
                        .map(|frequency| (index, frequency))
                })
                .collect::<Vec<_>>();
            if postings.is_empty() {
                continue;
            }
            let document_frequency = postings.len() as f64;
            let idf = (1.0
                + (self.docs.len() as f64 - document_frequency + 0.5) / (document_frequency + 0.5))
                .ln();
            for (index, frequency) in postings {
                let length = self.docs[index].tokens.len().max(1) as f64;
                let frequency = frequency as f64;
                let normalized = (frequency * 2.5)
                    / (frequency + 1.5 * (0.25 + 0.75 * length / average.max(1.0)));
                *scores.entry(index).or_default() += idf * normalized;
                matched.entry(index).or_default().insert(token.clone());
            }
        }
        let mut ranked = scores.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        ranked
            .into_iter()
            .take(top)
            .map(|(index, score)| {
                let doc = &self.docs[index];
                json!({
                    "section":doc.section,
                    "line":doc.line,
                    "score":round(score, 4),
                    "matchedTokens":matched.remove(&index).unwrap_or_default(),
                    "snippet":doc.text,
                    "fileId":0,
                })
            })
            .collect()
    }

    fn find_section(&self, name: &str) -> Option<&Section> {
        let target = normalize_section(name);
        self.sections.iter().find(|section| {
            let candidate = normalize_section(&section.name);
            candidate == target || candidate.starts_with(&target)
        })
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
    if let Some(path) = configured
        && let Ok(resolved) = context.existing_file(&format!(".ae-sdd/{path}"))
    {
        return Ok(resolved);
    }
    let nested = format!(".ae-sdd/assets/{project_key}/{project_key}.assets.md");
    if let Ok(resolved) = context.existing_file(&nested) {
        return Ok(resolved);
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
            let hashes = trimmed
                .chars()
                .take_while(|character| *character == '#')
                .count();
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
                name: section_anchor(heading),
                heading: heading.clone(),
                line: start + 1,
                content: lines[*start..end].join("\n"),
            }
        })
        .collect()
}

fn index_documents(text: &str, sections: &[Section]) -> Vec<IndexedDoc> {
    let mut in_code_fence = false;
    text.lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = index + 1;
            let text = raw.trim();
            if text.starts_with("```") {
                in_code_fence = !in_code_fence;
                return None;
            }
            if text.is_empty() || matches!(text, "---" | "***" | "___") || is_table_separator(text)
            {
                return None;
            }
            let tokens = tokenize(text);
            let mut term_frequency = BTreeMap::new();
            for token in &tokens {
                *term_frequency.entry(token.clone()).or_insert(0) += 1;
            }
            let section = sections
                .iter()
                .rev()
                .find(|section| section.line <= line)
                .map_or_else(|| "§0".to_owned(), |section| section.name.clone());
            Some(IndexedDoc {
                section,
                line,
                text: text.to_owned(),
                tokens,
                term_frequency,
            })
        })
        .collect()
}

fn is_table_separator(value: &str) -> bool {
    value.starts_with('|')
        && value.ends_with('|')
        && value
            .chars()
            .all(|character| matches!(character, '|' | '-' | ':' | ' ' | '\t'))
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
        "requirement-analysis" => Some(&["AppService", "Repository", "DomainService", "Service"]),
        "dr-generate" => Some(&[
            "AppService",
            "Repository",
            "Converter",
            "Facade",
            "FeignClient",
            "ServiceProviderConstants",
        ]),
        "story-generate" => Some(&[
            "AppService",
            "Controller",
            "ServiceImpl",
            "Repository",
            "Converter",
            "FeignClient",
        ]),
        "story-review" => Some(&[
            "AppService",
            "Repository",
            "@Transactional",
            "Facade",
            "deleted_flag",
            "cellphone",
        ]),
        "task-generate" => Some(&[
            "AppService",
            "Repository",
            "Mapper",
            "Converter",
            "Command",
            "Query",
        ]),
        "coding" => Some(&[
            "AppService",
            "Repository",
            "Converter",
            "PO",
            "DO",
            "Mapper",
            "@Transactional",
            "Facade",
        ]),
        "code-review" => Some(&[
            "@Transactional",
            "Facade",
            "FeignClient",
            "AccessUserInfoContext",
            "LocalDateTime",
            "cellphone",
            "deleted_flag",
            "错误码",
        ]),
        "testcase" => Some(&[
            "TestRestTemplate",
            "@SpringBootTest",
            "Mockito",
            "@Rollback",
            "ApiResult",
            "PagedModels",
        ]),
        _ => None,
    }
}

fn stage_sections(stage: &str) -> Option<&'static [&'static str]> {
    match stage {
        "requirement-analysis" => Some(&["§A", "§B"]),
        "dr-generate" => Some(&["§3", "§5", "§7"]),
        "story-generate" | "task-generate" => Some(&["§3", "§4", "§5"]),
        "story-review" => Some(&["§4", "§6"]),
        "coding" => Some(&["§4", "§5", "§6"]),
        "code-review" => Some(&["§6"]),
        "testcase" => Some(&[]),
        _ => None,
    }
}

fn query_tokens(value: &str) -> Vec<String> {
    tokenize(value)
}

fn tokenize(value: &str) -> Vec<String> {
    let characters = value.chars().collect::<Vec<_>>();
    let mut spaced = String::with_capacity(value.len());
    for (index, character) in characters.iter().copied().enumerate() {
        let previous = index.checked_sub(1).and_then(|value| characters.get(value));
        let next = characters.get(index + 1);
        if character.is_ascii_uppercase()
            && (previous.is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
                || (previous.is_some_and(|value| value.is_ascii_uppercase())
                    && next.is_some_and(|value| value.is_ascii_lowercase())))
        {
            spaced.push(' ');
        }
        spaced.push(if matches!(character, '_' | '-' | '.') {
            ' '
        } else {
            character
        });
    }
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_kind = 0_u8;
    for character in spaced.chars() {
        let kind = if is_cjk(character) {
            2
        } else if character.is_ascii_alphanumeric() {
            1
        } else {
            0
        };
        if (kind == 0 || (current_kind != 0 && kind != current_kind)) && !current.is_empty() {
            tokens.push(current.to_lowercase());
            current.clear();
        }
        if kind != 0 {
            current.push(character);
        }
        current_kind = kind;
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }
    tokens
}

fn is_cjk(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff)
}

fn section_anchor(heading: &str) -> String {
    let first = heading.split_whitespace().next().unwrap_or(heading);
    let value = first.trim_end_matches('.');
    if value.starts_with('§') {
        value.to_owned()
    } else {
        format!("§{value}")
    }
}

fn round(value: f64, places: i32) -> f64 {
    let scale = 10_f64.powi(places);
    (value * scale).round() / scale
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

fn trim_yaml_scalar(value: &str) -> String {
    value.trim_matches(['\'', '"']).to_owned()
}

fn relative_display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
