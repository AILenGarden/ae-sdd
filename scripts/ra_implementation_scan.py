#!/usr/bin/env python3
"""Scan Requirement Analysis (RA) documents for implementation-view completeness.

v3.5.18 — G-RA-6 implementation feasibility gate.

The RA stage is the handoff source for DR generation. Flow checks can prove that
an RA walked the required process, and depth checks can prove that derivations
were not empty. This scanner covers the remaining implementation-facing
contract: can an engineer answer "where does the data come from, where does it
go, what definitions/invariants govern it, what can be reused, which expensive
designs were rejected, and what exactly must the DR carry forward?"

Seven rules (I1~I7):
  I1  Data source inventory exists and names concrete source types/evidence.
  I2  Data flow chain exists from source/ingress through processing to sink/output.
  I3  Terms, definitions, status/enums, field semantics, or invariants are recorded.
  I4  Existing implementation/reuse evidence is recorded with reuse/adapt/new calls.
  I5  High-cost or hard-to-implement designs are rebutted with cheaper alternatives.
  I6  Developer questions are answered with evidence/status and DR-blocking signal.
  I7  DR handoff package is present for interfaces, data model, state/transactions,
      nonfunctional constraints, tests, rollout, or migration.

Output matches other RA scanners: status/raFiles/findings[]. BLOCKER > 0 fails.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

from ra_scan_scope import (
    RAScanScopeError,
    ra_scan_scope_error_payload,
    resolve_ra_scan_scope,
)


RA_FILENAME_PATTERN = re.compile(r"RA[-_]", re.IGNORECASE)


@dataclass
class Finding:
    severity: str
    rule: str
    path: str
    line: int
    message: str
    snippet: str


def rel(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root)).replace("\\", "/")
    except ValueError:
        return str(path).replace("\\", "/")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def line_no(text: str, needle: str) -> int:
    idx = text.find(needle)
    if idx < 0:
        return 0
    return text.count("\n", 0, idx) + 1


def add_finding(
    findings: list[Finding],
    severity: str,
    rule: str,
    path: Path,
    root: Path,
    line: int,
    message: str,
    snippet: str = "",
) -> None:
    findings.append(Finding(
        severity=severity,
        rule=rule,
        path=rel(path, root),
        line=line,
        message=message,
        snippet=snippet.strip()[:220],
    ))


def find_section(content: str, title_pattern: str) -> tuple[int, str]:
    """Return (1-based header line, section text) for the first matching header."""
    header = re.compile(rf"^#{{1,6}}\s+.*(?:{title_pattern}).*$", re.IGNORECASE | re.MULTILINE)
    match = header.search(content)
    if not match:
        return 0, ""

    start = match.start()
    start_line = content.count("\n", 0, start) + 1
    lines = content[start:].splitlines()
    if not lines:
        return start_line, ""

    current_level_match = re.match(r"^(#{1,6})\s+", lines[0])
    current_level = len(current_level_match.group(1)) if current_level_match else 2
    end = len(lines)
    for idx, line in enumerate(lines[1:], start=1):
        m = re.match(r"^(#{1,%d})\s+" % current_level, line)
        if m:
            end = idx
            break
    return start_line, "\n".join(lines[:end])


def has_any(text: str, words: tuple[str, ...]) -> bool:
    return any(word.lower() in text.lower() for word in words)


def count_hits(text: str, words: tuple[str, ...]) -> int:
    lowered = text.lower()
    return sum(1 for word in words if word.lower() in lowered)


def non_placeholder_text(section: str) -> str:
    lines = []
    for line in section.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if re.fullmatch(r"[-|:\s]+", stripped):
            continue
        lines.append(stripped)
    return "\n".join(lines)


def is_empty_or_placeholder(section: str) -> bool:
    body = non_placeholder_text(section)
    if len(body) < 80:
        return True
    blocker_tokens = ("TODO", "TBD", "待补充", "占位", "placeholder")
    if any(tok in body for tok in blocker_tokens):
        return True
    return bool(re.search(r"\{\s*(?:x|X|\.\.\.|待补充|TODO|TBD)\s*\}", body))


def check_required_section(
    findings: list[Finding],
    *,
    rule: str,
    path: Path,
    root: Path,
    content: str,
    title_pattern: str,
    missing_message: str,
    validate,
) -> None:
    header_line, section = find_section(content, title_pattern)
    if not section:
        add_finding(findings, "BLOCKER", rule, path, root, 0, missing_message)
        return
    if is_empty_or_placeholder(section):
        add_finding(findings, "BLOCKER", rule, path, root, header_line,
                    "实现视角章节为空、占位或信息量不足，无法支撑 DR。", section)
        return
    validate(section, header_line)

    if "待确认" in section:
        add_finding(findings, "WARN", rule, path, root, header_line,
                    "章节含“待确认”，需在 DR 前确认是否为阻断问题。", section)


def check_doc(path: Path, root: Path, content: str) -> list[Finding]:
    findings: list[Finding] = []

    def i1(section: str, header_line: int) -> None:
        source_types = ("DB", "数据库", "表", "view", "API", "接口", "MQ", "事件", "缓存",
                        "Redis", "配置", "Nacos", "文件", "OSS", "三方", "第三方",
                        "前端", "客户端", "日志", "审计", "指标")
        evidence_terms = ("来源", "证据", "owner", "Owner", "权威源", "读", "写", "字段")
        if count_hits(section, source_types) < 3 or not has_any(section, evidence_terms):
            add_finding(findings, "BLOCKER", "I1", path, root, header_line,
                        "数据源清单必须列出至少 3 类来源并带来源/证据/读写/owner/权威源信息。", section)

    def i2(section: str, header_line: int) -> None:
        flow_terms = ("来源", "入口", "API", "事件", "处理", "服务", "领域", "落点",
                      "持久化", "缓存", "MQ", "输出", "响应", "观测", "一致性", "事务")
        has_flow_shape = "->" in section or "→" in section or "|" in section
        if count_hits(section, flow_terms) < 5 or not has_flow_shape:
            add_finding(findings, "BLOCKER", "I2", path, root, header_line,
                        "数据流链路必须描述 source→ingress→domain/service→persistence/cache/MQ→output/observability，并体现事务或一致性。", section)

    def i3(section: str, header_line: int) -> None:
        definition_terms = ("术语", "定义", "字段", "枚举", "状态", "状态机", "不变量",
                            "单位", "空值", "null", "ID", "权威源", "唯一性")
        if count_hits(section, definition_terms) < 4:
            add_finding(findings, "BLOCKER", "I3", path, root, header_line,
                        "术语/定义章节必须覆盖术语、字段/状态枚举、不变量、单位/空值/ID/权威源等实现语义。", section)

    def i4(section: str, header_line: int) -> None:
        evidence_terms = ("代码", "路径", "class", "method", "接口", "表", "assets",
                          "git", "复用", "改造", "新建", "现有")
        reuse_terms = ("复用", "改造", "新建", "不复用")
        if count_hits(section, evidence_terms) < 4 or not has_any(section, reuse_terms):
            add_finding(findings, "BLOCKER", "I4", path, root, header_line,
                        "现有实现/复用证据必须列出代码/表/API/assets/git 证据，并明确复用、改造、新建或不复用结论。", section)

    def i5(section: str, header_line: int) -> None:
        reject_terms = ("高成本", "难实现", "不采用", "拒绝", "反驳", "成本", "风险")
        alternative_terms = ("替代", "更低成本", "简化", "分阶段", "保留", "建议")
        if not has_any(section, reject_terms) or not has_any(section, alternative_terms):
            add_finding(findings, "BLOCKER", "I5", path, root, header_line,
                        "必须列出高成本/难实现设计的反驳、不采用理由，并给出低成本替代方案。", section)

    def i6(section: str, header_line: int) -> None:
        question_terms = ("疑问", "问题", "开发者", "答案", "答复", "证据", "状态",
                          "阻断", "DR", "已解决")
        if count_hits(section, question_terms) < 5 or "?" not in section and "？" not in section:
            add_finding(findings, "BLOCKER", "I6", path, root, header_line,
                        "开发者疑问矩阵必须包含具体问题、答案/证据、状态以及是否阻断 DR。", section)

    def i7(section: str, header_line: int) -> None:
        handoff_terms = ("接口", "API", "数据模型", "表", "状态", "事务", "一致性",
                         "非功能", "性能", "权限", "测试", "验收", "迁移", "回滚",
                         "灰度", "DR")
        if count_hits(section, handoff_terms) < 7:
            add_finding(findings, "BLOCKER", "I7", path, root, header_line,
                        "DR 交接包必须覆盖接口、数据模型、状态/事务、非功能、测试验收、迁移/回滚/灰度等下游输入。", section)

    check_required_section(
        findings, rule="I1", path=path, root=root, content=content,
        title_pattern=r"数据源(?:清单|盘点|inventory)?",
        missing_message="缺数据源清单：RA 必须挖出 DB/API/MQ/cache/config/file/third-party/frontend/log 等来源。",
        validate=i1,
    )
    check_required_section(
        findings, rule="I2", path=path, root=root, content=content,
        title_pattern=r"数据流(?:链路|流程|图|逻辑|flow)",
        missing_message="缺数据流链路：RA 必须说明数据从哪里来、如何处理、写到哪里、对外输出什么。",
        validate=i2,
    )
    check_required_section(
        findings, rule="I3", path=path, root=root, content=content,
        title_pattern=r"(术语|定义|不变量)",
        missing_message="缺术语/定义/不变量：RA 必须消除字段、状态、枚举、单位、ID、权威源歧义。",
        validate=i3,
    )
    check_required_section(
        findings, rule="I4", path=path, root=root, content=content,
        title_pattern=r"(现有实现|复用证据|实现复用)",
        missing_message="缺现有实现/复用证据：RA 必须说明已有代码、表、接口、组件是否复用/改造/新建。",
        validate=i4,
    )
    check_required_section(
        findings, rule="I5", path=path, root=root, content=content,
        title_pattern=r"(高成本|难实现|反驳|拒绝方案)",
        missing_message="缺高成本/难实现设计反驳：RA 必须主动否决不经济或难落地方案并给替代方案。",
        validate=i5,
    )
    check_required_section(
        findings, rule="I6", path=path, root=root, content=content,
        title_pattern=r"(开发者疑问|开发疑问|工程疑问)",
        missing_message="缺开发者疑问答复矩阵：RA 必须回答开发者写 DR 前会问的问题。",
        validate=i6,
    )
    check_required_section(
        findings, rule="I7", path=path, root=root, content=content,
        title_pattern=r"(DR\s*交接|DR\s*生成|设计交接|交接包)",
        missing_message="缺 DR 生成交接包：RA 必须给 DR 可直接使用的接口、模型、状态、事务、测试与发布输入。",
        validate=i7,
    )

    return findings


def scan_ra_docs(root: Path, files: tuple[Path, ...] | None = None) -> tuple[list[Finding], int]:
    findings: list[Finding] = []
    ra_files = 0

    if files is None:
        files = resolve_ra_scan_scope(root).files

    for md in files:
        ra_files += 1
        try:
            content = read_text(md)
        except OSError as exc:
            add_finding(findings, "WARN", "IO", md, root, 0, f"无法读取 RA 文档：{exc}")
            continue
        findings.extend(check_doc(md, root, content))

    return findings, ra_files


def render_markdown(root: Path, findings: list[Finding], ra_files: int) -> str:
    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    lines = [
        "# RA 实现视角完整性扫描报告",
        "",
        f"- 扫描根：`{root}`",
        f"- RA 文件数：{ra_files}",
        f"- 阻断项：{blockers}",
        f"- 警告项：{warnings}",
        "",
        "## Findings",
        "",
    ]
    if not findings:
        lines.append("✅ 无违规。RA 实现视角七要素完整。")
    else:
        lines.append("| 规则 | 严重度 | 文件 | 行 | 描述 |")
        lines.append("|------|--------|------|----|------|")
        for f in findings:
            lines.append(f"| {f.rule} | {f.severity} | `{f.path}` | {f.line} | {f.message} |")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Scan RA documents for implementation-view completeness (G-RA-6).")
    parser.add_argument("--root", default=".", help="Project root to scan for RA documents.")
    parser.add_argument(
        "--file",
        action="append",
        default=[],
        help="Scan only this authoritative RA Markdown file (repeatable).",
    )
    parser.add_argument("--output", help="Write the scan report to this file.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument("--strict", action="store_true",
                        help="Exit 1 if any BLOCKER finding exists.")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    try:
        scope = resolve_ra_scan_scope(root, args.file)
    except RAScanScopeError as exc:
        if args.format == "json":
            sys.stdout.write(json.dumps(
                ra_scan_scope_error_payload(exc, root, args.file),
                ensure_ascii=False,
                indent=2,
            ))
            return 2
        parser.error(str(exc))
    findings, ra_files = scan_ra_docs(root, scope.files)
    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")

    if args.format == "json":
        payload = {
            "root": str(root),
            "scopeMode": scope.mode,
            "selectedFiles": scope.selected_files,
            "excludedFiles": scope.excluded_files,
            "status": "PASS" if blockers == 0 else "FAIL",
            "raFiles": ra_files,
            "ruleStats": {
                "rulesTotal": 7,
                "rulesFailed": len({f.rule for f in findings if f.severity == "BLOCKER"}),
                "rulesWarned": len({f.rule for f in findings if f.severity == "WARN"}),
            },
            "blockers": blockers,
            "warnings": warnings,
            "findings": [asdict(f) for f in findings],
        }
        output = json.dumps(payload, ensure_ascii=False, indent=2)
    else:
        output = render_markdown(root, findings, ra_files)

    if args.output:
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)

    return 1 if (args.strict and blockers > 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
