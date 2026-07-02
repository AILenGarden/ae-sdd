#!/usr/bin/env python3
"""Static quality checker for JUnit5 + Mockito + AssertJ test files.

Heuristic regex-based checks. False positives are possible; the tool is meant
as a guardrail signal, not a compiler. Output: JSON list of findings to stdout.

Usage:
    check_test_quality.py <TestFile.java> [<TestFile.java> ...]
    check_test_quality.py --sut com.example.Foo TestFile.java   # extra check: SUT not mocked

Exit codes: 0 = clean, 1 = findings present, 2 = input error.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


@dataclass
class Finding:
    file: str
    line: int
    rule: str
    severity: str  # "error" | "warn"
    message: str
    snippet: str


# --- Rules ---------------------------------------------------------------

RE_TEST_ANNOTATION = re.compile(r"^\s*@(Test|ParameterizedTest|RepeatedTest)\b")
RE_METHOD_DECL = re.compile(r"^\s*(?:public\s+|private\s+|protected\s+)?(?:static\s+)?\w[\w<>,\s\[\]]*\s+(\w+)\s*\(")
RE_ASSERT_TRUE_LITERAL = re.compile(r"\bassert(True|False)\s*\(\s*(true|false)\s*[\),]")
RE_ASSERT_NOT_NULL_BARE = re.compile(r"\bassert(?:NotNull|Null)\s*\(")
RE_ASSERTJ_NOT_NULL = re.compile(r"\bassertThat\s*\([^)]*\)\s*\.\s*isNotNull\s*\(\s*\)")
RE_ASSERT_ANY = re.compile(
    r"\b(assertThat|assertEquals|assertNotEquals|assertSame|assertNotSame|"
    r"assertTrue|assertFalse|assertNull|assertNotNull|assertThrows|assertDoesNotThrow|"
    r"assertAll|assertArrayEquals|assertIterableEquals|assertLinesMatch|"
    r"verify|verifyNoInteractions|verifyNoMoreInteractions|inOrder|then\s*\([^)]*\)\s*\.should)\b"
)
RE_MOCK_DECL = re.compile(r"@(Mock|Spy|InjectMocks)\b\s+[\w<>,\s\[\]]+\s+(\w+)\s*;")
RE_WHEN = re.compile(r"\bwhen\s*\(\s*(\w+)\s*\.")
RE_PRIVATE_REFLECTION = re.compile(r"\b(setAccessible|getDeclaredMethod|getDeclaredField|ReflectionUtils|ReflectionTestUtils\.invokeMethod)\b")
RE_THREAD_SLEEP = re.compile(r"\bThread\.sleep\s*\(")
RE_CATCH_THROWABLE = re.compile(r"\bcatch\s*\(\s*(Throwable|Exception)\s+\w+\s*\)")
RE_IGNORED = re.compile(r"^\s*@(Disabled|Ignore)\b")
RE_RANDOM_NEW = re.compile(r"\bnew\s+Random\s*\(\s*\)")
RE_NAME_WITH_SHOULD = re.compile(r"^(should|test|when|given)[A-Z_]")  # heuristic: at least starts with verb pattern


def _read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def _iter_test_methods(lines: list[str]):
    """Yield (start_line_idx, end_line_idx_exclusive, method_name) for @Test methods."""
    i = 0
    n = len(lines)
    while i < n:
        if RE_TEST_ANNOTATION.match(lines[i]):
            # find method declaration after annotation block
            j = i + 1
            while j < n and (RE_TEST_ANNOTATION.match(lines[j]) or lines[j].strip().startswith("@")):
                j += 1
            if j < n:
                m = RE_METHOD_DECL.match(lines[j])
                if m:
                    name = m.group(1)
                    # find body end via brace counting
                    depth = 0
                    started = False
                    k = j
                    while k < n:
                        depth += lines[k].count("{")
                        depth -= lines[k].count("}")
                        if "{" in lines[k]:
                            started = True
                        if started and depth <= 0:
                            yield i, k + 1, name
                            break
                        k += 1
                    i = k + 1
                    continue
        i += 1


def check_file(path: Path, sut_fqcn: str | None) -> list[Finding]:
    findings: list[Finding] = []
    lines = _read_lines(path)
    text = "\n".join(lines)

    # File-level: SUT must not be mocked.
    if sut_fqcn:
        sut_simple = sut_fqcn.rsplit(".", 1)[-1]
        # crude: a field annotated @Mock whose declared type is the SUT simple name
        for idx, line in enumerate(lines, 1):
            m = re.match(rf"\s*@Mock\b\s*\n?", line)
            # also check inline form
            if re.search(rf"@Mock\b[^;\n]*\b{sut_simple}\s+\w+\s*;", line) or (
                re.match(r"\s*@Mock\b", line)
                and idx < len(lines)
                and re.search(rf"\b{sut_simple}\s+\w+\s*;", lines[idx])
            ):
                findings.append(Finding(str(path), idx, "sut-mocked", "error",
                                        f"SUT '{sut_simple}' is being @Mock'd; mock dependencies, not the system under test.",
                                        line.strip()))

    # Collect mock field names for over-mock detection
    mock_names: set[str] = set()
    for idx, line in enumerate(lines, 1):
        # @Mock SomeType name;
        m = re.search(r"@(?:Mock|Spy|InjectMocks)\b[^;]*?\b(\w+)\s*;", line)
        if m:
            mock_names.add(m.group(1))
        else:
            # @Mock\n SomeType name;
            if re.match(r"\s*@(?:Mock|Spy|InjectMocks)\b\s*$", line) and idx < len(lines):
                m2 = re.search(r"\b(\w+)\s*;\s*$", lines[idx])
                if m2:
                    mock_names.add(m2.group(1))

    # Per-method checks
    has_any_test = False
    for start, end, name in _iter_test_methods(lines):
        has_any_test = True
        body = lines[start:end]
        body_text = "\n".join(body)

        # naming
        if not (
            re.match(r"^(should|test|given|when)[A-Z_]", name)
            or "_" in name
        ):
            findings.append(Finding(str(path), start + 1, "test-naming", "warn",
                                    f"Test method '{name}' does not follow should_xxx_when_yyy / testXxx / given_when_then naming.",
                                    lines[start].strip()))

        # disabled
        for k in range(max(0, start - 3), start + 1):
            if RE_IGNORED.match(lines[k]):
                findings.append(Finding(str(path), k + 1, "disabled-test", "warn",
                                        f"Test '{name}' is @Disabled; disabled tests do not contribute to coverage.",
                                        lines[k].strip()))
                break

        # assertion presence
        has_assert = bool(RE_ASSERT_ANY.search(body_text))
        if not has_assert:
            findings.append(Finding(str(path), start + 1, "no-assertion", "error",
                                    f"Test '{name}' contains no assertion or verification; it cannot fail meaningfully.",
                                    lines[start].strip()))

        # tautological assertions
        for off, line in enumerate(body):
            if RE_ASSERT_TRUE_LITERAL.search(line):
                findings.append(Finding(str(path), start + off + 1, "tautological-assert", "error",
                                        "assertTrue(true) / assertFalse(false) is meaningless.",
                                        line.strip()))

        # assertion of only nullness
        non_null_asserts = [
            ln for ln in body
            if RE_ASSERT_ANY.search(ln) and not (RE_ASSERT_NOT_NULL_BARE.search(ln) or RE_ASSERTJ_NOT_NULL.search(ln))
        ]
        if has_assert and not non_null_asserts:
            findings.append(Finding(str(path), start + 1, "only-not-null-assert", "error",
                                    f"Test '{name}' only asserts non-null/null; assert business behavior or returned values.",
                                    lines[start].strip()))

        # over-mocking: when(sut.x()) — mocking the SUT itself
        if sut_fqcn:
            sut_simple = sut_fqcn.rsplit(".", 1)[-1]
            for off, line in enumerate(body):
                m = RE_WHEN.search(line)
                if m and m.group(1) and m.group(1).lower() == sut_simple.lower()[:1] + sut_simple[1:]:
                    # very weak heuristic; keep low severity
                    findings.append(Finding(str(path), start + off + 1, "stub-on-sut", "warn",
                                            "Possible stubbing on SUT; verify you are not mocking the method under test.",
                                            line.strip()))

        # private reflection
        for off, line in enumerate(body):
            if RE_PRIVATE_REFLECTION.search(line):
                findings.append(Finding(str(path), start + off + 1, "tests-private-via-reflection", "warn",
                                        f"Test '{name}' reaches into private members via reflection; prefer testing through public API.",
                                        line.strip()))
            if RE_THREAD_SLEEP.search(line):
                findings.append(Finding(str(path), start + off + 1, "thread-sleep", "warn",
                                        "Thread.sleep in tests causes flakiness; prefer Awaitility or deterministic clocks.",
                                        line.strip()))
            if RE_CATCH_THROWABLE.search(line):
                findings.append(Finding(str(path), start + off + 1, "catch-and-pass", "warn",
                                        "Catch-all in test body usually hides failures; prefer assertThrows.",
                                        line.strip()))
            if RE_RANDOM_NEW.search(line):
                findings.append(Finding(str(path), start + off + 1, "nondeterministic-random", "warn",
                                        "new Random() without seed produces flaky tests; seed it or use a fixed value.",
                                        line.strip()))

    if not has_any_test:
        findings.append(Finding(str(path), 1, "no-tests", "warn",
                                "File contains no @Test / @ParameterizedTest methods.",
                                ""))

    return findings


def main() -> int:
    ap = argparse.ArgumentParser(description="Quality checker for JUnit5/Mockito/AssertJ test files.")
    ap.add_argument("files", nargs="+", help="Test .java files to inspect")
    ap.add_argument("--sut", default=None, help="FQCN of the system under test (enables SUT-mock checks)")
    args = ap.parse_args()

    all_findings: list[Finding] = []
    for f in args.files:
        p = Path(f)
        if not p.is_file():
            print(json.dumps({"error": f"file not found: {p}"}), file=sys.stderr)
            return 2
        all_findings.extend(check_file(p, args.sut))

    payload = {
        "files_checked": [str(Path(f)) for f in args.files],
        "sut": args.sut,
        "findings": [asdict(x) for x in all_findings],
        "summary": {
            "total": len(all_findings),
            "errors": sum(1 for x in all_findings if x.severity == "error"),
            "warnings": sum(1 for x in all_findings if x.severity == "warn"),
        },
    }
    json.dump(payload, sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0 if not any(x.severity == "error" for x in all_findings) else 1


if __name__ == "__main__":
    sys.exit(main())
