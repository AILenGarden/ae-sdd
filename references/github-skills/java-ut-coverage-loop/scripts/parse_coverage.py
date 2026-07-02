#!/usr/bin/env python3
"""Parse JaCoCo XML and report coverage status as JSON.

Usage:
    parse_coverage.py <jacoco.xml> [--line 80] [--branch 70] [--class FQCN]

Output: JSON to stdout with per-class line/branch coverage, uncovered line
numbers, partially-covered branch lines, and an overall pass/fail summary
against the thresholds. Exits 0 if all targeted classes pass thresholds, 1
otherwise. Exits 2 on input errors.
"""
from __future__ import annotations

import argparse
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def _counter(el: ET.Element, ctype: str) -> tuple[int, int]:
    """Return (missed, covered) for a counter type, or (0, 0) if absent."""
    for c in el.findall("counter"):
        if c.get("type") == ctype:
            return int(c.get("missed", "0")), int(c.get("covered", "0"))
    return 0, 0


def _pct(missed: int, covered: int) -> float:
    total = missed + covered
    return 100.0 if total == 0 else round(100.0 * covered / total, 2)


def parse(xml_path: Path, line_th: float, branch_th: float, only_fqcn: str | None):
    tree = ET.parse(xml_path)
    root = tree.getroot()

    classes: list[dict] = []
    for pkg in root.findall("package"):
        pkg_path = pkg.get("name", "")
        for sf in pkg.findall("sourcefile"):
            sf_name = sf.get("name", "")
            base = sf_name[:-5] if sf_name.endswith(".java") else sf_name
            fqcn = f"{pkg_path}/{base}".replace("/", ".") if pkg_path else base

            if only_fqcn and only_fqcn != fqcn:
                continue

            line_missed, line_covered = _counter(sf, "LINE")
            branch_missed, branch_covered = _counter(sf, "BRANCH")
            instr_missed, instr_covered = _counter(sf, "INSTRUCTION")

            uncovered_lines: list[int] = []
            partial_branches: list[dict] = []
            for ln in sf.findall("line"):
                nr = int(ln.get("nr", "0"))
                mi = int(ln.get("mi", "0"))
                ci = int(ln.get("ci", "0"))
                mb = int(ln.get("mb", "0"))
                cb = int(ln.get("cb", "0"))
                if mi > 0 and ci == 0:
                    uncovered_lines.append(nr)
                if mb > 0:
                    partial_branches.append({"line": nr, "missed": mb, "covered": cb})

            line_pct = _pct(line_missed, line_covered)
            branch_pct = _pct(branch_missed, branch_covered)
            line_ok = line_pct >= line_th
            branch_ok = branch_pct >= branch_th

            classes.append({
                "fqcn": fqcn,
                "source_file": sf_name,
                "line_coverage_pct": line_pct,
                "branch_coverage_pct": branch_pct,
                "lines_total": line_missed + line_covered,
                "lines_missed": line_missed,
                "branches_total": branch_missed + branch_covered,
                "branches_missed": branch_missed,
                "instructions_total": instr_missed + instr_covered,
                "instructions_missed": instr_missed,
                "uncovered_lines": uncovered_lines,
                "partial_branches": partial_branches,
                "line_threshold": line_th,
                "branch_threshold": branch_th,
                "line_passes": line_ok,
                "branch_passes": branch_ok,
                "passes": line_ok and branch_ok,
            })

    blockers = []
    for c in classes:
        if not c["passes"]:
            reasons = []
            if not c["line_passes"]:
                reasons.append(f"line {c['line_coverage_pct']}% < {line_th}%")
            if not c["branch_passes"]:
                reasons.append(f"branch {c['branch_coverage_pct']}% < {branch_th}%")
            blockers.append({"fqcn": c["fqcn"], "reasons": reasons})

    return {
        "thresholds": {"line": line_th, "branch": branch_th},
        "classes": classes,
        "summary": {
            "class_count": len(classes),
            "passing": sum(1 for c in classes if c["passes"]),
            "all_pass": bool(classes) and all(c["passes"] for c in classes),
            "blockers": blockers,
        },
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Parse JaCoCo XML coverage report.")
    ap.add_argument("xml", help="Path to jacoco.xml")
    ap.add_argument("--line", type=float, default=80.0, help="Line coverage threshold (default 80)")
    ap.add_argument("--branch", type=float, default=70.0, help="Branch coverage threshold (default 70)")
    ap.add_argument("--class", dest="fqcn", default=None, help="Filter to a single FQCN (e.g. com.example.Foo)")
    args = ap.parse_args()

    xml_path = Path(args.xml)
    if not xml_path.is_file():
        print(json.dumps({"error": f"jacoco xml not found: {xml_path}"}), file=sys.stderr)
        return 2

    try:
        result = parse(xml_path, args.line, args.branch, args.fqcn)
    except ET.ParseError as e:
        print(json.dumps({"error": f"invalid xml: {e}"}), file=sys.stderr)
        return 2

    json.dump(result, sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0 if result["summary"]["all_pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
