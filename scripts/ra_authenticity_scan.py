#!/usr/bin/env python3
"""Scan Requirement Analysis (RA) documents for fabrication / vagueness risk patterns.

RA 真实性扫描器 — 对标 test_authenticity_scan.py（测试真实性扫描）。

设计哲学：Coding 阶段用 test_authenticity_scan.py 防"假测试"；
需求分析阶段同样需要防"假需求"——无证据结论、省略性措辞、凭空字段、
掩盖缺口、占位填充、"无衍生"偷懒、时效模糊。本扫描器把 requirement-analysis-skill
的标尺 1（穷举优于抽样）/标尺 2（证据优于假设）/标尺 3（冲突显性化）/标尺 4
（缺口不掩盖）+ E.5（衍生规则强制）/G.5（衍生 AC 时效）从"软规则"变成"可执行检查"。

无第三方依赖，可在本地 agent 会话与 CI 中运行。BLOCKER 发现使当前 RA 文档无效，
直到修复或显式评审通过。
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


# RA 文档命名约定（与 document-storage-skill / requirement-analysis-skill 一致）：
#   ae-sdd-doc/iterations/{date}/RA/RA-{ID}-v{major}.{minor}.md
#   design/RA-*.md （旧路径兼容）
RA_FILENAME_PATTERN = re.compile(r"RA[-_]", re.IGNORECASE)


@dataclass
class Finding:
    severity: str
    rule: str
    path: str
    line: int
    message: str
    snippet: str


@dataclass
class RAStats:
    raFiles: int = 0
    blockerFindings: int = 0
    warnFindings: int = 0


# ─── 8 类禁止规则（对标 test_authenticity_scan.py 的 LINE_RULES）──────────────
# 源自 requirement-analysis-skill §总则 4 标尺 + §阶段 E.5 / §阶段 G.5。
# 每条：(severity, rule, pattern, message)
RA_LINE_RULES = [
    # 标尺 1：穷举优于抽样 — 禁止省略性措辞
    (
        "BLOCKER",
        "vague-ellipsis",
        re.compile(r"(等等|其他(?!角色|服务|模块|表)|大概|之类的|诸如此类|等等等等)"),
        "省略性措辞违反标尺1（穷举优于抽样）：任何'覆盖'类结论必须先有穷举清单，再逐项打钩。",
    ),
    # 输出核心原则：禁止占位填充
    (
        "WARN",
        "placeholder-fill",
        re.compile(r"(待补充|待确认|TODO|TBD|XXX|FIXME|\{待确认\}|\{xxx\})"),
        "占位符/待补充违反输出核心原则：不确定信息必须标注来源缺失，禁止用占位内容跳过。",
    ),
    # 标尺 4：缺口不掩盖 — "已解决/已确认"但无证据链
    (
        "BLOCKER",
        "masked-gap",
        re.compile(r"(已解决|已确认|无缺口|无需处理)"),
        "标尺4（缺口不掩盖）：'已解决/已确认'类结论必须可追溯到证据，禁止掩盖未决问题。请检查上下文是否有对应证据。",
    ),
    # 标尺 3：冲突显性化 — "无冲突"结论但未列冲突清单
    (
        "WARN",
        "hidden-conflict",
        re.compile(r"无冲突|没有冲突|不存在冲突"),
        "标尺3（冲突显性化）：'无冲突'结论必须建立在穷举冲突清单之上，禁止直接断言。",
    ),
    # §阶段 G.5.2：衍生 AC 时效必须具体秒数，禁止"尽快/及时/立即"
    (
        "BLOCKER",
        "missing-timeliness",
        re.compile(r"(尽快|及时|立即|马上|实时|迅速)"),
        "§阶段 G.5.2：衍生 AC 时效要求必须具体（如'5 秒内'），禁止'尽快/及时/立即'等模糊表述。",
    ),
]

# §阶段 E.5.1：禁止"无衍生规则"作为结论而不附 H.5 模式全检声明
NO_DERIVATIVE_PATTERN = re.compile(
    r"(无衍生规则|无衍生影响|无衍生|不涉及衍生|无级联|无跨域影响)"
)
H5_FULL_CHECK_PATTERN = re.compile(
    r"(H\.5 模式|H\.5\.1|经 H\.5|模式全检|全检后确认|不适用.{0,20}理由)",
    re.IGNORECASE,
)

# 标尺 2：证据优于假设 — 结论行应可 cite 到 PRD/Issue/对话/资产。
# 启发式：以"- "或"| "开头的"结论性"行，若含判断动词但无任何 cite 标记，则 WARN。
EVIDENCE_CITE_PATTERN = re.compile(
    r"(PRD\s*§|Issue\s*#|用户对话|assets\.(table|module|component|api|search|sections|outline)|"
    r"代码反查|项目资产|行号|§\d|cite|来源[:：])",
    re.IGNORECASE,
)
CONCLUSION_VERB_PATTERN = re.compile(r"(需要|必须|应该|会|将|应当|导致|触发|影响|要求|禁止)")


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


def iter_ra_docs(root: Path) -> Iterable[Path]:
    """枚举 RA 文档。

    命名约定（与 requirement-analysis-skill §文档存放前置调用一致）：
      - ae-sdd-doc/iterations/*/RA/RA-*.md
      - design/RA-*.md （旧路径兼容）
    匹配文件名含 "RA-" / "RA_"（不区分大小写）的 .md。
    """
    for path in root.rglob("*.md"):
        if not RA_FILENAME_PATTERN.search(path.name):
            continue
        # 排除 changelog / template 等非实例文档
        lower = path.as_posix().lower()
        if any(seg in lower for seg in ("changelog", "template", "ra-template", "change_log")):
            continue
        yield path


def scan_ra_doc(path: Path, root: Path, findings: list[Finding]) -> None:
    """扫描单个 RA 文档。"""
    text = read_text(path)
    lines = text.splitlines()

    for idx, source_line in enumerate(lines, start=1):
        stripped = source_line.lstrip()
        # 1. 通用行规则
        for severity, rule, pattern, message in RA_LINE_RULES:
            if pattern.search(source_line):
                add_finding(findings, severity, rule, path, root, idx, message, source_line)

        # 2. 标尺2 证据优于假设：结论性行无 cite（仅 WARN，避免误伤）
        if (stripped.startswith("- ") or stripped.startswith("| ")) and CONCLUSION_VERB_PATTERN.search(source_line):
            if not EVIDENCE_CITE_PATTERN.search(source_line):
                # 跳过纯模板占位行（含 {xxx}）
                if "{xxx}" not in source_line and "{...}" not in source_line:
                    add_finding(
                        findings,
                        "WARN",
                        "no-evidence",
                        path,
                        root,
                        idx,
                        "标尺2（证据优于假设）：结论行无可 cite 的来源（PRD §/Issue #/assets./用户对话），请补充证据。",
                        source_line,
                    )

    # 3. §阶段 E.5.1：禁止"无衍生规则"结论而不附 H.5 全检声明
    for match in NO_DERIVATIVE_PATTERN.finditer(text):
        # 查看该结论前后 200 字符内是否有 H.5 全检声明
        window_start = max(0, match.start() - 200)
        window_end = min(len(text), match.end() + 200)
        window = text[window_start:window_end]
        if not H5_FULL_CHECK_PATTERN.search(window):
            add_finding(
                findings,
                "BLOCKER",
                "assumed-no-derivative",
                path,
                root,
                line_no(text, match.start()),
                "§阶段 E.5.1：'无衍生规则/无级联'结论必须附 H.5 模式全检声明，禁止直接断言无衍生。",
                match.group(0),
            )


def scan_ra_docs(root: Path) -> tuple[list[Finding], int]:
    findings: list[Finding] = []
    ra_files = 0
    for path in iter_ra_docs(root):
        ra_files += 1
        scan_ra_doc(path, root, findings)
    return findings, ra_files


def render_markdown(root: Path, findings: list[Finding], ra_files: int) -> str:
    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    status = "PASS" if blockers == 0 else "FAIL"
    lines = [
        "# RA Authenticity Scan Report",
        "",
        "## Summary",
        "",
        "| Item | Value |",
        "|---|---|",
        f"| Root | `{root}` |",
        f"| Status | `{status}` |",
        f"| RA files scanned | {ra_files} |",
        f"| BLOCKER findings | {blockers} |",
        f"| WARN findings | {warnings} |",
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
    lines.append("BLOCKER findings make the current RA document invalid until fixed or explicitly reviewed.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Scan RA documents for fabrication / vagueness risk patterns.")
    parser.add_argument("--root", default=".", help="Project root to scan for RA documents.")
    parser.add_argument("--output", help="Write the scan report to this file.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    findings, ra_files = scan_ra_docs(root)
    findings.sort(key=lambda f: (0 if f.severity == "BLOCKER" else 1, f.path, f.line, f.rule))

    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    stats = RAStats(raFiles=ra_files, blockerFindings=blockers, warnFindings=warnings)

    if args.format == "json":
        payload = {
            "root": str(root),
            "status": "PASS" if blockers == 0 else "FAIL",
            "raFiles": ra_files,
            "reportStats": asdict(stats),
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

    return 1 if blockers > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
