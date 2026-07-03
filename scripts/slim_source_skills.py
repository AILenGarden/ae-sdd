#!/usr/bin/env python3
"""
slim_source_skills.py - standardize ae-sdd source SKILL slimming.

The slimmer edits ae-sdd's human-maintained source SKILL entry files, but never
uses the already-slimmed text as the semantic input. Full pre-slim source is
kept under source/skill-fallbacks/** and every slim entry is rendered from that
fallback through the same schema, template, and semantic inventory rules.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")


SLIMMER_VERSION = "2"
SLIM_SCHEMA = "ae-sdd-source-slim/v2"
STANDARD_REL = "standards/skill-source-slimming-standard.md"
TEMPLATE_REL = "templates/skill/source-skill-slim-entry-template.md"
HEADING_LIMIT = 120
REF_LIMIT = 120
REQUIRED_SECTIONS = [
    "## Load Contract",
    "## Semantic Inventory",
    "## Source Slimming SOP",
    "## Headings",
    "## Inline References",
]


SEMANTIC_CATEGORIES: list[dict[str, Any]] = [
    {
        "id": "identity_trigger",
        "label": "Identity and trigger semantics",
        "patterns": [r"trigger", r"use when", r"适用", r"触发", r"入口", r"路由到", r"调用"],
        "design_refs": ["source/docs/ae-sdd-design.md §2/§16/§18", "source/docs/skill-runtime-compiler.md §2"],
        "fallback_policy": "Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback.",
    },
    {
        "id": "workflow_route",
        "label": "Workflow and routing semantics",
        "patterns": [r"workflow", r"route", r"Step\s*\d+", r"Phase", r"流程", r"路由", r"阶段", r"状态机", r"子链"],
        "design_refs": ["source/docs/ae-sdd-design.md §2/§16", "source/standards/update-graph.json"],
        "fallback_policy": "Index the route/workflow outline; load fallback before executing low-frequency branch detail.",
    },
    {
        "id": "gate_constraint",
        "label": "Gate and hard constraint semantics",
        "patterns": [r"G-[A-Z0-9-]+", r"gate", r"门禁", r"MUST", r"必须", r"禁止", r"不得", r"BLOCK", r"WARN", r"ASK_USER"],
        "design_refs": ["source/docs/ae-sdd-design.md §5", "tools/lib/gates.py:GATE_REGISTRY"],
        "fallback_policy": "Preserve gate identifiers in index; CLI gate output remains higher authority than prose.",
    },
    {
        "id": "tool_command",
        "label": "Tool, command, and API semantics",
        "patterns": [r"ae-sdd\s+[A-Za-z0-9_-]+", r"CLI", r"命令", r"工具", r"API", r"接口", r"scripts/", r"tools/", r"python\s+"],
        "design_refs": ["source/docs/ae-sdd-implementation-architecture.md §4/§5", "source/docs/ae-sdd-design.md §13"],
        "fallback_policy": "Index command/API references; full invocation contracts stay in fallback or implementation docs.",
    },
    {
        "id": "state_data",
        "label": "State and data model semantics",
        "patterns": [r"state\.json", r"\bphase\b", r"状态", r"字段", r"JSON", r"YAML", r"config", r"reviewConsensus", r"manifest"],
        "design_refs": ["source/docs/ae-sdd-design.md §3/§15/§19", "tools/lib/state.py"],
        "fallback_policy": "Index state/config vocabulary; use tools/lib state output as execution truth.",
    },
    {
        "id": "output_doc_contract",
        "label": "Output and document contract semantics",
        "patterns": [r"输出", r"产出", r"文档", r"模板", r"保存", r"落地", r"ChangeLog", r"report", r"artifact", r"finalize"],
        "design_refs": ["source/docs/ae-sdd-design.md §7", "source/templates/**"],
        "fallback_policy": "Index document/output obligations; load fallback before generating exact long-form artifacts.",
    },
    {
        "id": "resource_reference",
        "label": "Resource and dependency reference semantics",
        "patterns": [r"source/", r"standards/", r"templates/", r"skills/", r"assets/", r"\.md", r"fallback"],
        "design_refs": ["source/standards/**", "source/templates/**", "source/skills/**"],
        "fallback_policy": "Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor.",
    },
    {
        "id": "design_alignment",
        "label": "Design and implementation alignment semantics",
        "patterns": [r"设计", r"实现", r"对齐", r"update-check", r"UC-\d+", r"Runtime IR", r"architecture", r"§"],
        "design_refs": [
            "source/docs/ae-sdd-design.md",
            "source/docs/ae-sdd-implementation-architecture.md",
            "source/docs/skill-runtime-compiler.md",
        ],
        "fallback_policy": "Index the alignment surface; update design docs before changing behavior.",
    },
    {
        "id": "fallback_only_detail",
        "label": "Fallback-only detail semantics",
        "patterns": [r"示例", r"例子", r"FAQ", r"历史", r"背景", r"变更", r"CHANGELOG", r"rationale", r"说明"],
        "design_refs": ["source/skill-fallbacks/**", "source/CHANGELOG/**"],
        "fallback_policy": "Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail.",
    },
]


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _is_true(value: str | None) -> bool:
    return str(value or "").strip().lower() in {"true", "yes", "1"}


def split_frontmatter(text: str) -> tuple[str, str]:
    candidate = text[1:] if text.startswith("\ufeff") else text
    match = re.match(r"^---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|$)", candidate, re.DOTALL)
    if not match:
        return "", candidate
    return match.group(1), candidate[match.end():]


def parse_frontmatter(text: str) -> dict[str, str]:
    frontmatter, _body = split_frontmatter(text)
    values: dict[str, str] = {}
    lines = frontmatter.replace("\r\n", "\n").replace("\r", "\n").splitlines()
    idx = 0
    while idx < len(lines):
        line = lines[idx]
        match = re.match(r"^([A-Za-z0-9_-]+):\s*(.*?)\s*$", line)
        if not match:
            idx += 1
            continue
        key = match.group(1)
        raw = match.group(2).strip()
        if raw in {"|", "|-", "|+", ">", ">-", ">+"}:
            idx += 1
            block_lines: list[str] = []
            while idx < len(lines) and (lines[idx].startswith((" ", "\t")) or not lines[idx].strip()):
                block_lines.append(lines[idx].strip())
                idx += 1
            values[key] = "\n".join(block_lines).strip()
            continue
        if (raw.startswith('"') and raw.endswith('"')) or (raw.startswith("'") and raw.endswith("'")):
            raw = raw[1:-1]
        values[key] = raw
        idx += 1
    return values


def _clean_frontmatter(frontmatter: str) -> str:
    lines = frontmatter.replace("\r\n", "\n").replace("\r", "\n").splitlines()
    cleaned: list[str] = []
    idx = 0
    while idx < len(lines):
        line = lines[idx]
        match = re.match(r"^([A-Za-z0-9_-]+):", line)
        if match and match.group(1).startswith("source_"):
            idx += 1
            while idx < len(lines) and (lines[idx].startswith((" ", "\t")) or not lines[idx].strip()):
                idx += 1
            continue
        cleaned.append(line.rstrip())
        idx += 1
    return "\n".join(line for line in cleaned if line.strip())


def _yaml_scalar(value: str) -> str:
    if re.fullmatch(r"[A-Za-z0-9_.@/+:-]+", value):
        return value
    return json.dumps(value, ensure_ascii=False)


def extract_headings(text: str, limit: int = HEADING_LIMIT) -> list[dict[str, Any]]:
    headings: list[dict[str, Any]] = []
    for line_no, line in enumerate(text.splitlines(), start=1):
        match = re.match(r"^(#{1,6})\s+(.+?)\s*$", line)
        if not match:
            continue
        title = re.sub(r"\s+", " ", match.group(2)).strip()
        headings.append({"level": len(match.group(1)), "line": line_no, "title": title})
        if len(headings) >= limit:
            break
    return headings


def extract_inline_refs(text: str, limit: int = REF_LIMIT) -> list[str]:
    refs: set[str] = set()
    for match in re.finditer(r"`([^`\n]+)`", text):
        value = match.group(1).strip()
        if not value:
            continue
        if ".md" in value or value.startswith("ae-sdd ") or "/" in value or "\\" in value:
            refs.add(value)
    return sorted(refs)[:limit]


def _title_from_body(body: str, fallback: str) -> str:
    for line in body.splitlines():
        match = re.match(r"^#\s+(.+?)\s*$", line)
        if match:
            return match.group(1).strip()
    return fallback


def _first_paragraph(body: str, limit: int = 500) -> str:
    lines: list[str] = []
    for raw in body.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or line == "---":
            if lines:
                break
            continue
        if line.startswith("```") or line.startswith("|"):
            if lines:
                break
            continue
        lines.append(line)
        if len(" ".join(lines)) >= limit:
            break
    text = " ".join(lines)
    return text if len(text) <= limit else text[: limit - 3].rstrip() + "..."


def _table(headers: list[str], rows: list[tuple[Any, ...]]) -> str:
    def cell(value: Any) -> str:
        return str(value).replace("|", "\\|").replace("\n", " ").strip()

    lines = [
        "| " + " | ".join(cell(h) for h in headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(cell(v) for v in row) + " |")
    return "\n".join(lines)


def _short_join(items: list[str], limit: int = 3) -> str:
    selected = [item for item in items if item][:limit]
    if not selected:
        return ""
    suffix = "" if len(items) <= limit else f"; +{len(items) - limit} more"
    return "; ".join(selected) + suffix


def _compile_patterns(patterns: list[str]) -> re.Pattern[str]:
    return re.compile("|".join(f"(?:{pattern})" for pattern in patterns), re.IGNORECASE)


def _semantic_evidence(
    category: dict[str, Any],
    text: str,
    metadata: dict[str, str],
    headings: list[dict[str, Any]],
    refs: list[str],
) -> str:
    category_id = category["id"]
    patterns = _compile_patterns(category["patterns"])
    evidence: list[str] = []

    if category_id == "identity_trigger":
        keys = [key for key in ("name", "description", "version") if metadata.get(key)]
        if keys:
            evidence.append("frontmatter: " + ", ".join(keys))
    if category_id == "resource_reference" and refs:
        evidence.append(f"inline_refs: {len(refs)}")
        evidence.append("refs: " + _short_join(refs))

    matching_headings = [
        f"L{heading['level']}:{heading['line']} {heading['title']}"
        for heading in headings
        if patterns.search(str(heading["title"]))
    ]
    if matching_headings:
        evidence.append("headings: " + _short_join(matching_headings))

    hit_count = len(patterns.findall(text))
    if hit_count:
        evidence.append(f"keyword_hits: {hit_count}")

    return _short_join(evidence, limit=4)


def recognize_semantics(original_text: str) -> list[dict[str, str]]:
    metadata = parse_frontmatter(original_text)
    headings = extract_headings(original_text)
    refs = extract_inline_refs(original_text)
    records: list[dict[str, str]] = []
    for category in SEMANTIC_CATEGORIES:
        evidence = _semantic_evidence(category, original_text, metadata, headings, refs)
        if not evidence:
            continue
        records.append(
            {
                "category": category["id"],
                "label": category["label"],
                "evidence": evidence,
                "design_refs": "; ".join(category["design_refs"]),
                "fallback_policy": category["fallback_policy"],
            }
        )
    if not any(record["category"] == "identity_trigger" for record in records):
        records.insert(
            0,
            {
                "category": "identity_trigger",
                "label": "Identity and trigger semantics",
                "evidence": "implicit source entry identity",
                "design_refs": "source/docs/skill-runtime-compiler.md §2",
                "fallback_policy": "Keep source identity in slim metadata; load fallback for exact wording.",
            },
        )
    return records


def semantic_inventory_sha256(records: list[dict[str, str]]) -> str:
    payload = json.dumps(records, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return _sha256_text(payload)


def fallback_rel_for(source_root: Path, skill_path: Path) -> Path:
    rel = skill_path.relative_to(source_root)
    if rel.as_posix() == "SKILL.md":
        return Path("skill-fallbacks") / "SKILL.full.md"
    return Path("skill-fallbacks") / rel.with_suffix(".full.md")


def _fallback_abs_from_metadata(source_root: Path, metadata: dict[str, str], skill_path: Path) -> tuple[Path, Path]:
    fallback_rel_raw = metadata.get("source_fallback")
    fallback_rel = Path(fallback_rel_raw) if fallback_rel_raw else fallback_rel_for(source_root, skill_path)
    fallback_abs = (source_root / fallback_rel).resolve()
    try:
        fallback_abs.relative_to(source_root.resolve())
    except ValueError as exc:
        raise RuntimeError(f"source_fallback must stay inside source root: {fallback_rel}") from exc
    return fallback_rel, fallback_abs


def render_slim_entry(source_root: Path, skill_path: Path, original_text: str, fallback_rel: Path) -> str:
    frontmatter, body = split_frontmatter(original_text)
    metadata = parse_frontmatter(original_text)
    cleaned_frontmatter = _clean_frontmatter(frontmatter)
    fallback_posix = fallback_rel.as_posix()
    fallback_sha = _sha256_text(original_text)
    original_lines = original_text.count("\n") + 1
    original_bytes = len(original_text.encode("utf-8"))
    title = _title_from_body(body, metadata.get("name") or skill_path.stem)
    summary = metadata.get("description") or _first_paragraph(body)
    headings = extract_headings(original_text)
    refs = extract_inline_refs(original_text)
    semantics = recognize_semantics(original_text)
    semantic_hash = semantic_inventory_sha256(semantics)
    rel = skill_path.relative_to(source_root).as_posix()

    semantic_rows = [
        (record["category"], record["evidence"], record["design_refs"], record["fallback_policy"])
        for record in semantics
    ]
    heading_rows = [(h["level"], h["line"], h["title"]) for h in headings] or [("-", "-", "(no headings extracted)")]
    ref_rows = [(ref,) for ref in refs] or [("(no inline refs extracted)",)]

    fm_lines = ["---"]
    if cleaned_frontmatter:
        fm_lines.append(cleaned_frontmatter)
    fm_lines.extend(
        [
            "source_slimmed: true",
            f"source_slim_schema: {SLIM_SCHEMA}",
            f"source_slim_standard: {STANDARD_REL}",
            f"source_slim_template: {TEMPLATE_REL}",
            f"source_fallback: {fallback_posix}",
            f"source_fallback_sha256: {fallback_sha}",
            f"source_original_bytes: {original_bytes}",
            f"source_original_lines: {original_lines}",
            f"source_semantic_inventory_sha256: {semantic_hash}",
            f"source_slimmer: slim_source_skills.py@{SLIMMER_VERSION}",
            "---",
        ]
    )
    frontmatter_text = "\n".join(fm_lines)

    return f"""{frontmatter_text}

# {title} Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `{fallback_posix}` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `{fallback_posix}` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `{fallback_posix}`, not this slim entry.

## Summary

- source: `{rel}`
- fallback: `{fallback_posix}`
- fallback_sha256: `{fallback_sha}`
- original_lines: {original_lines}
- original_bytes: {original_bytes}
- semantic_inventory_sha256: `{semantic_hash}`
- standard: `{STANDARD_REL}`
- template: `{TEMPLATE_REL}`
- summary: {summary or "(none)"}

## Semantic Inventory

{_table(["category", "evidence", "design_refs", "fallback_policy"], semantic_rows)}

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `{TEMPLATE_REL}`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

{_table(["level", "line", "title"], heading_rows)}

## Inline References

{_table(["ref"], ref_rows)}
"""


def discover_ae_sdd_source_skills(source_root: Path) -> list[Path]:
    files = [source_root / "SKILL.md"]
    skills_root = source_root / "skills"
    if skills_root.is_dir():
        files.extend(sorted(skills_root.rglob("*.md")))
    return [path for path in files if path.is_file()]


def validate_slim_entry(source_root: Path, skill_path: Path) -> list[str]:
    errors: list[str] = []
    text = skill_path.read_text(encoding="utf-8")
    metadata = parse_frontmatter(text)
    rel = skill_path.relative_to(source_root).as_posix()

    if not _is_true(metadata.get("source_slimmed")):
        return [f"{rel}: source_slimmed is not true"]
    if metadata.get("source_slim_schema") != SLIM_SCHEMA:
        errors.append(f"{rel}: source_slim_schema is not {SLIM_SCHEMA}")
    if metadata.get("source_slim_standard") != STANDARD_REL:
        errors.append(f"{rel}: source_slim_standard is not {STANDARD_REL}")
    if metadata.get("source_slim_template") != TEMPLATE_REL:
        errors.append(f"{rel}: source_slim_template is not {TEMPLATE_REL}")

    try:
        fallback_rel, fallback_abs = _fallback_abs_from_metadata(source_root, metadata, skill_path)
    except RuntimeError as exc:
        return [f"{rel}: {exc}"]
    if not fallback_abs.is_file():
        return [*errors, f"{rel}: fallback missing: {fallback_rel.as_posix()}"]

    fallback_text = fallback_abs.read_text(encoding="utf-8")
    fallback_sha = _sha256_text(fallback_text)
    if metadata.get("source_fallback_sha256") != fallback_sha:
        errors.append(f"{rel}: source_fallback_sha256 mismatch")

    semantics = recognize_semantics(fallback_text)
    semantic_hash = semantic_inventory_sha256(semantics)
    if metadata.get("source_semantic_inventory_sha256") != semantic_hash:
        errors.append(f"{rel}: source_semantic_inventory_sha256 mismatch")

    for section in REQUIRED_SECTIONS:
        if section not in text:
            errors.append(f"{rel}: missing required section {section}")

    expected = render_slim_entry(source_root, skill_path, fallback_text, fallback_rel)
    if text != expected:
        errors.append(f"{rel}: slim entry does not match {TEMPLATE_REL} rendering")
    return errors


def _failure(failures: list[dict[str, str]], source_root: Path, skill_path: Path, message: str) -> None:
    rel = skill_path.relative_to(source_root).as_posix() if skill_path.is_relative_to(source_root) else str(skill_path)
    failures.append({"source": rel, "error": message})


def slim_source_skills(
    source_root: Path,
    *,
    dry_run: bool = False,
    upgrade: bool = False,
    validate_existing: bool = True,
    validate_only: bool = False,
) -> dict[str, Any]:
    source_root = source_root.resolve()
    records: list[dict[str, Any]] = []
    skipped: list[dict[str, str]] = []
    failures: list[dict[str, str]] = []
    validated = 0

    skill_paths = discover_ae_sdd_source_skills(source_root)
    for skill_path in skill_paths:
        rel = skill_path.relative_to(source_root).as_posix()
        try:
            current_text = skill_path.read_text(encoding="utf-8")
            metadata = parse_frontmatter(current_text)
            fallback_rel, fallback_abs = _fallback_abs_from_metadata(source_root, metadata, skill_path)

            if _is_true(metadata.get("source_slimmed")):
                should_upgrade = upgrade and metadata.get("source_slim_schema") != SLIM_SCHEMA
                if should_upgrade:
                    if not fallback_abs.is_file():
                        raise RuntimeError(f"fallback missing: {fallback_rel.as_posix()}")
                    original_text = fallback_abs.read_text(encoding="utf-8")
                    existing_sha = metadata.get("source_fallback_sha256")
                    if existing_sha and existing_sha != _sha256_text(original_text):
                        raise RuntimeError("existing source_fallback_sha256 does not match fallback; refusing upgrade")
                    record = {
                        "source": rel,
                        "fallback": fallback_rel.as_posix(),
                        "action": "upgrade",
                        "original_lines": original_text.count("\n") + 1,
                        "original_bytes": len(original_text.encode("utf-8")),
                        "fallback_sha256": _sha256_text(original_text),
                    }
                    records.append(record)
                    if not dry_run and not validate_only:
                        _write_text(skill_path, render_slim_entry(source_root, skill_path, original_text, fallback_rel))
                    if not dry_run and validate_existing:
                        errors = validate_slim_entry(source_root, skill_path)
                        if errors:
                            failures.extend({"source": rel, "error": error} for error in errors)
                        else:
                            validated += 1
                    continue

                skipped.append(
                    {
                        "source": rel,
                        "reason": "already-slimmed-v2"
                        if metadata.get("source_slim_schema") == SLIM_SCHEMA
                        else "already-slimmed-needs-upgrade",
                    }
                )
                if validate_existing or validate_only:
                    errors = validate_slim_entry(source_root, skill_path)
                    if errors:
                        failures.extend({"source": rel, "error": error} for error in errors)
                    else:
                        validated += 1
                continue

            if validate_only:
                failures.append({"source": rel, "error": "source is not slimmed"})
                continue
            if fallback_abs.exists():
                raise RuntimeError(f"fallback already exists for unslimmed source; refusing to overwrite: {fallback_abs}")

            original_text = current_text
            record = {
                "source": rel,
                "fallback": fallback_rel.as_posix(),
                "action": "slim",
                "original_lines": original_text.count("\n") + 1,
                "original_bytes": len(original_text.encode("utf-8")),
                "fallback_sha256": _sha256_text(original_text),
            }
            records.append(record)
            if dry_run:
                continue
            _write_text(fallback_abs, original_text)
            _write_text(skill_path, render_slim_entry(source_root, skill_path, original_text, fallback_rel))
            if validate_existing:
                errors = validate_slim_entry(source_root, skill_path)
                if errors:
                    failures.extend({"source": rel, "error": error} for error in errors)
                else:
                    validated += 1
        except Exception as exc:
            _failure(failures, source_root, skill_path, str(exc))

    slimmed = sum(1 for record in records if record["action"] == "slim")
    upgraded = sum(1 for record in records if record["action"] == "upgrade")
    return {
        "schema": SLIM_SCHEMA,
        "slimmer": f"slim_source_skills.py@{SLIMMER_VERSION}",
        "standard": STANDARD_REL,
        "template": TEMPLATE_REL,
        "source_root": source_root.name,
        "dry_run": dry_run,
        "upgrade": upgrade,
        "validate_only": validate_only,
        "counts": {
            "discovered": len(skill_paths),
            "slimmed": 0 if dry_run else slimmed,
            "upgraded": 0 if dry_run else upgraded,
            "planned": len(records) if dry_run else 0,
            "skipped": len(skipped),
            "validated": validated,
            "failed": len(failures),
        },
        "records": records,
        "skipped": skipped,
        "failures": failures,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Slim ae-sdd source SKILL files with standard semantic inventories.")
    parser.add_argument("--source-root", type=Path, default=None, help="Default: <repo>/source")
    parser.add_argument("--dry-run", action="store_true", help="Report files that would be slimmed/upgraded without writing")
    parser.add_argument("--upgrade", action="store_true", help="Re-render already-slimmed files from fallback when schema is old")
    parser.add_argument("--validate", action="store_true", help="Validate slim entries only; do not slim or upgrade")
    parser.add_argument("--no-validate", action="store_true", help="Skip validation after normal slim/upgrade operations")
    parser.add_argument("--json", action="store_true", help="Print JSON summary")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    source_root = args.source_root.resolve() if args.source_root else repo_root / "source"
    result = slim_source_skills(
        source_root,
        dry_run=args.dry_run,
        upgrade=args.upgrade,
        validate_existing=not args.no_validate,
        validate_only=args.validate,
    )

    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        counts = result["counts"]
        print(
            "[source-slim] ok "
            f"schema={result['schema']} "
            f"slimmed={counts['slimmed']} "
            f"upgraded={counts['upgraded']} "
            f"planned={counts['planned']} "
            f"skipped={counts['skipped']} "
            f"validated={counts['validated']} "
            f"failed={counts['failed']}"
        )
        if result["failures"]:
            for failure in result["failures"][:20]:
                print(f"[source-slim] FAIL {failure['source']}: {failure['error']}", file=sys.stderr)
            if len(result["failures"]) > 20:
                print(f"[source-slim] FAIL ... {len(result['failures']) - 20} more", file=sys.stderr)
    return 1 if result["counts"]["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
