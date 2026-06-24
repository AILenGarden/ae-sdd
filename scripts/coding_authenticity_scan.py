#!/usr/bin/env python3
"""Scan production code and Coding reports for LLM Coding anti-patterns.

Coding 真实性扫描器 — 对标 test_authenticity_scan.py / ra_authenticity_scan.py。

目标不是替代编译、单元测试或 CodeReview，而是把 LLM 高频 Coding 反模式中
可静态命中的部分先变成硬信号：

- AP-1 幻觉 API：命中特定已知幻觉参数/过时 API。
- AP-2 抄旧代码：命中过时安全配置等版本错位信号。
- AP-3 过度设计：抽象命名过密时给 WARN，要求 CodeReview 解释。
- AP-4 注释撒谎：TODO/FIXME/XXX 留在生产代码中给 WARN。
- AP-5 默认值陷阱：硬编码 URL/密钥/timeout/retry/TTL 给 BLOCKER。
- AP-6 上下文漂移：Coding 报告引用不存在的代码文件给 BLOCKER。

无第三方依赖，可在本地 agent 会话与 CI 中运行。BLOCKER 发现使当前 Coding
结果无效，直到修复或显式评审通过。
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable


CODE_EXTENSIONS = (".java", ".kt", ".kts", ".xml", ".yaml", ".yml", ".properties")
TEXT_CODE_EXTENSIONS = CODE_EXTENSIONS + (".py", ".js", ".ts")
EXCLUDED_DIRS = {
    ".git",
    ".idea",
    ".gradle",
    ".ae-sdd",
    ".auto-engineering",
    "ae-sdd-doc",
    "node_modules",
    "target",
    "build",
    "dist",
    "__pycache__",
}


@dataclass
class Finding:
    severity: str
    rule: str
    path: str
    line: int
    message: str
    snippet: str


@dataclass
class CodeStats:
    codeFiles: int = 0
    codingReports: int = 0
    blockerFindings: int = 0
    warnFindings: int = 0


LINE_RULES = [
    (
        "BLOCKER",
        "hallucinated-transactional-event-fallback",
        re.compile(r"@TransactionalEventListener\s*\([^)]*\bfallback\s*=", re.DOTALL),
        "AP-1 幻觉 API：@TransactionalEventListener 没有 fallback 参数，请核对真实框架版本。",
    ),
    (
        "BLOCKER",
        "legacy-web-security-configurer-adapter",
        re.compile(r"\bWebSecurityConfigurerAdapter\b"),
        "AP-2 抄旧代码：WebSecurityConfigurerAdapter 是 Spring Security 旧式写法，请确认项目版本。",
    ),
    (
        "BLOCKER",
        "hardcoded-secret",
        re.compile(r"(?i)\b(password|passwd|secret|token|api[_-]?key)\b\s*[:=]\s*[\"'][^\"'${}]{4,}[\"']"),
        "AP-5 默认值陷阱：疑似硬编码密钥/口令/Token，必须走配置中心或环境变量。",
    ),
    (
        "BLOCKER",
        "hardcoded-external-url",
        re.compile(r"[\"']https?://(?!(?:localhost|127\.0\.0\.1|\$\{))[^\"']+[\"']"),
        "AP-5 默认值陷阱：生产代码中出现硬编码外部 URL，必须走配置项并说明来源。",
    ),
    (
        "BLOCKER",
        "hardcoded-timeout-retry-ttl",
        re.compile(
            r"(?i)(?:\b(?:timeout|retryCount|maxRetries|ttl|expireSeconds|delayMillis|sleepMillis)\b\s*=\s*\d+"
            r"|\.set(?:Connect|Read|Write)?Timeout\s*\(\s*\d+"
            r"|Duration\.of(?:Millis|Seconds|Minutes)\s*\(\s*\d+\s*\))"
        ),
        "AP-5 默认值陷阱：timeout/retry/TTL/delay 等可变参数不能硬编码，需来自配置或明确常量来源。",
    ),
    (
        "BLOCKER",
        "thread-sleep-production",
        re.compile(r"\bThread\.sleep\s*\(|\bTimeUnit\.[A-Z_]+\.sleep\s*\("),
        "生产代码禁止 Thread.sleep/TimeUnit.sleep 兜底等待，应使用确定性同步、队列或调度机制。",
    ),
    (
        "WARN",
        "todo-fixme-production",
        re.compile(r"\b(TODO|FIXME|XXX)\b"),
        "AP-4 注释撒谎风险：生产代码残留 TODO/FIXME/XXX，必须说明是否进入后续任务。",
    ),
    (
        "WARN",
        "suppress-warning-without-reason",
        re.compile(r"@SuppressWarnings\s*\("),
        "AP-4 注释/约束漂移风险：@SuppressWarnings 需要在邻近注释中说明原因。",
    ),
]

EMPTY_OR_RETURNING_CATCH = re.compile(
    r"catch\s*\([^)]*\)\s*\{\s*(?:(?://[^\n]*\n\s*)|(?:/\*.*?\*/\s*))*"
    r"(?:(return\s*;|return\s+null\s*;)\s*)?\}",
    re.DOTALL,
)

OVER_ABSTRACTION_NAME = re.compile(
    r"\b(?:class|interface)\s+([A-Za-z_][\w]*(?:Strategy|Factory|Builder|Template|Abstract|Base)[A-Za-z_]\w*)"
)

BACKTICK_CODE_REF = re.compile(
    r"`([^`\n]+\.(?:java|kt|kts|xml|yaml|yml|properties|py|js|ts))`"
)
PLAIN_CODE_REF = re.compile(
    r"(?<![`A-Za-z0-9_.-])((?:src|app|lib|tools|scripts)/[^\s`|)]+?\."
    r"(?:java|kt|kts|xml|yaml|yml|properties|py|js|ts))"
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


def _is_excluded(path: Path) -> bool:
    return any(part in EXCLUDED_DIRS for part in path.parts)


def _is_test_path(path: Path) -> bool:
    norm = path.as_posix().lower()
    return "/src/test/" in norm or "/test/" in norm or path.name.lower().endswith("test.java")


def iter_code_files(root: Path) -> Iterable[Path]:
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if _is_excluded(path):
            continue
        if path.suffix not in CODE_EXTENSIONS:
            continue
        if _is_test_path(path):
            continue
        # 默认聚焦生产代码/资源；旧项目若没有 src/main，也允许扫描根下代码文件。
        yield path


def iter_coding_reports(root: Path) -> Iterable[Path]:
    for path in root.rglob("*.md"):
        if _is_excluded(path):
            continue
        parts_lower = {part.lower() for part in path.parts}
        if "source" in parts_lower or "plugins" in parts_lower:
            continue
        lower = path.name.lower()
        lower_full = path.as_posix().lower()
        if any(seg in lower_full for seg in ("template", "changelog", "change_log")):
            continue
        is_instance_report = (
            "ae-sdd-doc" in parts_lower
            or "design" in parts_lower
            or ".auto-engineering" in parts_lower
        )
        if is_instance_report and "coding" in lower and ("report" in lower or "报告" in lower):
            yield path


def scan_code_file(path: Path, root: Path, findings: list[Finding]) -> None:
    text = read_text(path)
    lines = text.splitlines()

    for idx, source_line in enumerate(lines, start=1):
        for severity, rule, pattern, message in LINE_RULES:
            if pattern.search(source_line):
                add_finding(findings, severity, rule, path, root, idx, message, source_line)

        if OVER_ABSTRACTION_NAME.search(source_line):
            add_finding(
                findings,
                "WARN",
                "over-abstraction-name",
                path,
                root,
                idx,
                "AP-3 过度设计风险：Strategy/Factory/Builder/Template/Abstract/Base 命名需要说明调用方与扩展理由。",
                source_line,
            )

    for match in EMPTY_OR_RETURNING_CATCH.finditer(text):
        add_finding(
            findings,
            "BLOCKER",
            "empty-or-returning-catch-production",
            path,
            root,
            line_no(text, match.start()),
            "生产代码 catch 块吞异常或直接返回，会掩盖失败路径；必须记录、转换或补偿。",
            match.group(0),
        )


def _normalize_ref(raw: str) -> str:
    ref = raw.strip().strip(".,;:")
    return ref.replace("\\", "/")


def scan_coding_report(path: Path, root: Path, findings: list[Finding]) -> None:
    text = read_text(path)
    refs = {_normalize_ref(m.group(1)) for m in BACKTICK_CODE_REF.finditer(text)}
    refs.update(_normalize_ref(m.group(1)) for m in PLAIN_CODE_REF.finditer(text))

    if not refs:
        add_finding(
            findings,
            "WARN",
            "coding-report-without-code-evidence",
            path,
            root,
            1,
            "AP-6 上下文漂移风险：Coding 报告没有引用任何具体代码文件，报告-代码对账不可执行。",
            path.name,
        )
        return

    for ref in sorted(refs):
        # 只核对项目内相对路径；外链/绝对路径不由本扫描器判定。
        if re.match(r"^[A-Za-z]:/", ref) or ref.startswith("/"):
            continue
        if not (root / ref).is_file():
            add_finding(
                findings,
                "BLOCKER",
                "coding-report-missing-code-file",
                path,
                root,
                line_no(text, text.find(ref)),
                f"AP-6 上下文漂移：Coding 报告引用的代码文件不存在：{ref}",
                ref,
            )


def scan(root: Path) -> tuple[list[Finding], CodeStats]:
    findings: list[Finding] = []
    stats = CodeStats()

    for path in iter_code_files(root):
        stats.codeFiles += 1
        scan_code_file(path, root, findings)

    for path in iter_coding_reports(root):
        stats.codingReports += 1
        scan_coding_report(path, root, findings)

    findings.sort(key=lambda f: (0 if f.severity == "BLOCKER" else 1, f.path, f.line, f.rule))
    stats.blockerFindings = sum(1 for f in findings if f.severity == "BLOCKER")
    stats.warnFindings = sum(1 for f in findings if f.severity == "WARN")
    return findings, stats


def render_markdown(root: Path, findings: list[Finding], stats: CodeStats) -> str:
    status = "PASS" if stats.blockerFindings == 0 else "FAIL"
    lines = [
        "# Coding Authenticity Scan Report",
        "",
        "## Summary",
        "",
        "| Item | Value |",
        "|---|---|",
        f"| Root | `{root}` |",
        f"| Status | `{status}` |",
        f"| Production code files scanned | {stats.codeFiles} |",
        f"| Coding reports scanned | {stats.codingReports} |",
        f"| BLOCKER findings | {stats.blockerFindings} |",
        f"| WARN findings | {stats.warnFindings} |",
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
    lines.append("BLOCKER findings make the current Coding result invalid until fixed or explicitly reviewed.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Scan production code for LLM Coding anti-patterns.")
    parser.add_argument("--root", default=".", help="Repository or module root to scan.")
    parser.add_argument("--output", help="Write the scan report to this file.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    findings, stats = scan(root)

    if args.format == "json":
        payload = {
            "root": str(root),
            "status": "PASS" if stats.blockerFindings == 0 else "FAIL",
            "codeFiles": stats.codeFiles,
            "codingReports": stats.codingReports,
            "reportStats": asdict(stats),
            "findings": [asdict(f) for f in findings],
        }
        output = json.dumps(payload, ensure_ascii=False, indent=2)
    else:
        output = render_markdown(root, findings, stats)

    if args.output:
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)

    return 1 if stats.blockerFindings > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
