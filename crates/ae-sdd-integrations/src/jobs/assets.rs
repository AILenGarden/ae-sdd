use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ae_sdd_runtime::RuntimeResult;
use serde_json::{Map, Value, json};

use super::common::{
    JobContext, MAX_ASSET_BYTES, digest, read_bounded, required_string, schema_error,
};

const REQUIRED_SECTIONS: [&str; 7] = ["§A", "§B", "§C", "§D", "§E", "§F", "§G"];
const ASSET_SCHEMA: &str = "ae-sdd-project-assets/v1";
const FALLBACK_SCHEMA: &str = "ae-sdd-assets-fallback/v1";
const MAX_SECTION_OUTPUT_BYTES: usize = 32 * 1024;
const MAX_STAGE_SECTION_BYTES: usize = 4 * 1024;
const MAX_SNIPPET_BYTES: usize = 160;
const MAX_QUERY_CONTENT_BYTES: usize = 48 * 1024;
const MAX_READ_CONTENT_BYTES: usize = 40 * 1024;

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
    relative_path: String,
    resolution_source: String,
    bytes: Vec<u8>,
    metadata: AssetMetadata,
    sections: Vec<Section>,
    docs: Vec<IndexedDoc>,
}

#[derive(Default)]
struct AssetMetadata {
    present: bool,
    valid: bool,
    schema_version: Option<String>,
    project_key: Option<String>,
    inventory_digest: Option<String>,
    file_count: Option<u64>,
}

struct ResolvedAsset {
    path: PathBuf,
    relative_path: String,
    source: String,
}

struct SearchResult {
    hits: Vec<Value>,
    truncated: bool,
    original_bytes: usize,
    returned_bytes: usize,
}

struct BoundedText {
    content: String,
    truncated: bool,
    original_bytes: usize,
    returned_bytes: usize,
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
        let resolved = resolve_asset_path(context, arguments, &project_key)?;
        let bytes = read_bounded(&resolved.path, MAX_ASSET_BYTES)?;
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| schema_error("project asset must be UTF-8 Markdown"))?;
        let metadata = parse_metadata(&text, &project_key);
        let sections = parse_sections(&text);
        let docs = index_documents(&text, &sections);
        Ok(Self {
            project_key,
            relative_path: resolved.relative_path,
            resolution_source: resolved.source,
            bytes,
            metadata,
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
        let metadata_invalid = self.metadata.present && !self.metadata.valid;
        json!({
            "outcome": if missing.is_empty() && !metadata_invalid {"PASS"} else {"FAIL"},
            "projectKey":self.project_key,
            "assetFile":self.relative_path,
            "resolutionSource":self.resolution_source,
            "exists":true,
            "missingSections":missing,
            "metadataValid":self.metadata.valid,
            "metadata":self.metadata.payload(),
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
        if query.len() > 512 || query_tokens(query).len() > 32 {
            return Err(schema_error(
                "query exceeds the bounded asset query contract",
            ));
        }
        let top = super::common::bounded_u64(arguments, "top", 20, 100)? as usize;
        let result = self.search(query, top, MAX_QUERY_CONTENT_BYTES);
        Ok(json!({
            "outcome":"PASS",
            "projectKey":self.project_key,
            "query":query,
            "topN":top,
            "nHits":result.hits.len(),
            "hits":result.hits,
            "truncated":result.truncated,
            "originalByteLength":result.original_bytes,
            "returnedByteLength":result.returned_bytes,
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
        let content = bounded_text(&section.content, MAX_SECTION_OUTPUT_BYTES);
        Ok(json!({
            "outcome":"PASS",
            "projectKey":self.project_key,
            "section":name,
            "line":section.line,
            "content":content.content,
            "truncated":content.truncated,
            "originalByteLength":content.original_bytes,
            "returnedByteLength":content.returned_bytes,
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
        let mut remaining = MAX_READ_CONTENT_BYTES;
        let mut truncated = false;
        let mut original_bytes = 0_usize;
        let mut returned_bytes = 0_usize;
        let mut baseline_hits = Map::new();
        for key in baseline_keys {
            let result = self.search(key, 5, remaining);
            remaining = remaining.saturating_sub(result.returned_bytes);
            truncated |= result.truncated;
            original_bytes = original_bytes.saturating_add(result.original_bytes);
            returned_bytes = returned_bytes.saturating_add(result.returned_bytes);
            if !result.hits.is_empty() {
                baseline_hits.insert((*key).to_owned(), Value::Array(result.hits));
            }
        }
        let mut extra_hits = Map::new();
        for key in extra {
            let result = self.search(&key, 5, remaining);
            remaining = remaining.saturating_sub(result.returned_bytes);
            truncated |= result.truncated;
            original_bytes = original_bytes.saturating_add(result.original_bytes);
            returned_bytes = returned_bytes.saturating_add(result.returned_bytes);
            if !result.hits.is_empty() {
                extra_hits.insert(key.clone(), Value::Array(result.hits));
            }
        }
        let mut sections = Map::new();
        for name in stage_sections(stage).unwrap_or_default() {
            if let Some(section) = self.find_section(name) {
                let bounded =
                    bounded_text(&section.content, remaining.min(MAX_STAGE_SECTION_BYTES));
                remaining = remaining.saturating_sub(bounded.returned_bytes);
                truncated |= bounded.truncated;
                original_bytes = original_bytes.saturating_add(bounded.original_bytes);
                returned_bytes = returned_bytes.saturating_add(bounded.returned_bytes);
                sections.insert((*name).to_owned(), Value::String(bounded.content));
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
            "truncated":truncated,
            "originalByteLength":original_bytes,
            "returnedByteLength":returned_bytes,
            "stats":self.stats_payload(),
        }))
    }

    fn stats(&self) -> Value {
        let mut payload = self.stats_payload();
        payload["outcome"] = Value::String("PASS".to_owned());
        payload["projectKey"] = Value::String(self.project_key.clone());
        payload["assetFile"] = Value::String(self.relative_path.clone());
        payload["resolutionSource"] = Value::String(self.resolution_source.clone());
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

    fn search(&self, query: &str, top: usize, budget: usize) -> SearchResult {
        let tokens = query_tokens(query);
        if tokens.is_empty() || self.docs.is_empty() {
            return SearchResult {
                hits: Vec::new(),
                truncated: false,
                original_bytes: 0,
                returned_bytes: 0,
            };
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
        let mut hits = Vec::new();
        let mut truncated = false;
        let mut original_bytes = 0_usize;
        let mut returned_bytes = 0_usize;
        for (index, score) in ranked.into_iter().take(top) {
            let doc = &self.docs[index];
            let snippet = bounded_text(&doc.text, MAX_SNIPPET_BYTES);
            original_bytes = original_bytes.saturating_add(snippet.original_bytes);
            truncated |= snippet.truncated;
            let hit = json!({
                "section":bounded_text(&doc.section, 128).content,
                "line":doc.line,
                "score":round(score, 4),
                "matchedTokens":matched.remove(&index).unwrap_or_default(),
                "snippet":snippet.content,
                "snippetTruncated":snippet.truncated,
                "fileId":0,
            });
            let hit_bytes = serde_json::to_vec(&hit).map_or(usize::MAX, |value| value.len());
            if returned_bytes.saturating_add(hit_bytes) > budget {
                truncated = true;
                break;
            }
            returned_bytes = returned_bytes.saturating_add(hit_bytes);
            hits.push(hit);
        }
        SearchResult {
            hits,
            truncated,
            original_bytes,
            returned_bytes,
        }
    }

    fn find_section(&self, name: &str) -> Option<&Section> {
        let target = normalize_section(name);
        self.sections.iter().find(|section| {
            let candidate = normalize_section(&section.name);
            candidate == target || candidate.starts_with(&target)
        })
    }
}

impl AssetMetadata {
    fn payload(&self) -> Value {
        json!({
            "schemaVersion":self.schema_version,
            "projectKey":self.project_key,
            "inventoryDigest":self.inventory_digest,
            "fileCount":self.file_count,
        })
    }
}

fn resolve_asset_path(
    context: &JobContext<'_>,
    arguments: &Value,
    project_key: &str,
) -> RuntimeResult<ResolvedAsset> {
    if let Some(value) = arguments.get("assetFile") {
        if let Some(path) = value
            .as_str()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            && Path::new(path).is_absolute()
            && let Err(error) = context.existing_file(path)
            && error.code() == ae_sdd_protocol::StableErrorCode::WorkspaceOutsideAllowedRoot
        {
            return Err(error);
        }
        return Err(schema_error(
            "assetFile override is forbidden; use a schema-bound project-relative assetFallback",
        ));
    }

    let canonical = format!(".ae-sdd/assets/{project_key}.assets.md");
    if let Ok(path) = context.existing_file(&canonical) {
        return Ok(ResolvedAsset {
            path,
            relative_path: canonical,
            source: "canonical".to_owned(),
        });
    }

    if let Some(fallback) = arguments.get("assetFallback") {
        return typed_fallback(context, fallback, project_key);
    }

    if let Some(configured) = read_config_asset_path(context)? {
        let relative = if configured.starts_with(".ae-sdd/") {
            configured
        } else {
            format!(".ae-sdd/{configured}")
        };
        validate_asset_relative(&relative)?;
        if let Ok(path) = context.existing_file(&relative) {
            return Ok(ResolvedAsset {
                path,
                relative_path: relative,
                source: "declared-config-fallback".to_owned(),
            });
        }
    }

    let nested = format!(".ae-sdd/assets/{project_key}/{project_key}.assets.md");
    if let Ok(path) = context.existing_file(&nested) {
        return Ok(ResolvedAsset {
            path,
            relative_path: nested,
            source: "project-bound-legacy-fallback".to_owned(),
        });
    }
    Err(schema_error(
        "canonical project asset is missing and no declared contained fallback resolved",
    ))
}

fn typed_fallback(
    context: &JobContext<'_>,
    value: &Value,
    project_key: &str,
) -> RuntimeResult<ResolvedAsset> {
    let object = value
        .as_object()
        .ok_or_else(|| schema_error("assetFallback must be a typed object"))?;
    if object.len() != 3
        || !object.contains_key("schemaVersion")
        || !object.contains_key("projectKey")
        || !object.contains_key("relativePath")
    {
        return Err(schema_error("assetFallback has unknown or missing fields"));
    }
    if object.get("schemaVersion").and_then(Value::as_str) != Some(FALLBACK_SCHEMA) {
        return Err(schema_error("assetFallback schemaVersion is unsupported"));
    }
    if object.get("projectKey").and_then(Value::as_str) != Some(project_key) {
        return Err(schema_error(
            "assetFallback projectKey does not match workspace identity",
        ));
    }
    let relative = object
        .get("relativePath")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| schema_error("assetFallback relativePath is required"))?;
    validate_asset_relative(relative)?;
    let path = context.existing_file(relative)?;
    Ok(ResolvedAsset {
        path,
        relative_path: relative.replace('\\', "/"),
        source: "typed-fallback".to_owned(),
    })
}

fn validate_asset_relative(value: &str) -> RuntimeResult<()> {
    let normalized = value.replace('\\', "/");
    if Path::new(&normalized).is_absolute()
        || !normalized.starts_with(".ae-sdd/assets/")
        || !normalized.ends_with(".assets.md")
    {
        return Err(schema_error(
            "asset fallback must be a project-relative .ae-sdd/assets/*.assets.md path",
        ));
    }
    Ok(())
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

fn parse_metadata(text: &str, expected_project_key: &str) -> AssetMetadata {
    let mut metadata = AssetMetadata::default();
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return metadata;
    }
    metadata.present = true;
    let mut closed = false;
    for line in lines.by_ref().take(16) {
        if line == "---" {
            closed = true;
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return metadata;
        };
        let value = value.trim();
        match name.trim() {
            "schemaVersion" => metadata.schema_version = Some(value.to_owned()),
            "projectKey" => metadata.project_key = Some(value.to_owned()),
            "inventoryDigest" => metadata.inventory_digest = Some(value.to_owned()),
            "fileCount" => metadata.file_count = value.parse().ok(),
            _ => return metadata,
        }
    }
    metadata.valid = closed
        && metadata.schema_version.as_deref() == Some(ASSET_SCHEMA)
        && metadata.project_key.as_deref() == Some(expected_project_key)
        && metadata.file_count.is_some()
        && metadata.inventory_digest.as_deref().is_some_and(is_sha256);
    metadata
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn bounded_text(value: &str, limit: usize) -> BoundedText {
    let original_bytes = value.len();
    if original_bytes <= limit {
        return BoundedText {
            content: value.to_owned(),
            truncated: false,
            original_bytes,
            returned_bytes: original_bytes,
        };
    }
    let mut end = limit.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let content = value[..end].to_owned();
    BoundedText {
        returned_bytes: content.len(),
        content,
        truncated: true,
        original_bytes,
    }
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

#[cfg(test)]
mod tests {
    use std::fs;

    use ae_sdd_protocol::WorkspaceMode;
    use ae_sdd_runtime::BusinessWorkspace;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn workspace(root: &TempDir) -> BusinessWorkspace {
        BusinessWorkspace {
            workspace_id: "assets-unit-workspace".to_owned(),
            canonical_root: fs::canonicalize(root.path())
                .expect("canonical fixture")
                .to_string_lossy()
                .into_owned(),
            project_key: "assets-unit".to_owned(),
            mode: WorkspaceMode::Shadow,
            agent_role: None,
            agent_grant: None,
            caller_kind: None,
            inventory_generation: 1,
        }
    }

    fn canonical_asset(body: &str) -> String {
        format!(
            "---\nschemaVersion: ae-sdd-project-assets/v1\nprojectKey: assets-unit\ninventoryDigest: {}\nfileCount: 1\n---\n# assets-unit Project Assets\n## §A Outline\n{body}\n## §B Modules\nmodule\n## §C Fields\nfield\n## §D Components\ncomponent\n## §E API\napi\n## §F Keywords\nkeyword\n## §G Read API\nread\n",
            "a".repeat(64)
        )
    }

    #[test]
    fn assets_reject_arbitrary_absolute_asset_file_even_inside_workspace() {
        let root = TempDir::new().expect("fixture");
        let path = root.path().join("inside.assets.md");
        fs::write(&path, canonical_asset("body")).expect("asset");
        let workspace = workspace(&root);
        let context = JobContext::new(&workspace, None).expect("job context");

        let error = match AssetDocument::load(&context, &json!({"assetFile": path})) {
            Ok(_) => panic!("absolute override must be denied"),
            Err(error) => error,
        };
        assert_eq!(
            error.code(),
            ae_sdd_protocol::StableErrorCode::OperationSchemaInvalid
        );
    }

    #[test]
    fn assets_accept_only_schema_bound_contained_fallbacks() {
        let root = TempDir::new().expect("fixture");
        let assets = root.path().join(".ae-sdd/assets/legacy");
        fs::create_dir_all(&assets).expect("assets directory");
        fs::write(assets.join("legacy.assets.md"), canonical_asset("service")).expect("asset");
        let workspace = workspace(&root);
        let context = JobContext::new(&workspace, None).expect("job context");

        let document = AssetDocument::load(
            &context,
            &json!({
                "assetFallback": {
                    "schemaVersion": "ae-sdd-assets-fallback/v1",
                    "projectKey": "assets-unit",
                    "relativePath": ".ae-sdd/assets/legacy/legacy.assets.md"
                }
            }),
        )
        .expect("typed contained fallback");
        assert_eq!(document.resolution_source, "typed-fallback");

        assert!(
            AssetDocument::load(
                &context,
                &json!({"assetFallback":{"relativePath":".ae-sdd/assets/legacy/legacy.assets.md"}}),
            )
            .is_err()
        );
    }

    #[test]
    fn assets_section_and_query_report_explicit_bounded_truncation() {
        let root = TempDir::new().expect("fixture");
        let assets = root.path().join(".ae-sdd/assets");
        fs::create_dir_all(&assets).expect("assets directory");
        fs::write(
            assets.join("assets-unit.assets.md"),
            canonical_asset(&format!("service {}", "x".repeat(96 * 1024))),
        )
        .expect("asset");
        let workspace = workspace(&root);
        let context = JobContext::new(&workspace, None).expect("job context");
        let document = AssetDocument::load(&context, &json!({})).expect("asset document");

        let section = document.section(&json!({"name":"A"})).expect("section");
        assert_eq!(section["truncated"], true);
        assert!(section["content"].as_str().expect("content").len() <= 32 * 1024);
        assert!(
            section["originalByteLength"]
                .as_u64()
                .expect("original length")
                > section["returnedByteLength"]
                    .as_u64()
                    .expect("returned length")
        );

        let query = document
            .query(&json!({"query":"service","top":100}))
            .expect("query");
        assert_eq!(query["truncated"], true);
        assert!(query["returnedByteLength"].as_u64().expect("query bytes") <= 64 * 1024);
    }

    #[test]
    fn assets_check_reports_canonical_metadata_and_digest() {
        let root = TempDir::new().expect("fixture");
        let assets = root.path().join(".ae-sdd/assets");
        fs::create_dir_all(&assets).expect("assets directory");
        fs::write(
            assets.join("assets-unit.assets.md"),
            canonical_asset("service"),
        )
        .expect("asset");
        let workspace = workspace(&root);
        let context = JobContext::new(&workspace, None).expect("job context");
        let document = AssetDocument::load(&context, &json!({})).expect("asset document");

        let check = document.check();
        assert_eq!(check["outcome"], "PASS");
        assert_eq!(check["metadataValid"], true);
        assert_eq!(
            check["metadata"]["schemaVersion"],
            "ae-sdd-project-assets/v1"
        );
        assert_eq!(check["metadata"]["projectKey"], "assets-unit");
        assert_eq!(check["digest"].as_str().map(str::len), Some(64));
    }

    // `assets.query` scores hits over tokens, so the split contract is part of
    // the query surface: a change in casing, separator, or CJK boundary
    // handling silently changes which sections a query can reach. These cases
    // carry the inputs and expectations the retired Python oracle asserted.

    #[test]
    fn tokenize_splits_pascal_case_on_each_capital() {
        assert_eq!(
            tokenize("CsTicketAppService"),
            ["cs", "ticket", "app", "service"]
        );
    }

    #[test]
    fn tokenize_keeps_a_trailing_acronym_whole() {
        assert_eq!(tokenize("BossUserPO"), ["boss", "user", "po"]);
    }

    #[test]
    fn tokenize_splits_snake_and_kebab_separators() {
        assert_eq!(tokenize("boss_user_role"), ["boss", "user", "role"]);
        assert_eq!(
            tokenize("icec-cloud-life-cs"),
            ["icec", "cloud", "life", "cs"]
        );
    }

    #[test]
    fn tokenize_separates_latin_from_cjk() {
        let tokens = tokenize("BossUser 脱敏");
        assert!(tokens.contains(&"boss".to_owned()), "{tokens:?}");
        assert!(tokens.contains(&"user".to_owned()), "{tokens:?}");
        assert!(
            tokens.iter().any(|token| token.contains('脱')),
            "{tokens:?}"
        );
    }

    #[test]
    fn tokenize_splits_digit_runs_on_a_separator() {
        assert_eq!(tokenize("11101-11107"), ["11101", "11107"]);
    }

    #[test]
    fn tokenize_yields_nothing_for_empty_or_punctuation_only_input() {
        assert!(tokenize("").is_empty());
        let punctuation = tokenize("---|***");
        assert!(punctuation.is_empty(), "{punctuation:?}");
    }

    #[test]
    fn tokenize_lowercases_every_token() {
        let tokens = tokenize("AppService vs APPSERVICE");
        assert!(
            tokens.iter().all(|token| *token == token.to_lowercase()),
            "{tokens:?}"
        );
    }
}
