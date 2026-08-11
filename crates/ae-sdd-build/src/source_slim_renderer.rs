//! Template-authoritative rendering for source-SKILL slim entries.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::source_slim_frontmatter::{clean_frontmatter, parse_frontmatter};

const HEADING_LIMIT: usize = 120;
const REFERENCE_LIMIT: usize = 120;
const CRITICAL_TOKEN_LIMIT: usize = 128;
const TEMPLATE_FRONTMATTER_SLOT: &str = "{original frontmatter without source_* fields}";
const TEMPLATE_FALLBACK_SLOT: &str = "{fallback path}";
const TEMPLATE_FALLBACK_HASH_SLOT: &str =
    "{sha256 of canonical full fallback text (UTF-8, LF, no BOM)}";
const TEMPLATE_BYTE_COUNT_SLOT: &str = "{canonical byte count}";
const TEMPLATE_LINE_COUNT_SLOT: &str = "{line count}";
const TEMPLATE_SEMANTIC_HASH_SLOT: &str = "{sha256 of semantic inventory JSON}";
const TEMPLATE_TITLE_SLOT: &str = "{title}";
const TEMPLATE_ROOT_SLOT: &str = "{source root}";
const TEMPLATE_SOURCE_SLOT: &str = "{source path}";
const TEMPLATE_HASH_SLOT: &str = "{sha256}";
const TEMPLATE_SUMMARY_SLOT: &str = "{frontmatter description or first paragraph}";
const TEMPLATE_SEMANTIC_ROW: &str =
    "| identity_trigger | {detected evidence} | {design docs} | {fallback rule} |";
const TEMPLATE_HEADING_ROW: &str = "| {level} | {line} | {heading title} |";
const TEMPLATE_REFERENCE_ROW: &str = "| {inline reference} |";
const TEMPLATE_ROW_MARKERS: [&str; 3] = [
    TEMPLATE_SEMANTIC_ROW,
    TEMPLATE_HEADING_ROW,
    TEMPLATE_REFERENCE_ROW,
];
const REQUIRED_TEMPLATE_CONTENT: [&str; 9] = [
    "source_slimmed: true",
    "source_slim_schema: ae-sdd-source-slim/v2",
    "source_slim_standard: standards/skill-source-slimming-standard.md",
    "source_slim_template: templates/skill/source-skill-slim-entry-template.md",
    "## Load Contract",
    "## Semantic Inventory",
    "## Source Slimming SOP",
    "## Headings",
    "## Inline References",
];

pub(crate) struct RenderRequest<'a> {
    pub(crate) root_name: &'a str,
    pub(crate) source: &'a Path,
    pub(crate) fallback: &'a Path,
    pub(crate) fallback_text: &'a str,
    pub(crate) template: &'a str,
}

#[derive(Debug)]
pub(crate) enum RenderError {
    Frontmatter(String),
    Template(String),
    Semantic(String),
    Serialize(serde_json::Error),
}

/// Renders a slim entry from canonical fallback bytes and the source template.
pub(crate) fn render_slim_entry(request: RenderRequest<'_>) -> Result<String, RenderError> {
    let frontmatter = parse_frontmatter(request.fallback_text)
        .map_err(RenderError::Frontmatter)?
        .ok_or_else(|| RenderError::Frontmatter("fallback has no frontmatter".to_owned()))?;
    let fallback = path_to_posix(request.fallback);
    let source = path_to_posix(request.source);
    let fallback_hash = sha256_hex(request.fallback_text.as_bytes());
    let byte_count = request.fallback_text.len();
    let line_count = request.fallback_text.lines().count();
    let title = title_from_body(&frontmatter.body, frontmatter.metadata.get("name"));
    let summary = frontmatter
        .metadata
        .get("description")
        .map_or_else(|| first_paragraph(&frontmatter.body), Clone::clone)
        .replace('\n', " ");
    let heading_values = headings(request.fallback_text);
    let references = inline_references(request.fallback_text);
    let records = semantic_records(request.fallback_text)?;
    let semantic_hash = semantic_inventory_hash(&records)?;
    let semantic_rows = records
        .iter()
        .map(|record| {
            vec![
                record.category.clone(),
                record.evidence.clone(),
                record.design_refs.clone(),
                record.fallback_policy.clone(),
            ]
        })
        .collect::<Vec<_>>();
    let heading_rows = if heading_values.is_empty() {
        vec![vec![
            "-".to_owned(),
            "-".to_owned(),
            "(no headings extracted)".to_owned(),
        ]]
    } else {
        heading_values
            .iter()
            .map(|heading| {
                vec![
                    heading.level.to_string(),
                    heading.line.to_string(),
                    heading.title.clone(),
                ]
            })
            .collect()
    };
    let reference_rows = if references.is_empty() {
        vec![vec!["(no inline refs extracted)".to_owned()]]
    } else {
        references
            .into_iter()
            .map(|reference| vec![reference])
            .collect()
    };

    let mut rendered = markdown_template_body(request.template)?;
    validate_template(&rendered)?;
    for (slot, value) in [
        (TEMPLATE_FRONTMATTER_SLOT, clean_frontmatter(&frontmatter)),
        (TEMPLATE_FALLBACK_SLOT, fallback.clone()),
        (TEMPLATE_FALLBACK_HASH_SLOT, fallback_hash.clone()),
        (TEMPLATE_BYTE_COUNT_SLOT, byte_count.to_string()),
        (TEMPLATE_LINE_COUNT_SLOT, line_count.to_string()),
        (TEMPLATE_SEMANTIC_HASH_SLOT, semantic_hash.clone()),
        (TEMPLATE_TITLE_SLOT, title),
        (TEMPLATE_ROOT_SLOT, request.root_name.to_owned()),
        (TEMPLATE_SOURCE_SLOT, source),
        (TEMPLATE_HASH_SLOT, fallback_hash),
        (TEMPLATE_SUMMARY_SLOT, summary),
    ] {
        replace_required_slot(&mut rendered, slot, &value)?;
    }
    replace_required_line(
        &mut rendered,
        TEMPLATE_SEMANTIC_ROW,
        &markdown_table_rows(&semantic_rows),
    )?;
    replace_required_line(
        &mut rendered,
        TEMPLATE_HEADING_ROW,
        &markdown_table_rows(&heading_rows),
    )?;
    replace_required_line(
        &mut rendered,
        TEMPLATE_REFERENCE_ROW,
        &markdown_table_rows(&reference_rows),
    )?;
    for marker in TEMPLATE_ROW_MARKERS {
        if rendered.contains(marker) {
            return Err(RenderError::Template(format!(
                "rendered entry retains template row marker {marker:?}"
            )));
        }
    }
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered)
}

fn markdown_template_body(template: &str) -> Result<String, RenderError> {
    let marker = "```markdown";
    let start = template
        .find(marker)
        .ok_or_else(|| RenderError::Template("template has no markdown fence".to_owned()))?;
    let content_start = template[start + marker.len()..]
        .find('\n')
        .map(|offset| start + marker.len() + offset + 1)
        .ok_or_else(|| RenderError::Template("template fence has no body".to_owned()))?;
    let end = template[content_start..]
        .find("\n```")
        .map(|offset| content_start + offset)
        .ok_or_else(|| RenderError::Template("template fence is not closed".to_owned()))?;
    Ok(template[content_start..end].to_owned())
}

fn validate_template(template: &str) -> Result<(), RenderError> {
    for required in REQUIRED_TEMPLATE_CONTENT {
        if !template.contains(required) {
            return Err(RenderError::Template(format!(
                "template is missing required content {required:?}"
            )));
        }
    }
    for marker in [
        TEMPLATE_FRONTMATTER_SLOT,
        TEMPLATE_FALLBACK_SLOT,
        TEMPLATE_FALLBACK_HASH_SLOT,
        TEMPLATE_BYTE_COUNT_SLOT,
        TEMPLATE_LINE_COUNT_SLOT,
        TEMPLATE_SEMANTIC_HASH_SLOT,
        TEMPLATE_TITLE_SLOT,
        TEMPLATE_ROOT_SLOT,
        TEMPLATE_SOURCE_SLOT,
        TEMPLATE_HASH_SLOT,
        TEMPLATE_SUMMARY_SLOT,
        TEMPLATE_SEMANTIC_ROW,
        TEMPLATE_HEADING_ROW,
        TEMPLATE_REFERENCE_ROW,
    ] {
        if !template.contains(marker) {
            return Err(RenderError::Template(format!(
                "template is missing required slot {marker:?}"
            )));
        }
    }
    for marker in TEMPLATE_ROW_MARKERS {
        let count = template.match_indices(marker).count();
        if count != 1 {
            return Err(RenderError::Template(format!(
                "template row marker {marker:?} must appear exactly once, found {count}"
            )));
        }
    }
    let direct_slots = BTreeSet::from([
        TEMPLATE_FRONTMATTER_SLOT,
        TEMPLATE_FALLBACK_SLOT,
        TEMPLATE_FALLBACK_HASH_SLOT,
        TEMPLATE_BYTE_COUNT_SLOT,
        TEMPLATE_LINE_COUNT_SLOT,
        TEMPLATE_SEMANTIC_HASH_SLOT,
        TEMPLATE_TITLE_SLOT,
        TEMPLATE_ROOT_SLOT,
        TEMPLATE_SOURCE_SLOT,
        TEMPLATE_HASH_SLOT,
        TEMPLATE_SUMMARY_SLOT,
    ]);
    let mut template_without_row_markers = template.to_owned();
    for marker in TEMPLATE_ROW_MARKERS {
        template_without_row_markers = template_without_row_markers.replacen(marker, "", 1);
    }
    let placeholders = Regex::new(r"\{[^}\r\n]+\}").expect("static placeholder regex");
    for found in placeholders.find_iter(&template_without_row_markers) {
        if !direct_slots.contains(found.as_str()) {
            return Err(RenderError::Template(format!(
                "template has unsupported slot {:?}",
                found.as_str()
            )));
        }
    }
    Ok(())
}

fn replace_required_slot(
    template: &mut String,
    slot: &str,
    value: &str,
) -> Result<(), RenderError> {
    if !template.contains(slot) {
        return Err(RenderError::Template(format!(
            "template is missing required slot {slot:?}"
        )));
    }
    *template = template.replace(slot, value);
    Ok(())
}

fn replace_required_line(
    template: &mut String,
    marker: &str,
    replacement: &str,
) -> Result<(), RenderError> {
    let count = template.match_indices(marker).count();
    if count != 1 {
        return Err(RenderError::Template(format!(
            "template row marker {marker:?} must appear exactly once, found {count}"
        )));
    }
    let index = template
        .find(marker)
        .expect("an exactly-once marker has a first occurrence");
    template.replace_range(index..index + marker.len(), replacement);
    Ok(())
}

#[derive(Clone, Debug)]
struct Heading {
    level: usize,
    line: usize,
    title: String,
}

fn headings(text: &str) -> Vec<Heading> {
    let pattern = Regex::new(r"(?m)^(#{1,6})[ \t]+(.+?)[ \t]*$").expect("static heading regex");
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            pattern.captures(line).map(|captures| Heading {
                level: captures[1].len(),
                line: index + 1,
                title: captures[2].split_whitespace().collect::<Vec<_>>().join(" "),
            })
        })
        .take(HEADING_LIMIT)
        .collect()
}

fn inline_references(text: &str) -> Vec<String> {
    let pattern = Regex::new(r"`([^`\r\n]+)`").expect("static inline reference regex");
    pattern
        .captures_iter(text)
        .filter_map(|captures| {
            let value = captures[1].trim();
            (!value.is_empty()
                && (value.contains(".md")
                    || value.starts_with("ae-sdd ")
                    || value.contains('/')
                    || value.contains('\\')))
            .then(|| value.to_owned())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(REFERENCE_LIMIT)
        .collect()
}

struct SemanticCategory {
    id: &'static str,
    patterns: &'static [&'static str],
    design_refs: &'static [&'static str],
    fallback_policy: &'static str,
}

const SEMANTIC_CATEGORIES: [SemanticCategory; 9] = [
    SemanticCategory {
        id: "identity_trigger",
        patterns: &[
            "trigger",
            "use when",
            "适用",
            "触发",
            "入口",
            "路由到",
            "调用",
        ],
        design_refs: &[
            "source/docs/ae-sdd-design.md §2/§16/§18",
            "source/docs/skill-runtime-compiler.md §2",
        ],
        fallback_policy: "Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback.",
    },
    SemanticCategory {
        id: "workflow_route",
        patterns: &[
            "workflow",
            "route",
            "Step\\s*\\d+",
            "Phase",
            "流程",
            "路由",
            "阶段",
            "状态机",
            "子链",
        ],
        design_refs: &[
            "source/docs/ae-sdd-design.md §2/§16",
            "source/standards/update-graph.json",
        ],
        fallback_policy: "Index the route/workflow outline; load fallback before executing low-frequency branch detail.",
    },
    SemanticCategory {
        id: "gate_constraint",
        patterns: &[
            "G-[A-Z0-9-]+",
            "gate",
            "门禁",
            "MUST",
            "必须",
            "禁止",
            "不得",
            "BLOCK",
            "WARN",
            "ASK_USER",
        ],
        design_refs: &[
            "source/docs/ae-sdd-design.md §5",
            "crates/ae-sdd-gates/src/registry.rs:GateRegistry",
        ],
        fallback_policy: "Preserve gate identifiers in index; CLI gate output remains higher authority than prose.",
    },
    SemanticCategory {
        id: "tool_command",
        patterns: &[
            "ae-sdd\\s+[A-Za-z0-9_-]+",
            "CLI",
            "命令",
            "工具",
            "API",
            "接口",
            "scripts/",
            "tools/",
            "python\\s+",
        ],
        design_refs: &[
            "source/docs/ae-sdd-implementation-architecture.md §4/§5",
            "source/docs/ae-sdd-design.md §13",
        ],
        fallback_policy: "Index command/API references; full invocation contracts stay in fallback or implementation docs.",
    },
    SemanticCategory {
        id: "state_data",
        patterns: &[
            "state\\.json",
            "\\bphase\\b",
            "状态",
            "字段",
            "JSON",
            "YAML",
            "config",
            "reviewConsensus",
            "manifest",
        ],
        design_refs: &[
            "source/docs/ae-sdd-design.md §3/§15/§19",
            "crates/ae-sdd-store/src (StateAuthority)",
        ],
        fallback_policy: "Index state/config vocabulary; use CLI state output as execution truth.",
    },
    SemanticCategory {
        id: "output_doc_contract",
        patterns: &[
            "输出",
            "产出",
            "文档",
            "模板",
            "保存",
            "落地",
            "ChangeLog",
            "report",
            "artifact",
            "finalize",
        ],
        design_refs: &["source/docs/ae-sdd-design.md §7", "source/templates/**"],
        fallback_policy: "Index document/output obligations; load fallback before generating exact long-form artifacts.",
    },
    SemanticCategory {
        id: "resource_reference",
        patterns: &[
            "source/",
            "standards/",
            "templates/",
            "skills/",
            "assets/",
            "\\.md",
            "fallback",
        ],
        design_refs: &[
            "source/standards/**",
            "source/templates/**",
            "source/skills/**",
        ],
        fallback_policy: "Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor.",
    },
    SemanticCategory {
        id: "design_alignment",
        patterns: &[
            "设计",
            "实现",
            "对齐",
            "update-check",
            "UC-\\d+",
            "Runtime IR",
            "architecture",
            "§",
        ],
        design_refs: &[
            "source/docs/ae-sdd-design.md",
            "source/docs/ae-sdd-implementation-architecture.md",
            "source/docs/skill-runtime-compiler.md",
        ],
        fallback_policy: "Index the alignment surface; update design docs before changing behavior.",
    },
    SemanticCategory {
        id: "fallback_only_detail",
        patterns: &[
            "示例",
            "例子",
            "FAQ",
            "历史",
            "背景",
            "变更",
            "CHANGELOG",
            "rationale",
            "说明",
        ],
        design_refs: &["source/skill-fallbacks/**", "source/CHANGELOG/**"],
        fallback_policy: "Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail.",
    },
];

#[derive(Clone, Debug)]
struct SemanticRecord {
    category: String,
    evidence: String,
    design_refs: String,
    fallback_policy: String,
}

fn semantic_records(text: &str) -> Result<Vec<SemanticRecord>, RenderError> {
    let metadata = parse_frontmatter(text)
        .map_err(RenderError::Frontmatter)?
        .map_or_else(BTreeMap::new, |frontmatter| frontmatter.metadata);
    let heading_values = headings(text);
    let references = inline_references(text);
    let mut records = Vec::new();

    for category in SEMANTIC_CATEGORIES {
        let pattern = Regex::new(&format!(
            "(?i){}",
            category
                .patterns
                .iter()
                .map(|pattern| format!("(?:{pattern})"))
                .collect::<Vec<_>>()
                .join("|")
        ))
        .expect("static semantic regex");
        let mut evidence = Vec::new();
        let critical = critical_tokens(category.id, text, &metadata, &references)?;
        if !critical.is_empty() {
            evidence.push(format!("critical: {}", critical.join(", ")));
        }
        if category.id == "identity_trigger" {
            let keys = ["name", "description", "version"]
                .into_iter()
                .filter(|key| metadata.contains_key(*key))
                .collect::<Vec<_>>();
            if !keys.is_empty() {
                evidence.push(format!("frontmatter: {}", keys.join(", ")));
            }
        }
        if category.id == "resource_reference" && !references.is_empty() {
            evidence.push(format!("inline_refs: {}", references.len()));
            evidence.push(format!("refs: {}", short_join(&references)));
        }
        let matching_headings = heading_values
            .iter()
            .filter(|heading| pattern.is_match(&heading.title))
            .map(|heading| format!("L{}:{} {}", heading.level, heading.line, heading.title))
            .collect::<Vec<_>>();
        if !matching_headings.is_empty() {
            evidence.push(format!("headings: {}", short_join(&matching_headings)));
        }
        let hits = pattern.find_iter(text).count();
        if hits > 0 {
            evidence.push(format!("keyword_hits: {hits}"));
        }
        let evidence = short_join(&evidence);
        if !evidence.is_empty() {
            records.push(SemanticRecord {
                category: category.id.to_owned(),
                evidence,
                design_refs: category.design_refs.join("; "),
                fallback_policy: category.fallback_policy.to_owned(),
            });
        }
    }

    if !records
        .iter()
        .any(|record| record.category == "identity_trigger")
    {
        records.insert(
            0,
            SemanticRecord {
                category: "identity_trigger".to_owned(),
                evidence: "implicit source entry identity".to_owned(),
                design_refs: "source/docs/skill-runtime-compiler.md §2".to_owned(),
                fallback_policy:
                    "Keep source identity in slim metadata; load fallback for exact wording."
                        .to_owned(),
            },
        );
    }
    Ok(records)
}

fn critical_tokens(
    category: &str,
    text: &str,
    metadata: &BTreeMap<String, String>,
    references: &[String],
) -> Result<Vec<String>, RenderError> {
    let mut tokens = BTreeSet::new();
    match category {
        "gate_constraint" => {
            let pattern = Regex::new(r"(?i)\bG-[A-Z0-9-]+\b").expect("static gate regex");
            tokens.extend(
                pattern
                    .find_iter(text)
                    .map(|found| found.as_str().to_ascii_uppercase()),
            );
        }
        "tool_command" => {
            let pattern = Regex::new(r"(?m)\bae-sdd(?:-build)?(?:[ \t]+[A-Za-z0-9_.-]+){0,3}")
                .expect("static command regex");
            tokens.extend(pattern.find_iter(text).map(|found| {
                found
                    .as_str()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            }));
            if let Some(allowed_tools) = metadata.get("allowed_tools") {
                tokens.extend(
                    allowed_tools
                        .split(',')
                        .map(str::trim)
                        .filter(|tool| !tool.is_empty())
                        .map(|tool| format!("allowed_tools={tool}")),
                );
            }
        }
        "state_data" => {
            let pattern = Regex::new(
                r"\b(?:[A-Za-z][A-Za-z0-9_]*(?:Id|Key|Digest|Token|Ref|State)|phase|revision|fencing|state\.json|[A-Za-z0-9_.-]+\.json)\b",
            )
            .expect("static state token regex");
            tokens.extend(
                pattern
                    .find_iter(text)
                    .map(|found| found.as_str().to_owned()),
            );
        }
        "output_doc_contract" => {
            let pattern =
                Regex::new(r"(?i)\b(?:[A-Za-z0-9_-]+/)*[A-Za-z0-9_.-]+\.(?:md|json|ya?ml)\b")
                    .expect("static output path regex");
            tokens.extend(
                pattern
                    .find_iter(text)
                    .map(|found| found.as_str().to_owned()),
            );
        }
        "resource_reference" => tokens.extend(references.iter().cloned()),
        _ => {}
    }
    if tokens.len() > CRITICAL_TOKEN_LIMIT {
        return Err(RenderError::Semantic(format!(
            "{category} has {} critical references, exceeding the {CRITICAL_TOKEN_LIMIT} token limit",
            tokens.len()
        )));
    }
    Ok(tokens.into_iter().collect())
}

fn semantic_inventory_hash(records: &[SemanticRecord]) -> Result<String, RenderError> {
    let values = records
        .iter()
        .map(|record| {
            BTreeMap::from([
                ("category", record.category.as_str()),
                ("design_refs", record.design_refs.as_str()),
                ("evidence", record.evidence.as_str()),
                ("fallback_policy", record.fallback_policy.as_str()),
            ])
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&values)
        .map(sha256_hex)
        .map_err(RenderError::Serialize)
}

fn title_from_body(body: &str, fallback: Option<&String>) -> String {
    let heading = Regex::new(r"(?m)^#[ \t]+(.+?)[ \t]*$").expect("static title regex");
    heading
        .captures_iter(body)
        .next()
        .map(|captures| captures[1].trim().to_owned())
        .or_else(|| fallback.cloned())
        .unwrap_or_else(|| "Source SKILL".to_owned())
}

fn first_paragraph(body: &str) -> String {
    let mut lines = Vec::new();
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line == "---"
            || line.starts_with("```")
            || line.starts_with('|')
        {
            if !lines.is_empty() {
                break;
            }
            continue;
        }
        lines.push(line);
        if lines.join(" ").len() >= 500 {
            break;
        }
    }
    truncate_utf8(&lines.join(" "), 500)
}

fn truncate_utf8(value: &str, maximum_bytes: usize) -> String {
    if value.len() <= maximum_bytes {
        return value.to_owned();
    }
    let mut end = maximum_bytes.saturating_sub(3);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", value[..end].trim_end())
}

fn markdown_table_rows(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| {
            format!(
                "| {} |",
                row.iter()
                    .map(|value| table_cell(value))
                    .collect::<Vec<_>>()
                    .join(" | ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn table_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_owned()
}

fn short_join(values: &[String]) -> String {
    let selected = values
        .iter()
        .filter(|value| !value.is_empty())
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return String::new();
    }
    let suffix = if values.len() > selected.len() {
        format!("; +{} more", values.len() - selected.len())
    } else {
        String::new()
    };
    format!("{}{}", selected.join("; "), suffix)
}

fn sha256_hex(input: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(input.as_ref()))
}

fn path_to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_stops_at_a_utf8_character_boundary() {
        let value = "测".repeat(600);
        let truncated = truncate_utf8(&value, 500);
        assert!(truncated.ends_with("..."));
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn heading_extraction_scans_each_markdown_line() {
        let body = "intro\n\n# First heading\n\n## Second heading\n";
        let extracted = headings(body);

        assert_eq!(title_from_body(body, None), "First heading");
        assert_eq!(extracted.len(), 2);
        assert_eq!(extracted[0].line, 3);
        assert_eq!(extracted[1].title, "Second heading");
    }

    #[test]
    fn renderer_keeps_critical_semantic_identifiers_visible_without_duplicate_headers() {
        let fallback = "---\nname: example\nallowed_tools:\n  - \"ae-sdd\" # native CLI\n  - Bash\n---\n\n# Example\n\nPass G-14 before continuing.\nRun `ae-sdd state transition`.\nPersist `workItemId` and `revision`.\nWrite `ae-sdd-doc/Story/Story-WriterReport.md`.\n";
        let rendered = render_slim_entry(RenderRequest {
            root_name: "source",
            source: Path::new("skills/example.md"),
            fallback: Path::new("skill-fallbacks/skills/example.full.md"),
            fallback_text: fallback,
            template: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../source/templates/skill/source-skill-slim-entry-template.md"
            )),
        })
        .expect("rendered entry");

        for identifier in [
            "G-14",
            "ae-sdd state transition",
            "allowed_tools=ae-sdd",
            "workItemId",
            "revision",
            "ae-sdd-doc/Story/Story-WriterReport.md",
        ] {
            assert!(rendered.contains(identifier), "missing {identifier}");
        }
        assert_eq!(
            rendered
                .matches("| category | evidence | design_refs | fallback_policy |")
                .count(),
            1
        );
        assert_eq!(rendered.matches("| level | line | title |").count(), 1);
        assert_eq!(rendered.matches("| ref |").count(), 1);
    }
}
