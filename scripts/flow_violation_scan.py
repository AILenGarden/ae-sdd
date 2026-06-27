#!/usr/bin/env python3
"""
flow_violation_scan.py — 流程违规审计工具

🆕 2026-06-27（来自 `2026-06-27-RA多轮挖掘流程未执行-自我修订建议书.md` §3.4）：
扫描 RA 文档是否符合 `requirement-analysis-skill.md` 完整 7 步流程。
解决"AI Agent 跳过 RA skill 完整流程直接出 RA 文档"的系统性漏洞（实测案例：
2026-06-27 AI Agent 直接读 PDF 出 36KB proposal，未走 RAGeneratePlan / 8 维度 / 5 问
自检 / 缺口管理 / 规模裁定 / RA-G01~G16 闸判定）。

8 条规则（与 `tools/lib/document_storage.py:check_ra_prerequisites` 互补）：
  R1: RA 文档必须包含 §0.5 RequirementAnalysisModel（12 维决策记录）
  R2: RA 文档必须包含 8 维度挖掘章节（§2~§9）
  R3: RA 文档必须包含 §10 缺口管理
  R4: RA 文档必须包含 §11 规模裁定
  R5: RA 文档必须含 RAGeneratePlan 字样（Plan-first 硬前置）
  R6: RA 文档必须显式列出 RA-G01~RA-G16 闸的判定结果
  R7: RA 文档必须包含 5 问自检记录（通过率 100%）
  R8: 缺口管理中 🔴/🟠 缺口必须有解决方案

与 check_ra_prerequisites 的关系：
  - check_ra_prerequisites = "落地前"前置（save_doc 调用，单一文档）
  - flow_violation_scan    = "落地后"审计（扫描一个目录下所有 RA，可批量）

调用入口：
  python scripts/flow_violation_scan.py --root <dir> [--format json|markdown] [--strict]
  ae-sdd gates check --only G-RA-FLOW-VIOLATION  （内部调本脚本）
  ae-sdd flow-violation-scan --root <dir>          （CLI 子命令）
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, asdict, field
from pathlib import Path


# ─── 8 条规则定义 ─────────────────────────────────────────────────────────────
# 每条 = (rule_id, description, check_function_or_pattern, severity)
# severity: BLOCKER = 阻断（违反即 🔴）/ WARN = 提示

_R1_PATTERN = "§0.5 RequirementAnalysisModel"
_R2_PATTERNS = ["§2 角色", "§3 场景", "§4 流程", "§5 数据",
                "§6 规则", "§7 设计方向", "§8 AC", "§9 假设"]
_R3_PATTERN = "§10 缺口"
_R4_PATTERN = "§11 规模"
_R5_PATTERN = re.compile(r"RAGeneratePlan", re.IGNORECASE)
_R6_PATTERN = re.compile(r"RA-G\d+", re.IGNORECASE)
_R7_PATTERN = re.compile(r"5\s*问自检|5-question|5question", re.IGNORECASE)
_R8_RE_PATTERNS = [
    (re.compile(r"🔴.*?(?:已解决|用户接受|已闭环)", re.IGNORECASE), "🔴 阻断型缺口已闭环"),
    (re.compile(r"🟠.*?(?:已解决|用户接受|已闭环)", re.IGNORECASE), "🟠 严重型缺口已闭环"),
]


@dataclass
class ViolationFinding:
    """单条违规。"""
    rule: str           # R1~R8
    severity: str       # BLOCKER / WARN
    path: str           # 相对 root 的路径
    line: int           # 行号（0 = 全文级违规）
    message: str        # 描述


@dataclass
class ScanStats:
    """扫描统计。"""
    raFiles: int = 0
    blockerViolations: int = 0
    warnViolations: int = 0
    rulesPassed: int = 0
    rulesFailed: int = 0


def _check_doc(content: str, rel_path: str) -> list[ViolationFinding]:
    """对单个 RA 文档跑 8 条规则，返回违规清单。"""
    findings: list[ViolationFinding] = []
    lines = content.splitlines()

    # R1：必须含 §0.5 RequirementAnalysisModel
    if _R1_PATTERN not in content:
        findings.append(ViolationFinding("R1", "BLOCKER", rel_path, 0,
            f"缺 {_R1_PATTERN}（RAModel 12 维决策记录，见 requirement-analysis-skill §第 0.5 步）"))

    # R2：必须含 8 维度章节
    missing_dims = [p for p in _R2_PATTERNS if p not in content]
    if missing_dims:
        findings.append(ViolationFinding("R2", "BLOCKER", rel_path, 0,
            f"缺 8 维度章节：{missing_dims}（见 requirement-analysis-skill §第一步）"))

    # R3：必须含 §10 缺口管理
    if _R3_PATTERN not in content:
        findings.append(ViolationFinding("R3", "BLOCKER", rel_path, 0,
            f"缺 {_R3_PATTERN}（缺口管理，见 requirement-analysis-skill §第二步）"))

    # R4：必须含 §11 规模裁定
    if _R4_PATTERN not in content:
        findings.append(ViolationFinding("R4", "BLOCKER", rel_path, 0,
            f"缺 {_R4_PATTERN}（规模裁定 + 路由决策，见 requirement-analysis-skill §第五步）"))

    # R5：必须含 RAGeneratePlan 字样
    if not _R5_PATTERN.search(content):
        findings.append(ViolationFinding("R5", "BLOCKER", rel_path, 0,
            "缺 RAGeneratePlan 字样（Plan-first 硬前置，见 requirement-analysis-skill §Plan-first）"))

    # R6：必须含 RA-G01~RA-G16 闸判定结果（至少 4 个）
    gate_marks = _R6_PATTERN.findall(content)
    if len(gate_marks) < 4:
        findings.append(ViolationFinding("R6", "BLOCKER", rel_path, 0,
            f"仅含 {len(gate_marks)} 个 RA-G 闸标记（要求 ≥4，见 requirement-analysis-skill §第七步 16 道闸）"))

    # R7：必须含 5 问自检记录
    if not _R7_PATTERN.search(content):
        findings.append(ViolationFinding("R7", "BLOCKER", rel_path, 0,
            "缺 5 问自检记录（见 requirement-analysis-skill §第一步 bis，通过率 100%）"))

    # R8：🔴/🟠 缺口必须有解决方案（INFO 级，不阻断 — 仅在 R3 已通过的文档检查）
    if _R3_PATTERN in content:
        # 提取缺口章节
        gap_section = _extract_section(content, _R3_PATTERN)
        if gap_section:
            for pattern, desc in _R8_RE_PATTERNS:
                if not pattern.search(gap_section):
                    # 找到 🔴/🟠 但没有"已解决/用户接受/已闭环"标记
                    if ("🔴" in gap_section and "已解决" not in gap_section
                            and "用户接受" not in gap_section and "已闭环" not in gap_section):
                        findings.append(ViolationFinding("R8", "WARN", rel_path, 0,
                            "缺口章节有 🔴 标记但未见闭环标记（已解决/用户接受/已闭环）"))

    return findings


def _extract_section(content: str, header: str) -> str:
    """从 content 提取 header 开头到下一个同级或更高级 header 之间的内容。"""
    lines = content.splitlines()
    start_idx = -1
    for i, line in enumerate(lines):
        if header in line:
            start_idx = i
            break
    if start_idx < 0:
        return ""
    # 找下一个 ## 级 header
    end_idx = len(lines)
    for j in range(start_idx + 1, len(lines)):
        if lines[j].startswith("## "):
            end_idx = j
            break
    return "\n".join(lines[start_idx:end_idx])


def scan_ra_docs(root: Path) -> tuple[list[ViolationFinding], int]:
    """扫描 root 下所有 *.md（含 RA 字样或 RA 目录下的），返回违规清单 + RA 文件数。"""
    findings: list[ViolationFinding] = []
    ra_files = 0

    # 候选 RA 文档：文件名含 RA- 或路径含 /RA/
    for md in root.rglob("*.md"):
        rel = str(md.relative_to(root)).replace("\\", "/")
        is_ra = ("/RA/" in rel) or re.search(r"\bRA[-_]", md.name, re.IGNORECASE)
        if not is_ra:
            continue
        ra_files += 1
        try:
            content = md.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        findings.extend(_check_doc(content, rel))

    return findings, ra_files


def render_markdown(root: Path, findings: list[ViolationFinding], stats: ScanStats) -> str:
    """渲染 markdown 报告。"""
    lines = [
        "# RA 流程违规审计报告",
        "",
        f"- 扫描根：`{root}`",
        f"- RA 文件数：{stats.raFiles}",
        f"- 阻断违规：{stats.blockerViolations}",
        f"- 警告违规：{stats.warnViolations}",
        f"- 规则通过：{stats.rulesPassed} / 8",
        f"- 规则失败：{stats.rulesFailed} / 8",
        "",
        "## 违规清单",
        "",
    ]
    if not findings:
        lines.append("✅ 无违规。所有 RA 文档均通过 8 条规则。")
    else:
        lines.append("| 规则 | 严重度 | 文件 | 行 | 描述 |")
        lines.append("|------|--------|------|----|------|")
        for f in findings:
            lines.append(f"| {f.rule} | {f.severity} | `{f.path}` | {f.line} | {f.message} |")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Scan RA documents for flow violations (8 rules from requirement-analysis-skill).")
    parser.add_argument("--root", default=".", help="Project root to scan for RA documents.")
    parser.add_argument("--output", help="Write the scan report to this file.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument("--strict", action="store_true",
                        help="Exit 1 if any BLOCKER violation found.")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    findings, ra_files = scan_ra_docs(root)

    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    rules_failed = len({f.rule for f in findings})
    stats = ScanStats(raFiles=ra_files, blockerViolations=blockers,
                      warnViolations=warnings, rulesPassed=8 - rules_failed,
                      rulesFailed=rules_failed)

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
        output = render_markdown(root, findings, stats)

    if args.output:
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)

    return 1 if (args.strict and blockers > 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
