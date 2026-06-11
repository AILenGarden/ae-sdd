#!/usr/bin/env python3
"""Scan Java test sources and Maven reports for fake-test risk patterns.

The scanner is intentionally conservative: BLOCKER findings make the current
test report invalid until fixed or explicitly reviewed. It has no third-party
dependencies so it can run in local agent sessions and CI.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
import xml.etree.ElementTree as ET
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable


TEST_SUFFIXES = ("Test.java", "Tests.java", "TestCase.java", "IT.java", "ITCase.java")


@dataclass
class Finding:
    severity: str
    rule: str
    path: str
    line: int
    message: str
    snippet: str


@dataclass
class ReportStats:
    suites: int = 0
    tests: int = 0
    failures: int = 0
    errors: int = 0
    skipped: int = 0


LINE_RULES = [
    ("BLOCKER", "disabled-test", re.compile(r"@(Disabled|Ignore)\b"), "Test is disabled or ignored."),
    (
        "BLOCKER",
        "assumption-skip",
        re.compile(r"\b(?:Assume\.|Assumptions\.)?assume(?:True|False)\s*\(\s*(?:false|true)\s*\)"),
        "Assumption can skip the test instead of validating behavior.",
    ),
    (
        "BLOCKER",
        "literal-assert-true",
        re.compile(r"\b(?:Assertions\.|Assert\.)?assertTrue\s*\(\s*true\s*\)"),
        "assertTrue(true) is an always-pass assertion.",
    ),
    (
        "BLOCKER",
        "literal-assert-false",
        re.compile(r"\b(?:Assertions\.|Assert\.)?assertFalse\s*\(\s*false\s*\)"),
        "assertFalse(false) is an always-pass assertion.",
    ),
    (
        "BLOCKER",
        "literal-assert-not-null",
        re.compile(r"\b(?:Assertions\.|Assert\.)?assertNotNull\s*\(\s*new\s+Object\s*\("),
        "assertNotNull(new Object()) does not assert tested behavior.",
    ),
    (
        "BLOCKER",
        "thread-sleep",
        re.compile(r"\bThread\.sleep\s*\(|\bTimeUnit\.[A-Z_]+\.sleep\s*\("),
        "Fixed sleeps are banned in tests; use a deterministic wait such as Awaitility or CountDownLatch.",
    ),
    (
        "BLOCKER",
        "deep-stubs",
        re.compile(r"\bRETURNS_DEEP_STUBS\b"),
        "Deep stubs usually test mocks instead of business behavior.",
    ),
    (
        "BLOCKER",
        "mock-any-return",
        re.compile(r"\bthenReturn\s*\(\s*(?:Mockito\.)?any(?:\w*)?\s*\("),
        "Mock returns an argument matcher instead of a concrete value.",
    ),
]


SAME_VALUE_ASSERT = re.compile(
    r"\b(?:Assertions\.|Assert\.)?assert(?:Equals|Same)\s*\(\s*([A-Za-z_][\w.]*)\s*,\s*\1\s*(?:,|\))"
)
SAME_VALUE_ASSERT_THAT = re.compile(
    r"\bassertThat\s*\(\s*([A-Za-z_][\w.]*)\s*\)\s*\.isEqualTo\s*\(\s*\1\s*\)"
)
EMPTY_OR_RETURNING_CATCH = re.compile(
    r"catch\s*\([^)]*\)\s*\{\s*(?:(?://[^\n]*\n\s*)|(?:/\*.*?\*/\s*))*"
    r"(?:(return\s*;)\s*)?\}",
    re.DOTALL,
)
TEST_METHOD_BLOCK = re.compile(
    r"@Test\b(?P<body>.*?)(?=\n\s*@Test\b|\n\s*(?:public\s+)?class\s+|\Z)",
    re.DOTALL,
)
ASSERT_OR_VERIFY = re.compile(
    r"\b(assert\w*|assertThat|verify\s*\(|then\s*\(|expectThrows|assertThrows|ExpectedException)\b"
)


def rel(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root)).replace("\\", "/")
    except ValueError:
        return str(path).replace("\\", "/")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def line_no(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def add_finding(
    findings: list[Finding],
    severity: str,
    rule: str,
    path: Path,
    root: Path,
    line: int,
    message: str,
    snippet: str,
) -> None:
    findings.append(
        Finding(
            severity=severity,
            rule=rule,
            path=rel(path, root),
            line=line,
            message=message,
            snippet=snippet.strip()[:220],
        )
    )


def iter_java_tests(root: Path) -> Iterable[Path]:
    for path in root.rglob("*.java"):
        parts = {p.lower() for p in path.parts}
        if "src" in parts and "test" in parts:
            yield path


def scan_java_tests(root: Path) -> tuple[list[Finding], int]:
    findings: list[Finding] = []
    test_files = 0
    for path in iter_java_tests(root):
        test_files += 1
        text = read_text(path)
        lines = text.splitlines()

        for idx, source_line in enumerate(lines, start=1):
            for severity, rule, pattern, message in LINE_RULES:
                if pattern.search(source_line):
                    add_finding(findings, severity, rule, path, root, idx, message, source_line)
            if SAME_VALUE_ASSERT.search(source_line):
                add_finding(
                    findings,
                    "BLOCKER",
                    "same-value-assert",
                    path,
                    root,
                    idx,
                    "Expected value and actual value appear to be the same variable.",
                    source_line,
                )
            if SAME_VALUE_ASSERT_THAT.search(source_line):
                add_finding(
                    findings,
                    "BLOCKER",
                    "same-value-assert-that",
                    path,
                    root,
                    idx,
                    "assertThat(actual).isEqualTo(actual) is self-proving.",
                    source_line,
                )

        for match in EMPTY_OR_RETURNING_CATCH.finditer(text):
            body = match.group(0)
            add_finding(
                findings,
                "BLOCKER",
                "empty-or-returning-catch",
                path,
                root,
                line_no(text, match.start()),
                "catch block swallows the exception or returns without asserting it.",
                body,
            )

        if "@Test" in text and not path.name.endswith(TEST_SUFFIXES):
            add_finding(
                findings,
                "WARN",
                "test-class-name",
                path,
                root,
                1,
                "File contains @Test but does not match common Surefire/Failsafe naming patterns.",
                path.name,
            )

        for match in TEST_METHOD_BLOCK.finditer(text):
            body = match.group("body")
            if "expected =" not in body and not ASSERT_OR_VERIFY.search(body):
                add_finding(
                    findings,
                    "WARN",
                    "test-without-assertion",
                    path,
                    root,
                    line_no(text, match.start()),
                    "Test block has no visible assertion, verification, or expected exception.",
                    body.splitlines()[0] if body.strip() else "@Test",
                )

    return findings, test_files


def scan_poms(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    for path in root.rglob("pom.xml"):
        text = read_text(path)
        for pattern, rule, message in [
            (r"<skipTests>\s*true\s*</skipTests>", "maven-skip-tests", "pom.xml enables skipTests."),
            (
                r"<maven\.test\.skip>\s*true\s*</maven\.test\.skip>",
                "maven-test-skip",
                "pom.xml enables maven.test.skip.",
            ),
            (
                r"<testFailureIgnore>\s*true\s*</testFailureIgnore>",
                "maven-test-failure-ignore",
                "pom.xml ignores test failures.",
            ),
        ]:
            for match in re.finditer(pattern, text, flags=re.IGNORECASE):
                add_finding(
                    findings,
                    "BLOCKER",
                    rule,
                    path,
                    root,
                    line_no(text, match.start()),
                    message,
                    match.group(0),
                )

        for match in re.finditer(r"<excludes>\s*<exclude>.*?</exclude>\s*</excludes>", text, flags=re.DOTALL):
            add_finding(
                findings,
                "WARN",
                "maven-test-excludes",
                path,
                root,
                line_no(text, match.start()),
                "pom.xml excludes tests; the exclusion must be justified in the test report.",
                match.group(0),
            )
    return findings


def parse_int(value: str | None) -> int:
    if value is None:
        return 0
    try:
        return int(float(value))
    except ValueError:
        return 0


def scan_test_reports(root: Path, require_reports: bool) -> tuple[list[Finding], ReportStats]:
    findings: list[Finding] = []
    stats = ReportStats()
    report_files = list(root.rglob("target/surefire-reports/TEST-*.xml")) + list(
        root.rglob("target/failsafe-reports/TEST-*.xml")
    )

    if require_reports and not report_files:
        findings.append(
            Finding(
                severity="BLOCKER",
                rule="missing-test-xml",
                path=rel(root, root),
                line=0,
                message="No Surefire/Failsafe TEST-*.xml files found after test execution.",
                snippet="target/surefire-reports/TEST-*.xml",
            )
        )
        return findings, stats

    for path in report_files:
        try:
            suite = ET.parse(path).getroot()
        except ET.ParseError as exc:
            add_finding(
                findings,
                "BLOCKER",
                "invalid-test-xml",
                path,
                root,
                1,
                f"Cannot parse test XML: {exc}",
                "",
            )
            continue

        stats.suites += 1
        stats.tests += parse_int(suite.attrib.get("tests"))
        stats.failures += parse_int(suite.attrib.get("failures"))
        stats.errors += parse_int(suite.attrib.get("errors"))
        stats.skipped += parse_int(suite.attrib.get("skipped"))

    if stats.failures or stats.errors:
        findings.append(
            Finding(
                severity="BLOCKER",
                rule="test-xml-failures",
                path="target/*-reports/TEST-*.xml",
                line=0,
                message=f"Test XML contains failures={stats.failures}, errors={stats.errors}.",
                snippet="",
            )
        )
    if stats.skipped:
        findings.append(
            Finding(
                severity="BLOCKER",
                rule="test-xml-skipped",
                path="target/*-reports/TEST-*.xml",
                line=0,
                message=f"Test XML contains skipped={stats.skipped}; skipped tests require explicit Story-approved skip records.",
                snippet="",
            )
        )

    return findings, stats


def render_markdown(root: Path, findings: list[Finding], test_files: int, stats: ReportStats) -> str:
    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    status = "PASS" if blockers == 0 else "FAIL"
    lines = [
        "# Test Authenticity Scan Report",
        "",
        "## Summary",
        "",
        "| Item | Value |",
        "|---|---|",
        f"| Root | `{root}` |",
        f"| Status | `{status}` |",
        f"| Java test files scanned | {test_files} |",
        f"| BLOCKER findings | {blockers} |",
        f"| WARN findings | {warnings} |",
        f"| XML suites | {stats.suites} |",
        f"| XML tests | {stats.tests} |",
        f"| XML failures | {stats.failures} |",
        f"| XML errors | {stats.errors} |",
        f"| XML skipped | {stats.skipped} |",
        "",
        "## Findings",
        "",
    ]
    if not findings:
        lines.append("No findings.")
    else:
        lines.extend(["| Severity | Rule | Location | Message | Snippet |", "|---|---|---|---|---|"])
        for f in findings:
            location = f"`{f.path}:{f.line}`" if f.line else f"`{f.path}`"
            snippet = html.escape(f.snippet).replace("|", "\\|").replace("\n", " ")
            lines.append(f"| {f.severity} | `{f.rule}` | {location} | {f.message} | `{snippet}` |")
    lines.append("")
    lines.append("BLOCKER findings make the current test report invalid until fixed or explicitly reviewed.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Scan Java tests for fake-test risk patterns.")
    parser.add_argument("--root", default=".", help="Repository or module root to scan.")
    parser.add_argument("--output", help="Write the scan report to this file.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument(
        "--require-reports",
        action="store_true",
        help="Fail when Surefire/Failsafe XML reports are missing.",
    )
    args = parser.parse_args()

    root = Path(args.root).resolve()
    findings, test_files = scan_java_tests(root)
    findings.extend(scan_poms(root))
    report_findings, stats = scan_test_reports(root, args.require_reports)
    findings.extend(report_findings)
    findings.sort(key=lambda f: (0 if f.severity == "BLOCKER" else 1, f.path, f.line, f.rule))

    if args.format == "json":
        payload = {
            "root": str(root),
            "status": "PASS" if not any(f.severity == "BLOCKER" for f in findings) else "FAIL",
            "javaTestFiles": test_files,
            "reportStats": asdict(stats),
            "findings": [asdict(f) for f in findings],
        }
        output = json.dumps(payload, ensure_ascii=False, indent=2)
    else:
        output = render_markdown(root, findings, test_files, stats)

    if args.output:
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)

    return 1 if any(f.severity == "BLOCKER" for f in findings) else 0


if __name__ == "__main__":
    raise SystemExit(main())
