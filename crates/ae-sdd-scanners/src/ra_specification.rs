//! Requirement Analysis (RA) Specification v2 bounded parser and scanners.
//!
//! This module owns the single deterministic parser used by the RA content
//! gates (`G-RA-1..4`). It replaces the historical `contains()` / heading-count
//! heuristics with a structured section/table index so gates reason over a
//! shared fact instead of reimplementing weak substring copies.
//!
//! The parser scans the bounded subset declared by the SRS template (§0 identity,
//! §3 applicability, §4 requirements, §5 acceptance, §6 gaps, §7 scale) in a
//! single pass — no third-party Markdown dependency.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use ae_sdd_domain::{ProjectRelativePath, WorkScale};
use regex::Regex;

use crate::{FindingSeverity, ScannerFinding};

/// Upper bound on parsed input length, keeping the parser O(n) and bounded.
const MAX_RA_BYTES: usize = 4 * 1024 * 1024;
const MAX_RA_LINES: usize = 20_000;

/// The seven conditional dimensions declared by the SRS §3 applicability table.
///
/// Kept as a closed enum so the parser and the applicability scanner reason
/// over the exact same key set; ordering is stable (definition order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RaLens {
    Participants,
    Scenarios,
    StateLifecycle,
    DataSemantics,
    ExternalContracts,
    QualitySecurityCompliance,
    CompatibilityMigrationOperations,
}

impl RaLens {
    /// Stable canonical key as written in the SRS §3 applicability table.
    pub const fn as_key(self) -> &'static str {
        match self {
            Self::Participants => "participants",
            Self::Scenarios => "scenarios",
            Self::StateLifecycle => "state_lifecycle",
            Self::DataSemantics => "data_semantics",
            Self::ExternalContracts => "external_contracts",
            Self::QualitySecurityCompliance => "quality_security_compliance",
            Self::CompatibilityMigrationOperations => "compatibility_migration_operations",
        }
    }

    pub const ALL: [Self; 7] = [
        Self::Participants,
        Self::Scenarios,
        Self::StateLifecycle,
        Self::DataSemantics,
        Self::ExternalContracts,
        Self::QualitySecurityCompliance,
        Self::CompatibilityMigrationOperations,
    ];

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|lens| lens.as_key() == key)
    }
}

/// Applicability status recorded against each conditional dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    Applicable,
    NotApplicable,
    Unknown,
}

impl Applicability {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "applicable" => Some(Self::Applicable),
            "not_applicable" => Some(Self::NotApplicable),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Structured projection of a parsed `ae-sdd-ra-srs/v2` document.
///
/// Fields are intentionally minimal and typed so downstream gate scanners can
/// cross-check structure (IDs, traceability, closure) without re-scanning raw
/// text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaSpecificationIndex {
    pub schema: String,
    pub scale: Option<WorkScale>,
    pub confidence: Option<u8>,
    pub analysis_state: Option<String>,
    pub lenses: BTreeMap<RaLens, Applicability>,
    pub requirement_ids: BTreeSet<String>,
    pub acceptance_links: BTreeMap<String, BTreeSet<String>>,
    pub source_links: BTreeMap<String, BTreeSet<String>>,
    pub source_ref_ids: BTreeSet<String>,
    pub blocking_gaps: BTreeSet<String>,
    pub scale_scores: [u8; 6],
    pub rubric_scale: Option<WorkScale>,
    pub conditional_sections: BTreeSet<RaLens>,
}

impl RaSpecificationIndex {
    /// Sentinel schema that every v2 SRS must declare in its §0 identity table.
    pub const SCHEMA: &'static str = "ae-sdd-ra-srs/v2";
}

/// Which scanner class owns a finding, used by `scan_ra_v2` to filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LensCheck {
    Core,
    Applicability,
    Closure,
}

impl LensCheck {
    /// Rule-id prefix owned by this check class.
    fn prefix(self) -> &'static str {
        match self {
            Self::Core => "core-",
            Self::Applicability => "applicability-",
            Self::Closure => "closure-",
        }
    }
}

/// Scan a parsed SRS through one lens, appending only the findings this scanner
/// class owns. The parser is the single source of structural truth; each
/// `ScannerId` (RaCore/RaApplicability/RaClosure) filters the same finding set
/// by rule-id prefix so each gate attributes precisely its own checks.
pub fn scan_ra_v2(
    path: &ProjectRelativePath,
    text: &str,
    check: LensCheck,
    findings: &mut Vec<ScannerFinding>,
) {
    let prefix = check.prefix();
    match parse_ra_specification_at(path, text) {
        Ok(_) => {}
        Err(parse_findings) => findings.extend(
            parse_findings
                .into_iter()
                .filter(|finding| finding.rule.starts_with(prefix))
                .collect::<Vec<_>>(),
        ),
    }
    // A finding-less parse is a pass for this lens.
}

/// Parse an SRS v2 document into a structured index.
///
/// Returns `Ok(index)` when the document is structurally well-formed enough to
/// reason about; returns `Err(findings)` with the precise structural findings
/// when core structure is missing or ambiguous.
pub fn parse_ra_specification(text: &str) -> Result<RaSpecificationIndex, Vec<ScannerFinding>> {
    let sentinel = ProjectRelativePath::new("ra-specification").expect("sentinel path is portable");
    parse_ra_specification_at(&sentinel, text)
}

fn parse_ra_specification_at(
    path: &ProjectRelativePath,
    text: &str,
) -> Result<RaSpecificationIndex, Vec<ScannerFinding>> {
    let mut findings = Vec::new();

    if text.len() > MAX_RA_BYTES {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "core-input-too-many-bytes",
            path.clone(),
            1,
            "RA specification exceeds the bounded parser byte limit.",
        ));
        return Err(findings);
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.len() > MAX_RA_LINES {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "core-input-too-many-lines",
            path.clone(),
            1,
            "RA specification exceeds the bounded parser line limit.",
        ));
        return Err(findings);
    }

    // Sectionize: collect (section_key, body_lines) by walking headings. Keys
    // are the leading "N" or "N.M" after the '#' run. Owned strings keep this
    // `forbid(unsafe_code)` clean.
    let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for (line_index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let hashes = trimmed
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if (2..=3).contains(&hashes) {
            let title = trimmed[hashes..].trim();
            if let Some(key) = section_key(title) {
                let owned = key.to_owned();
                if sections.contains_key(&owned) {
                    findings.push(ScannerFinding::new(
                        FindingSeverity::Blocker,
                        "core-duplicate-section",
                        path.clone(),
                        line_index + 1,
                        "Normative SRS section keys must be unique.",
                    ));
                }
                sections.entry(owned.clone()).or_default();
                current = Some(owned);
                continue;
            }
        }
        if let Some(key) = &current {
            sections
                .entry(key.clone())
                .or_default()
                .push((*line).to_owned());
        }
    }

    for key in ["0", "0.1", "1", "2", "3", "4", "5", "6", "7"] {
        if !sections.contains_key(key) {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                format!("core-missing-section-{key}"),
                path.clone(),
                1,
                format!("SRS Core section §{key} is required."),
            ));
        }
    }
    for key in ["1", "2"] {
        if sections
            .get(key)
            .is_some_and(|body| body.iter().all(|line| line.trim().is_empty()))
        {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                format!("core-empty-section-{key}"),
                path.clone(),
                1,
                format!("SRS Core section §{key} must contain analysis."),
            ));
        }
    }

    static PLACEHOLDER: OnceLock<Regex> = OnceLock::new();
    let placeholder = PLACEHOLDER.get_or_init(|| {
        Regex::new(r"(?i)\b(?:TODO|TBD|FIXME|XXX)\b|\{[^{}\r\n]{1,128}\}|待补充|请填写")
            .expect("placeholder regex")
    });
    for (line_index, line) in lines.iter().enumerate() {
        if placeholder.is_match(line) {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "core-placeholder",
                path.clone(),
                line_index + 1,
                "SRS must not contain unresolved placeholders.",
            ));
        }
    }

    // §0 identity + schema gate.
    let identity = body_for(&sections, "0");
    let schema = cell_value(identity.iter().map(String::as_str), "Schema");
    match schema.as_deref() {
        Some(RaSpecificationIndex::SCHEMA) => {}
        _ => findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "core-schema",
            path.clone(),
            1,
            "SRS must declare the ae-sdd-ra-srs/v2 schema in §0.",
        )),
    }

    let scale = cell_value(identity.iter().map(String::as_str), "Scale")
        .as_deref()
        .and_then(parse_work_scale);
    let confidence = cell_value(identity.iter().map(String::as_str), "Scale confidence")
        .and_then(|value| value.trim().parse::<u8>().ok());
    let analysis_state = cell_value(identity.iter().map(String::as_str), "Analysis state")
        .map(|value| value.trim().to_owned());

    for label in [
        "RA ID",
        "Work Item",
        "Revision",
        "Analysis state",
        "Scale",
        "Scale confidence",
    ] {
        if cell_value(identity.iter().map(String::as_str), label).is_none() {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "core-missing-identity-field",
                path.clone(),
                1,
                format!("SRS §0 identity field `{label}` is required."),
            ));
        }
    }

    // §0.1 declared REF ids.
    let source_rows = checked_rows(
        &body_for(&sections, "0.1"),
        5,
        "REF ID",
        "core-malformed-ref-row",
        path,
        &mut findings,
    );
    let mut source_ref_ids = BTreeSet::new();
    for row in source_rows {
        let ref_id = row[0].trim();
        if !ref_id.starts_with("REF-") {
            findings.push(finding(
                "core-invalid-ref-id",
                path,
                "Source rows must use REF-* ids.",
            ));
        } else if !source_ref_ids.insert(ref_id.to_owned()) {
            findings.push(finding(
                "core-duplicate-ref",
                path,
                "Source reference ids must be unique.",
            ));
        }
    }

    // §3 applicability.
    let applicability_body = body_for(&sections, "3");
    let mut lenses: BTreeMap<RaLens, Applicability> = BTreeMap::new();
    let mut unknown_gap_links: BTreeMap<RaLens, BTreeSet<String>> = BTreeMap::new();
    for row in checked_rows(
        &applicability_body,
        4,
        "条件维度",
        "applicability-malformed-row",
        path,
        &mut findings,
    ) {
        let Some(lens) = RaLens::from_key(row[0].trim()) else {
            findings.push(finding(
                "applicability-unknown-lens",
                path,
                "Applicability rows must name one of the seven canonical dimensions.",
            ));
            continue;
        };
        if lenses.contains_key(&lens) {
            findings.push(finding(
                "applicability-duplicate-lens",
                path,
                "Each applicability dimension must occur exactly once.",
            ));
            continue;
        }
        match Applicability::parse(&row[1]) {
            Some(status) => {
                lenses.insert(lens, status);
                if status == Applicability::Unknown {
                    unknown_gap_links.insert(lens, ids_in(&row[3], "GAP-").into_iter().collect());
                }
            }
            None => findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "applicability-unknown-status",
                path.clone(),
                1,
                "Applicability status must be applicable / not_applicable / unknown.",
            )),
        }
    }
    for lens in RaLens::ALL {
        if !lenses.contains_key(&lens) {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "applicability-missing-lens",
                path.clone(),
                1,
                "SRS §3 must judge every applicability dimension.",
            ));
        }
    }

    // Conditional sections present (§8.1..§8.7).
    let mut conditional_sections: BTreeSet<RaLens> = BTreeSet::new();
    for (key, lens) in [
        ("8.1", RaLens::Participants),
        ("8.2", RaLens::Scenarios),
        ("8.3", RaLens::StateLifecycle),
        ("8.4", RaLens::DataSemantics),
        ("8.5", RaLens::ExternalContracts),
        ("8.6", RaLens::QualitySecurityCompliance),
        ("8.7", RaLens::CompatibilityMigrationOperations),
    ] {
        if sections.contains_key(key) {
            conditional_sections.insert(lens);
        }
    }
    // Applicability <-> conditional section consistency.
    for (lens, status) in &lenses {
        let has_section = conditional_sections.contains(lens);
        match (status, has_section) {
            (Applicability::Applicable, false) => findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "applicability-applicable-missing-section",
                path.clone(),
                1,
                "An applicable dimension must generate its conditional section.",
            )),
            (Applicability::NotApplicable, true) => findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "applicability-not-applicable-has-section",
                path.clone(),
                1,
                "A not_applicable dimension must not generate a conditional section.",
            )),
            (Applicability::Unknown, false) => {
                // Unknown without a section is fine only if a blocking GAP closes it;
                // closure check below verifies open blocking gaps.
            }
            _ => {}
        }
    }

    // §4 requirements: REQ id + source refs column.
    let req_body = body_for(&sections, "4");
    let mut requirement_ids: BTreeSet<String> = BTreeSet::new();
    let mut source_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in checked_rows(
        &req_body,
        5,
        "REQ ID",
        "core-malformed-req-row",
        path,
        &mut findings,
    ) {
        let req_id = row[0].trim();
        if !req_id.starts_with("REQ-") {
            findings.push(finding(
                "core-invalid-req-id",
                path,
                "Requirement rows must use REQ-* ids.",
            ));
            continue;
        }
        if row[1].trim().is_empty() {
            findings.push(finding(
                "core-empty-requirement",
                path,
                "A requirement must contain a normative statement.",
            ));
        }
        if !requirement_ids.insert(req_id.to_owned()) {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "core-duplicate-req",
                path.clone(),
                1,
                "Requirement ids must be unique.",
            ));
        }
        let linked: BTreeSet<String> = ids_in(&row[3], "REF-").into_iter().collect();
        source_links.insert(req_id.to_owned(), linked);
    }

    // §5 acceptance: AC id + covered REQ column.
    let ac_body = body_for(&sections, "5");
    let mut acceptance_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut acceptance_ids = BTreeSet::new();
    let mut acceptance_targets = Vec::new();
    for row in checked_rows(
        &ac_body,
        4,
        "AC ID",
        "core-malformed-ac-row",
        path,
        &mut findings,
    ) {
        let ac_id = row[0].trim();
        if !ac_id.starts_with("AC-") {
            findings.push(finding(
                "core-invalid-ac-id",
                path,
                "Acceptance rows must use AC-* ids.",
            ));
            continue;
        }
        if !acceptance_ids.insert(ac_id.to_owned()) {
            findings.push(finding(
                "core-duplicate-ac",
                path,
                "Acceptance ids must be unique.",
            ));
        }
        if row[3].trim().is_empty() {
            findings.push(finding(
                "core-empty-acceptance",
                path,
                "An acceptance criterion must contain an observable decision.",
            ));
        }
        for req_id in ids_in(&row[1], "REQ-") {
            acceptance_targets.push(req_id.clone());
            acceptance_links
                .entry(req_id)
                .or_default()
                .insert(ac_id.to_owned());
        }
    }

    // §6 gaps: blocking GAP = type==gap && severity blocking && status open.
    let gap_body = body_for(&sections, "6");
    let mut risk_ids = BTreeSet::new();
    let mut gap_ids = BTreeSet::new();
    let mut closed_blocking_gaps = BTreeSet::new();
    let mut blocking_gaps: BTreeSet<String> = BTreeSet::new();
    for row in checked_rows(
        &gap_body,
        5,
        "ID",
        "core-malformed-risk-row",
        path,
        &mut findings,
    ) {
        let id = row[0].trim();
        if !id.is_empty() && !risk_ids.insert(id.to_owned()) {
            findings.push(finding(
                "core-duplicate-risk-id",
                path,
                "Constraint, assumption, risk, conflict, and GAP ids must be unique.",
            ));
        }
        if !id.starts_with("GAP-") {
            continue;
        }
        if !gap_ids.insert(id.to_owned()) {
            findings.push(finding(
                "core-duplicate-gap",
                path,
                "GAP ids must be unique.",
            ));
        }
        let kind = row[1].trim().to_ascii_lowercase();
        let severity = row[3].trim().to_ascii_lowercase();
        let status = row[4].trim().to_ascii_lowercase();
        let is_blocking = kind == "gap"
            && (severity.contains('🔴')
                || severity.contains("阻断")
                || severity.contains("blocking")
                || severity == "blocker");
        let is_closed = ["closed", "resolved", "关闭", "已关闭", "已解决"]
            .iter()
            .any(|marker| status.starts_with(marker));
        if is_blocking {
            if is_closed {
                closed_blocking_gaps.insert(id.to_owned());
            } else {
                blocking_gaps.insert(id.to_owned());
            }
        }
    }

    for links in unknown_gap_links.values() {
        if links.is_empty() {
            findings.push(finding(
                "applicability-unknown-gap-missing",
                path,
                "An unknown applicability dimension must reference a GAP-* closure record.",
            ));
        }
        for gap in links {
            if !gap_ids.contains(gap) {
                findings.push(finding(
                    "applicability-unknown-gap-missing",
                    path,
                    "An unknown applicability dimension references an undeclared GAP.",
                ));
            } else if !closed_blocking_gaps.contains(gap) && !blocking_gaps.contains(gap) {
                findings.push(finding(
                    "applicability-unknown-gap-not-blocking",
                    path,
                    "An unknown applicability dimension must bind a blocking GAP.",
                ));
            }
        }
    }

    // §7 scale: six scores + rubric line "最高分 = N -> Scale = X".
    let scale_body = body_for(&sections, "7");
    let mut scale_scores: [u8; 6] = [0; 6];
    let dimension_names = [
        "可观察行为与场景广度",
        "参与方、权限或业务域广度",
        "状态、数据语义与不变量复杂度",
        "外部契约与协调范围",
        "性能、安全、合规、可用性等质量风险",
        "兼容、迁移、回滚和运行影响",
    ];
    let mut rubric_scale: Option<WorkScale> = None;
    let mut rubric_maximum = None;
    let score_rows = checked_rows(
        &scale_body,
        3,
        "需求维度",
        "closure-scale-malformed-row",
        path,
        &mut findings,
    );
    if score_rows.len() != 6 {
        findings.push(finding(
            "closure-scale-dimension-count",
            path,
            "Scale rubric must contain exactly six dimension rows.",
        ));
    }
    let mut seen_dimensions = BTreeSet::new();
    for row in score_rows {
        let Some(index) = dimension_names.iter().position(|name| *name == row[0]) else {
            findings.push(finding(
                "closure-scale-unknown-dimension",
                path,
                "Scale rubric contains an unknown dimension.",
            ));
            continue;
        };
        if !seen_dimensions.insert(index) {
            findings.push(finding(
                "closure-scale-duplicate-dimension",
                path,
                "Scale dimensions must be unique.",
            ));
        }
        match row[1].parse::<u8>() {
            Ok(score @ 1..=4) => scale_scores[index] = score,
            _ => findings.push(finding(
                "closure-scale-score-range",
                path,
                "Each scale score must be an integer from 1 through 4.",
            )),
        }
    }
    for line in &scale_body {
        if line.trim_start().starts_with("最高分") {
            match rubric_from_line(line) {
                Some((maximum, rubric)) => {
                    rubric_maximum = Some(maximum);
                    rubric_scale = Some(rubric);
                }
                None => findings.push(finding(
                    "closure-scale-rubric-malformed",
                    path,
                    "Scale rubric summary must declare maximum and scale.",
                )),
            }
        }
    }
    if rubric_scale.is_none() {
        findings.push(finding(
            "closure-scale-rubric-missing",
            path,
            "Scale rubric summary is required.",
        ));
    }
    let actual_maximum = scale_scores.iter().copied().max().unwrap_or(0);
    if rubric_maximum.is_some_and(|maximum| maximum != actual_maximum) {
        findings.push(finding(
            "closure-scale-maximum-mismatch",
            path,
            "Declared rubric maximum must equal the six score rows.",
        ));
    }
    let calculated_scale = scale_from_score(actual_maximum);
    if scale != calculated_scale || rubric_scale != calculated_scale {
        findings.push(finding(
            "closure-scale-inconsistent",
            path,
            "Header and rubric scale must equal the actual maximum score.",
        ));
    }

    for req_id in &acceptance_targets {
        if !requirement_ids.contains(req_id) {
            findings.push(finding(
                "closure-ac-unknown-req",
                path,
                "Acceptance criteria may only cover declared requirements.",
            ));
        }
    }
    let referenced_sources: BTreeSet<String> = source_links
        .values()
        .flat_map(|sources| sources.iter().cloned())
        .collect();
    for source in &referenced_sources {
        if !source_ref_ids.contains(source) {
            findings.push(finding(
                "closure-req-unknown-source",
                path,
                "Requirements may only cite declared source references.",
            ));
        }
    }
    for source in &source_ref_ids {
        if !referenced_sources.contains(source) {
            findings.push(finding(
                "closure-source-unreferenced",
                path,
                "Every declared source reference must trace to a requirement.",
            ));
        }
    }

    let index = RaSpecificationIndex {
        schema: schema.unwrap_or_default().trim().to_owned(),
        scale,
        confidence,
        analysis_state,
        lenses,
        requirement_ids,
        acceptance_links,
        source_links,
        source_ref_ids,
        blocking_gaps,
        scale_scores,
        rubric_scale,
        conditional_sections,
    };

    // Closure checks (REQ traceability/acceptance, blocking gap, scale consistency,
    // analysisState). These run after indexing so they reason over structured facts.
    closure_findings(&index, &mut findings, path);

    if findings.is_empty() {
        Ok(index)
    } else {
        Err(findings)
    }
}

fn closure_findings(
    index: &RaSpecificationIndex,
    findings: &mut Vec<ScannerFinding>,
    sentinel: &ProjectRelativePath,
) {
    if index.requirement_ids.is_empty() {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "core-missing-req-section",
            sentinel.clone(),
            1,
            "SRS §4 must declare at least one requirement.",
        ));
    }
    for req in &index.requirement_ids {
        let sources = index.source_links.get(req);
        let has_source = sources.map(|set| !set.is_empty()).unwrap_or(false);
        if !has_source {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "closure-req-missing-source",
                sentinel.clone(),
                1,
                "Each requirement must cite at least one source reference.",
            ));
        }
        let covered = index.acceptance_links.get(req);
        let has_ac = covered.map(|set| !set.is_empty()).unwrap_or(false);
        if !has_ac {
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "closure-req-missing-acceptance",
                sentinel.clone(),
                1,
                "Each requirement must be covered by at least one acceptance criterion.",
            ));
        }
    }
    for gap in &index.blocking_gaps {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "closure-open-blocking-gap",
            sentinel.clone(),
            1,
            "Blocking gaps must be closed before analysisState=complete.",
        ));
        let _ = gap;
    }
    if index.analysis_state.as_deref() != Some("complete") {
        findings.push(finding(
            "closure-analysis-incomplete",
            sentinel,
            "SRS analysisState must be complete before closure.",
        ));
    }
    if !index.confidence.is_some_and(|confidence| confidence <= 100) {
        findings.push(finding(
            "closure-confidence-range",
            sentinel,
            "Scale confidence must be an integer from 0 through 100.",
        ));
    }
}

/// Normalize a heading title to its section key (leading "N" or "N.M").
fn section_key(title: &str) -> Option<&str> {
    let trimmed = title.trim_start();
    let bytes = trimmed.as_bytes();
    let mut end = 0;
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        end += 1;
    }
    // Drop a trailing dot so "0." -> "0" and "3." -> "3" while "0.1" stays "0.1".
    while end > 0 && bytes[end - 1] == b'.' {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    Some(&trimmed[..end])
}

fn body_for(sections: &BTreeMap<String, Vec<String>>, key: &str) -> Vec<String> {
    sections.get(key).cloned().unwrap_or_default()
}

fn finding(
    rule: &'static str,
    path: &ProjectRelativePath,
    message: &'static str,
) -> ScannerFinding {
    ScannerFinding::new(FindingSeverity::Blocker, rule, path.clone(), 1, message)
}

fn checked_rows(
    lines: &[String],
    expected_cells: usize,
    header: &str,
    malformed_rule: &'static str,
    path: &ProjectRelativePath,
    findings: &mut Vec<ScannerFinding>,
) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in lines {
        let Some(row) = table_row(line) else {
            continue;
        };
        if row.first().is_some_and(|cell| cell == header) {
            continue;
        }
        if row.len() != expected_cells {
            findings.push(finding(
                malformed_rule,
                path,
                "SRS table row has an unexpected number of cells; literal pipes must be escaped.",
            ));
            continue;
        }
        rows.push(row);
    }
    rows
}

/// Extract the value cell for a row whose first cell equals `label`.
fn cell_value<'a>(lines: impl Iterator<Item = &'a str>, label: &str) -> Option<String> {
    for line in lines {
        if let Some(row) = table_row(line)
            && row
                .first()
                .map(|first| first.trim() == label)
                .unwrap_or(false)
            && let Some(value) = row.get(1)
        {
            return Some(value.trim().to_owned());
        }
    }
    None
}

/// Iterate table data rows (skip separator/alignment rows) from body lines.
/// Parse a single markdown table row into trimmed cells, or None if not a data row.
fn table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut characters = inner.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' if characters.peek() == Some(&'|') => {
                let _ = characters.next();
                cell.push('|');
            }
            '\\' if characters.peek() == Some(&'\\') => {
                let _ = characters.next();
                cell.push('\\');
            }
            '|' => {
                cells.push(cell.trim().to_owned());
                cell.clear();
            }
            _ => cell.push(character),
        }
    }
    cells.push(cell.trim().to_owned());
    // Separator row like | --- | --- |.
    if cells.iter().all(|cell| {
        let alignment = cell.trim().trim_matches(':');
        !alignment.is_empty() && alignment.chars().all(|character| character == '-')
    }) {
        return None;
    }
    Some(cells)
}

/// Collect all ids with the given prefix from a text cell.
fn ids_in(text: &str, prefix: &str) -> Vec<String> {
    text.split([',', ' ', '/'])
        .filter_map(|token| {
            let token = token.trim();
            token
                .starts_with(prefix)
                .then(|| token.trim_matches([',', '.', ';', ':']).to_owned())
        })
        .collect()
}

fn parse_work_scale(value: &str) -> Option<WorkScale> {
    match value.trim() {
        "micro" => Some(WorkScale::Micro),
        "small" => Some(WorkScale::Small),
        "medium" => Some(WorkScale::Medium),
        "large" => Some(WorkScale::Large),
        _ => None,
    }
}

/// Parse a rubric line like `最高分 = 4 -> Scale = large` into the declared scale.
fn rubric_from_line(line: &str) -> Option<(u8, WorkScale)> {
    let (maximum, scale) = line.split_once("->")?;
    let maximum = maximum.split_once('=')?.1.trim().parse::<u8>().ok()?;
    let marker = "Scale = ";
    let position = scale.find(marker)?;
    let token = scale[position + marker.len()..]
        .trim_start()
        .split([',', '。', '（', ' ', '\t'])
        .next()?;
    Some((maximum, parse_work_scale(token)?))
}

fn scale_from_score(score: u8) -> Option<WorkScale> {
    match score {
        1 => Some(WorkScale::Micro),
        2 => Some(WorkScale::Small),
        3 => Some(WorkScale::Medium),
        4 => Some(WorkScale::Large),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_POSITIVE: &str = "\
# 需求规格说明书：示例

## 0. 文档与需求身份
| 字段 | 值 |
| --- | --- |
| Schema | ae-sdd-ra-srs/v2 |
| RA ID | RA-X-001 |
| Work Item | ROUTE-X |
| Revision | 1 |
| Scale | micro |
| Scale confidence | 90 |
| Analysis state | complete |

### 0.1 来源与实际使用的上下文
| REF ID | 类型 | 引用/摘要 | Digest/版本 | 用途 |
| --- | --- | --- | --- | --- |
| REF-001 | 对话 | 示例 | v1 | 输入 |

## 1. 问题、目标与非目标
问题、目标与非目标均已闭合。

## 2. 范围
范围已闭合。

## 3. 适用性判定
| 条件维度 | 状态 | 依据 | 目标章节/处置 |
| --- | --- | --- | --- |
| participants | not_applicable | 无 | §3 |
| scenarios | not_applicable | 无 | §3 |
| state_lifecycle | not_applicable | 无 | §3 |
| data_semantics | not_applicable | 无 | §3 |
| external_contracts | not_applicable | 无 | §3 |
| quality_security_compliance | not_applicable | 无 | §3 |
| compatibility_migration_operations | not_applicable | 无 | §3 |

## 4. 需求清单
| REQ ID | 规范性需求 | 优先级 | Source refs | 依赖/冲突 |
| --- | --- | --- | --- | --- |
| REQ-001 | 示例需求 | P0 | REF-001 | 无 |

## 5. 验收与追溯
| AC ID | 覆盖 REQ | 验收类型 | 可执行/可观察判定 |
| --- | --- | --- | --- |
| AC-001 | REQ-001 | operational | 可观察 |

## 6. 约束、假设、冲突、风险与未决
| ID | 类型 | 内容 | 严重度 | 状态/处置 |
| --- | --- | --- | --- | --- |
| A-001 | 假设 | 示例 | 中 | 已确认 |

## 7. 规模裁定
| 需求维度 | 评分 1-4 | 证据 |
| --- | --- | --- |
| 可观察行为与场景广度 | 1 | 单一行为 |
| 参与方、权限或业务域广度 | 1 | 无 |
| 状态、数据语义与不变量复杂度 | 1 | 无 |
| 外部契约与协调范围 | 1 | 无 |
| 性能、安全、合规、可用性等质量风险 | 1 | 无 |
| 兼容、迁移、回滚和运行影响 | 1 | 无 |

最高分 = 1 -> Scale = micro。
";

    #[test]
    fn minimal_positive_srs_parses_cleanly() {
        let index = parse_ra_specification(MINIMAL_POSITIVE).expect("positive parses");
        assert_eq!(index.schema, RaSpecificationIndex::SCHEMA);
        assert_eq!(index.scale, Some(WorkScale::Micro));
        assert_eq!(index.rubric_scale, Some(WorkScale::Micro));
        assert_eq!(
            index.requirement_ids,
            ["REQ-001".to_owned()].into_iter().collect()
        );
        assert!(index.blocking_gaps.is_empty());
        assert_eq!(index.analysis_state.as_deref(), Some("complete"));
    }

    #[test]
    fn missing_schema_is_a_core_finding() {
        let without_schema = MINIMAL_POSITIVE.replace("ae-sdd-ra-srs/v2", "old-schema");
        let findings = parse_ra_specification(&without_schema).expect_err("schema missing");
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule.as_ref() == "core-schema")
        );
    }
}
