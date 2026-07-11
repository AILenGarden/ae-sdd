#!/usr/bin/env python3
"""
compile_skill_runtime.py - compile ae-sdd source package into a compact runtime package.

This script runs after build_dist.py has copied source/ into dist/ae-sdd/.
It turns dist/ae-sdd/SKILL.md into a short bootloader and emits runtime/*.compact.md.
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
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

COMPILER_VERSION = "2"
SUBSKILL_SCHEMA = "ae-sdd-subskill-runtime/v1"

LOAD_ORDER = [
    "runtime/boot.compact.md",
    "runtime/route.compact.md",
    "runtime/subskills.compact.md",
    "runtime/gates.compact.md",
    "runtime/flow.compact.md",
    "runtime/macros.compact.md",
]


ROUTE_ROWS = [
    ("self-update", "modify/optimize/update ae-sdd or SKILL", "skills/orchestration/ae-sdd-update-skill.md"),
    ("resume", "continue/resume/previous flow", "read state, report phase, load next slice"),
    ("large", "has DR", "DR -> Story -> TestCase -> CodingPlan -> Coding -> Test -> Review"),
    ("medium", "has Story", "Story -> TestCase -> CodingPlan -> Coding -> Test -> Review"),
    ("small", "has Story+TestCase, enter at CodingPlan", "CodingPlan -> Coding -> Test -> Review"),
    ("micro", "BUG/config, no docs", "CodingPlan -> Coding -> Test -> Review"),
    ("doc-storage", "where to write/read generated docs", "skills/cross-cutting/document-storage-skill.md"),
    ("plugin", "load/override coding skill", "skills/cross-cutting/ae-sdd-plugin-loader-skill.md"),
]


MACROS = [
    ("BLOCK", "Stop current flow, report gate/reason/action, do not proceed."),
    ("WARN", "Report risk; continuation is allowed."),
    ("SKIP", "Gate or phase is explicitly exempted for this route."),
    ("ASK_USER", "Wait for explicit user confirmation before continuing."),
    ("LOOP3", "Repeat generate/review/fix until 3 consecutive rounds have no new issues."),
    ("EVIDENCE", "Claims need source + reasoning + uncertainty; fabricated sources invalidate the claim."),
    ("STATE_READ", "Read ae-sdd state before resuming or deciding next phase."),
    ("STATE_WRITE", "Write phase/event only after gates and user confirmation requirements are satisfied."),
    ("LOAD_SLICE", "Read only the runtime slice needed by the current route; fallback lazily."),
]

SCALE_ORDER = ["大", "中", "小", "微"]


GATE_HINTS = {
    "G-00": ("entry", "project assets exist and 7-layer index is complete", "BLOCK -> project-assets-update"),
    "G-RA-1": ("before DR/Story/Task generation", "RA document exists or route is exempt", "BLOCK"),
    "G-RA-2": ("before DR/Story/Task generation", "RA dimensions and RAModel are complete", "BLOCK"),
    "G-RA-3": ("before DR/Story/Task generation", "RA derivative sections are complete", "BLOCK"),
    "G-RA-4": ("before DR/Story/Task generation", "RA authenticity scanner passes", "BLOCK"),
    "G-RA-5": ("before DR/Story/Task generation", "RA mechanical derivation scanner passes", "BLOCK"),
    "G-RA-6": ("before DR/Story/Task generation", "RA implementation-view scanner passes", "BLOCK"),
    "G-RA-FLOW-VIOLATION": ("before downstream generation", "RA flow violation scanner passes", "BLOCK"),
    "G-DOC-STORAGE": ("before doc write", "path/name resolved by document-storage", "BLOCK"),
    "G-DOC-CONSISTENCY": ("entry/doc workspace check", "project memory path agrees with config", "BLOCK"),
    "G-CODEPLAN-SRC": ("before coding execute", "CodingPlan class skeleton has source-read evidence", "BLOCK"),
    "G-14": ("before coding execute", "CodingPlan references Story and aligns with AC", "BLOCK"),
    "G-08": ("before coding execute", "CodingPlan 14 gates are present", "BLOCK"),
    "G-09": ("test review", "test authenticity scanner passes", "BLOCK"),
    "G-09B": ("review phase transition", "reviewer independence requirement passes", "BLOCK"),
    "G-CODE-1": ("coding/code review", "coding authenticity scanner passes", "BLOCK"),
    "G-REVIEW-LOOP": ("review phase transition", "review-loop exit condition is satisfied", "BLOCK"),
    "G-PATH": ("build/update check", "source docs do not hardcode output paths", "BLOCK"),
}


def parse_version(text: str) -> str:
    match = re.search(r"^version:\s*([^\s]+)", text, re.MULTILINE)
    return match.group(1) if match else "0.0.0"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def _stable_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _escape_cell(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ").strip()


def _table(headers: list[str], rows: list[tuple[Any, ...]]) -> str:
    lines = [
        "| " + " | ".join(_escape_cell(h) for h in headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(_escape_cell(c) for c in row) + " |")
    return "\n".join(lines)


def _yaml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def _yaml_block_scalar(value: str) -> str:
    text = value.replace("\r\n", "\n").replace("\r", "\n").strip()
    if not text:
        text = "ae-sdd compiled sub-SKILL entry."
    return "\n".join(f"  {line}" if line else "  " for line in text.split("\n"))


def read_frontmatter(text: str) -> dict[str, str]:
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    data: dict[str, str] = {}
    lines = text[3:end].splitlines()
    idx = 0
    while idx < len(lines):
        line = lines[idx]
        if ":" not in line:
            idx += 1
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        value = value.strip()
        if value in {"|", ">"}:
            idx += 1
            parts: list[str] = []
            while idx < len(lines) and (lines[idx].startswith(" ") or not lines[idx].strip()):
                parts.append(lines[idx].strip())
                idx += 1
            data[key] = " ".join(part for part in parts if part).strip()
            continue
        data[key] = value.strip().strip('"').strip("'")
        idx += 1
    return data


def _is_true(value: str | None) -> bool:
    return str(value or "").strip().lower() in {"true", "yes", "1"}


def _source_fallback_path(source: Path, metadata: dict[str, str]) -> Path | None:
    rel = metadata.get("source_fallback") or metadata.get("slim_fallback")
    if not rel:
        return None
    candidate = (source / rel).resolve()
    try:
        candidate.relative_to(source.resolve())
    except ValueError:
        raise ValueError(f"source_fallback must stay inside source root: {rel}")
    return candidate


def _read_source_fallback_text(
    source: Path,
    source_skill: Path,
    metadata: dict[str, str],
    default_text: str,
) -> str:
    if not _is_true(metadata.get("source_slimmed")):
        return default_text
    fallback_path = _source_fallback_path(source, metadata)
    if fallback_path is None:
        return default_text
    if not fallback_path.is_file():
        raise FileNotFoundError(f"source fallback missing for {source_skill}: {fallback_path}")
    return fallback_path.read_text(encoding="utf-8")


def extract_headings(text: str, limit: int = 80) -> list[dict[str, Any]]:
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


def extract_inline_refs(text: str, limit: int = 80) -> list[str]:
    refs: set[str] = set()
    for match in re.finditer(r"`([^`\n]+)`", text):
        value = match.group(1).strip()
        if not value:
            continue
        if ".md" in value or value.startswith("ae-sdd ") or "/" in value or "\\" in value:
            refs.add(value)
    return sorted(refs)[:limit]


def subskill_runtime_base(rel: Path) -> Path:
    stem_rel = rel.with_suffix("")
    parts = stem_rel.parts
    if parts and parts[0] == "skills":
        parts = parts[1:]
    return Path("runtime") / "skills" / Path(*parts)


def render_subskill_boot_compact(record: dict[str, Any]) -> str:
    return f"""# ae-sdd Sub-SKILL Boot Compact

- schema: {SUBSKILL_SCHEMA}
- entry: `{record["entry"]}`
- source: `{record["source_path"]}`
- runtime_fingerprint: {record["runtime_fingerprint"]}
- deterministic: true

## Load Order

1. `{record["manifest"]}`
2. `{record["boot"]}`
3. `{record["outline"]}`

## Runtime Contract

- Use this compact entry before reading the full source fallback.
- Keep the public entry path stable: `{record["entry"]}`.
- Load fallback only when outline and compact route do not contain enough detail.
- Fallback source: `{record["fallback"]}`.
"""


def render_subskill_outline_compact(
    rel: str,
    metadata: dict[str, str],
    headings: list[dict[str, Any]],
    refs: list[str],
) -> str:
    heading_rows = [
        (item["level"], item["line"], item["title"])
        for item in headings
    ] or [("-", "-", "(no headings extracted)")]
    ref_rows = [(ref,) for ref in refs] or [("(no inline refs extracted)",)]
    return (
        "# ae-sdd Sub-SKILL Outline Compact\n\n"
        f"- entry: `{rel}`\n"
        f"- name: {metadata.get('name') or Path(rel).stem}\n"
        f"- description: {metadata.get('description') or '(none)'}\n\n"
        "## Headings\n\n"
        + _table(["level", "line", "title"], heading_rows)
        + "\n\n## Inline References\n\n"
        + _table(["ref"], ref_rows)
        + "\n"
    )


def render_subskill_entry(
    rel: str,
    metadata: dict[str, str],
    manifest_rel: str,
    boot_rel: str,
    outline_rel: str,
    fallback_rel: str,
    runtime_fingerprint: str,
) -> str:
    name = metadata.get("name") or Path(rel).stem
    description = metadata.get("description") or f"ae-sdd compiled sub-SKILL entry for {rel}."
    return f"""---
name: {_yaml_string(name)}
description: |-
{_yaml_block_scalar(description)}
compiled: true
runtime: {manifest_rel}
source: source/{rel}
---

# {name} Compiled Sub-SKILL Entry

This is a generated compact entry for an ae-sdd child SKILL. Do not hand-edit it.

## Load

1. Read `{manifest_rel}`.
2. Read `{boot_rel}`.
3. Read `{outline_rel}`.
4. Use fallback only when compact runtime is insufficient: `{fallback_rel}`.

runtime_fingerprint: {runtime_fingerprint}
"""


def compile_subskills(source: Path, dist: Path) -> tuple[list[dict[str, Any]], list[str]]:
    skills_root = source / "skills"
    if not skills_root.is_dir():
        return [], []

    records: list[dict[str, Any]] = []
    generated_files: list[str] = []
    for source_skill in sorted(skills_root.rglob("*.md")):
        rel_path = source_skill.relative_to(source)
        rel = rel_path.as_posix()
        text = source_skill.read_text(encoding="utf-8")
        metadata = read_frontmatter(text)
        fallback_text = _read_source_fallback_text(source, source_skill, metadata, text)
        headings = extract_headings(fallback_text)
        refs = extract_inline_refs(fallback_text)
        source_sha = sha256_file(source_skill)
        fallback_sha = _sha256_text(fallback_text)

        base_rel = subskill_runtime_base(rel_path)
        manifest_rel = (base_rel / "manifest.json").as_posix()
        boot_rel = (base_rel / "boot.compact.md").as_posix()
        outline_rel = (base_rel / "outline.compact.md").as_posix()
        fallback_rel = (base_rel / "fallback" / "SKILL.full.md").as_posix()

        fingerprint_payload = {
            "schema": SUBSKILL_SCHEMA,
            "compiler": {"name": "compile_skill_runtime.py", "version": COMPILER_VERSION},
            "entry": rel,
            "source_sha256": source_sha,
            "fallback_sha256": fallback_sha,
            "metadata": metadata,
            "headings": headings,
            "refs": refs,
            "contract": ["manifest", "boot.compact.md", "outline.compact.md", "fallback/SKILL.full.md"],
        }
        runtime_fingerprint = _sha256_text(_stable_json(fingerprint_payload))

        record = {
            "entry": rel,
            "source_path": f"source/{rel}",
            "manifest": manifest_rel,
            "boot": boot_rel,
            "outline": outline_rel,
            "fallback": fallback_rel,
            "source_sha256": source_sha,
            "fallback_sha256": fallback_sha,
            "runtime_fingerprint": runtime_fingerprint,
            "heading_count": len(headings),
            "ref_count": len(refs),
            "source_slimmed": _is_true(metadata.get("source_slimmed")),
        }

        manifest = {
            "schema": SUBSKILL_SCHEMA,
            "compiled": True,
            "deterministic": True,
            "compiler": {"name": "compile_skill_runtime.py", "version": COMPILER_VERSION},
            "entry": rel,
            "source_path": f"source/{rel}",
            "runtime_fingerprint": runtime_fingerprint,
            "load_order": [boot_rel, outline_rel],
            "generated_files": [manifest_rel, boot_rel, outline_rel, fallback_rel, rel],
            "source": {
                "sha256": source_sha,
                "fallback_sha256": fallback_sha,
                "source_slimmed": _is_true(metadata.get("source_slimmed")),
                "source_fallback": metadata.get("source_fallback", ""),
            },
            "extracts": {
                "heading_count": len(headings),
                "ref_count": len(refs),
            },
        }

        _write_text(dist / fallback_rel, fallback_text)
        _write_text(dist / boot_rel, render_subskill_boot_compact(record))
        _write_text(dist / outline_rel, render_subskill_outline_compact(rel, metadata, headings, refs))
        _write_text(dist / manifest_rel, json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
        _write_text(
            dist / rel,
            render_subskill_entry(
                rel,
                metadata,
                manifest_rel,
                boot_rel,
                outline_rel,
                fallback_rel,
                runtime_fingerprint,
            ),
        )

        records.append(record)
        generated_files.extend([manifest_rel, boot_rel, outline_rel, fallback_rel, rel])

    return records, sorted(generated_files)


def render_subskills_compact(subskills: list[dict[str, Any]]) -> str:
    rows = [
        (
            item["entry"],
            item["manifest"],
            item["outline"],
            item["fallback"],
            item["heading_count"],
        )
        for item in subskills
    ]
    return (
        "# ae-sdd Sub-SKILL Index Compact\n\n"
        f"- subskill_count: {len(subskills)}\n"
        "- entry files under `skills/` are compiled bootloaders.\n"
        "- full source fallbacks live under `runtime/skills/**/fallback/SKILL.full.md`.\n\n"
        + _table(["entry", "manifest", "outline", "fallback", "headings"], rows)
        + "\n"
    )


def collect_source_checksums(source: Path) -> dict[str, str]:
    patterns = [
        "SKILL.md",
        "HARNESS.md",
        "skill-fallbacks/**/*.md",
        "skills/**/*.md",
        "standards/**/*.md",
        "templates/**/*.md",
        "assets/**/*.md",
    ]
    files: dict[str, str] = {}
    seen: set[Path] = set()
    for pattern in patterns:
        for path in source.glob(pattern):
            if not path.is_file() or path in seen:
                continue
            seen.add(path)
            rel = path.relative_to(source).as_posix()
            files[rel] = sha256_file(path)
    return dict(sorted(files.items()))


def _load_gate_registry(repo_root: Path) -> tuple[list[dict[str, Any]], list[str]]:
    sys.path.insert(0, str(repo_root / "tools"))
    try:
        from lib.gates import GATE_REGISTRY  # type: ignore
        return [dict(g) for g in GATE_REGISTRY], []
    except Exception as exc:  # pragma: no cover - defensive fallback
        return [], [f"failed to import GATE_REGISTRY: {exc}"]


def _load_phase_flows(repo_root: Path) -> tuple[dict[str, list[str]], list[str]]:
    sys.path.insert(0, str(repo_root / "tools"))
    try:
        from lib.state import PHASE_FLOWS  # type: ignore
        return {str(k): list(v) for k, v in PHASE_FLOWS.items()}, []
    except Exception as exc:  # pragma: no cover - defensive fallback
        return {}, [f"failed to import PHASE_FLOWS: {exc}"]


def compute_runtime_fingerprint(
    version: str,
    source_checksums: dict[str, str],
    fallback_hash: str,
    gates: list[dict[str, Any]],
    flows: dict[str, list[str]],
    subskills: list[dict[str, Any]],
) -> str:
    payload = {
        "compiler": {"name": "compile_skill_runtime.py", "version": COMPILER_VERSION},
        "version": version,
        "load_order": LOAD_ORDER,
        "route_rows": ROUTE_ROWS,
        "macros": MACROS,
        "gate_hints": GATE_HINTS,
        "source_checksums": source_checksums,
        "fallback_sha256": fallback_hash,
        "gates": gates,
        "flows": flows,
        "subskills": subskills,
    }
    return _sha256_text(_stable_json(payload))


def render_boot_compact(version: str, source_hash: str, runtime_fingerprint: str) -> str:
    load = "\n".join(f"{idx + 1}. `{path}`" for idx, path in enumerate(LOAD_ORDER))
    return f"""# ae-sdd Runtime Boot Compact

- schema: ae-sdd-runtime/v1
- version: {version}
- source_skill_sha256: {source_hash}
- runtime_fingerprint: {runtime_fingerprint}
- deterministic: true

## Load Order

{load}

## Runtime Contract

- Use compact runtime first; read fallback only when the current slice lacks needed detail.
- Hard gate/tool output wins over compact prose.
- Do not install or execute `source/` directly as an Agent skill package.
- `dist/ae-sdd/SKILL.md` and `runtime/*.compact.md` are generated files; do not hand-edit.
- If compact and fallback conflict, prefer compact only when `runtime/manifest.json` exists and `compiled=true`.

## Fallback

- Main fallback: `runtime/fallback/SKILL.full.md`
- Child SKILL compact index: `runtime/subskills.compact.md`
- Child SKILL entries: `skills/**/*.md` (compiled bootloaders)
- Child SKILL fallbacks: `runtime/skills/**/fallback/SKILL.full.md`
- Standards/templates: `standards/`, `templates/`
"""


def render_route_compact() -> str:
    return "# ae-sdd Route Compact\n\n" + _table(
        ["route", "when", "load/action"],
        ROUTE_ROWS,
    ) + "\n\n## Rule\n\nClassify first, then load only the matching route slice or fallback skill.\n"


def render_gates_compact(gates: list[dict[str, Any]]) -> str:
    rows: list[tuple[Any, ...]] = []
    for gate in gates:
        gate_id = str(gate.get("id", ""))
        scope, pass_rule, fail_rule = GATE_HINTS.get(
            gate_id,
            ("see CLI", f"ae-sdd gates check --only {gate_id}", "follow GateResult action"),
        )
        rows.append((
            gate_id,
            gate.get("name", ""),
            gate.get("severity", ""),
            scope,
            pass_rule,
            fail_rule,
        ))
    return (
        "# ae-sdd Gates Compact\n\n"
        f"- gate_count: {len(gates)}\n"
        "- authority: `tools/lib/gates.py:GATE_REGISTRY`\n"
        "- command: `ae-sdd gates check [--only <GATE-ID>]`\n\n"
        + _table(["gate", "name", "severity", "scope", "pass", "fail"], rows)
        + "\n"
    )


def render_flow_compact(flows: dict[str, list[str]]) -> str:
    rows = []
    for scale in _ordered_scales(flows):
        phases = flows[scale]
        rows.append((scale, len(phases), " -> ".join(phases)))
    return (
        "# ae-sdd Flow Compact\n\n"
        "- authority: `tools/lib/state.py:PHASE_FLOWS`\n"
        "- scale_values: 大 / 中 / 小 / 微\n"
        "- `paused` is a meta phase and may be entered from any phase.\n\n"
        + _table(["scale", "phase_count", "chain"], rows)
        + "\n"
    )


def _ordered_scales(flows: dict[str, list[str]]) -> list[str]:
    known = [scale for scale in SCALE_ORDER if scale in flows]
    extra = sorted(scale for scale in flows.keys() if scale not in SCALE_ORDER)
    return known + extra


def render_macros_compact() -> str:
    return "# ae-sdd Macros Compact\n\n" + _table(["macro", "meaning"], MACROS) + "\n"


def render_bootloader(version: str, runtime_fingerprint: str) -> str:
    return f"""---
name: ae-sdd
version: {version}
compiled: true
runtime: runtime/manifest.json
description: ae-sdd compiled runtime entry; load compact runtime slices first.
---

# ae-sdd Compiled Runtime Entry

This is the compiled Agent runtime entry. Do not treat it as the human-maintained source.

## Load

1. Read `runtime/manifest-index.json` (slim routing index; the full `runtime/manifest.json` is for verification only — do not read it into context).
2. Read `runtime/boot.compact.md`.
3. Read `runtime/route.compact.md`.
4. Load only the compact slice or fallback SKILL required by the route.

## Authority

- Human-maintained source: `source/` in the ae-sdd repository.
- Compiled runtime package: this `dist/ae-sdd/` package.
- Gate and state truth: `tools/bin/ae-sdd` CLI output.
- Fallback main source: `runtime/fallback/SKILL.full.md`.

## Hard Rules

- Prefer compact runtime for execution.
- Use fallback only when compact runtime is insufficient.
- CLI gate/state output overrides prompt text.
- Never hand-edit generated runtime files; rebuild with `scripts/build_dist.py`.

runtime_fingerprint: {runtime_fingerprint}
"""


def _read_fallback_text(dist_skill: Path, fallback_path: Path, source_text: str) -> str:
    if fallback_path.is_file():
        return fallback_path.read_text(encoding="utf-8")
    if dist_skill.is_file():
        return dist_skill.read_text(encoding="utf-8")
    return source_text


def compile_runtime_package(
    repo_root: Path,
    source: Path,
    dist: Path,
    build_date: str | None = None,
) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    source = source.resolve()
    dist = dist.resolve()

    source_skill = source / "SKILL.md"
    if not source_skill.is_file():
        raise FileNotFoundError(f"source SKILL.md not found: {source_skill}")
    if not dist.is_dir():
        raise FileNotFoundError(f"dist package not found: {dist}")

    source_text = source_skill.read_text(encoding="utf-8")
    source_metadata = read_frontmatter(source_text)
    version = parse_version(source_text)
    source_checksums = collect_source_checksums(source)
    source_hash = source_checksums.get("SKILL.md", sha256_file(source_skill))

    gates, gate_warnings = _load_gate_registry(repo_root)
    flows, flow_warnings = _load_phase_flows(repo_root)
    warnings = gate_warnings + flow_warnings

    runtime_dir = dist / "runtime"
    fallback_dir = runtime_dir / "fallback"
    fallback_dir.mkdir(parents=True, exist_ok=True)

    dist_skill = dist / "SKILL.md"
    fallback_path = fallback_dir / "SKILL.full.md"
    fallback_text = _read_source_fallback_text(
        source,
        source_skill,
        source_metadata,
        _read_fallback_text(dist_skill, fallback_path, source_text),
    )
    _write_text(fallback_path, fallback_text)
    fallback_hash = _sha256_text(fallback_text)

    subskills, subskill_generated_files = compile_subskills(source, dist)
    runtime_fingerprint = compute_runtime_fingerprint(version, source_checksums, fallback_hash, gates, flows, subskills)

    runtime_files = {
        "runtime/boot.compact.md": render_boot_compact(version, source_hash, runtime_fingerprint),
        "runtime/route.compact.md": render_route_compact(),
        "runtime/subskills.compact.md": render_subskills_compact(subskills),
        "runtime/gates.compact.md": render_gates_compact(gates),
        "runtime/flow.compact.md": render_flow_compact(flows),
        "runtime/macros.compact.md": render_macros_compact(),
    }
    for rel, content in runtime_files.items():
        _write_text(dist / rel, content)

    manifest: dict[str, Any] = {
        "schema": "ae-sdd-runtime/v1",
        "compiled": True,
        "version": version,
        "deterministic": True,
        "compiler": {
            "name": "compile_skill_runtime.py",
            "version": COMPILER_VERSION,
        },
        "runtime_fingerprint": runtime_fingerprint,
        "entry": "SKILL.md",
        "load_order": LOAD_ORDER,
        "generated_files": sorted(runtime_files.keys()) + ["runtime/fallback/SKILL.full.md"] + subskill_generated_files,
        "source": {
            "root": "source",
            "skill_sha256": source_hash,
            "fallback_sha256": fallback_hash,
            "source_slimmed": _is_true(source_metadata.get("source_slimmed")),
            "source_fallback": source_metadata.get("source_fallback", ""),
            "file_count": len(source_checksums),
            "checksums": source_checksums,
        },
        "subskills": subskills,
        "extracts": {
            "gate_count": len(gates),
            "flow_scales": _ordered_scales(flows),
            "subskill_count": len(subskills),
        },
        "warnings": warnings,
    }
    _write_text(runtime_dir / "manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")

    # manifest-index.json: LLM-facing slim view of the manifest.
    # The full manifest.json retains all sha256/checksums/generated_files for
    # runtime_verify.py + UC-15 byte-idempotence. But the bootloader instructs
    # the agent to read the manifest on every load — the full 52KB file wastes
    # ~13k tokens of pure hash/path data the LLM never consumes. This slim index
    # keeps only what routing/loading needs (entry, load_order, subskill paths)
    # and is what the bootloader Load step now reads. Deterministic → UC-15 safe.
    manifest_index: dict[str, Any] = {
        "schema": "ae-sdd-runtime-index/v1",
        "compiled": True,
        "version": version,
        "runtime_fingerprint": runtime_fingerprint,
        "entry": "SKILL.md",
        "load_order": LOAD_ORDER,
        "subskills": [
            {
                "entry": rec.get("entry", ""),
                "manifest": rec.get("manifest", ""),
                "boot": rec.get("boot", ""),
                "outline": rec.get("outline", ""),
                "fallback": rec.get("fallback", ""),
            }
            for rec in subskills
        ],
        "extracts": {
            "gate_count": len(gates),
            "subskill_count": len(subskills),
        },
    }
    _write_text(
        runtime_dir / "manifest-index.json",
        json.dumps(manifest_index, ensure_ascii=False, indent=2) + "\n",
    )
    _write_text(dist_skill, render_bootloader(version, runtime_fingerprint))
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compile ae-sdd dist package into compact Agent runtime.",
    )
    parser.add_argument("--repo-root", type=Path, default=None, help="Repository root; default = script parent parent")
    parser.add_argument("--source", type=Path, default=None, help="Uncompiled source dir; default = <repo>/source")
    parser.add_argument("--dist", type=Path, default=None, help="Dist package dir; default = <repo>/dist/ae-sdd")
    parser.add_argument(
        "--build-date",
        default=None,
        help="Deprecated compatibility option; ignored because runtime output is deterministic",
    )
    args = parser.parse_args()

    repo_root = args.repo_root.resolve() if args.repo_root else Path(__file__).resolve().parent.parent
    source = args.source.resolve() if args.source else repo_root / "source"
    dist = args.dist.resolve() if args.dist else repo_root / "dist" / "ae-sdd"

    try:
        manifest = compile_runtime_package(repo_root, source, dist, build_date=args.build_date)
    except Exception as exc:
        print(f"[compile-runtime] ERROR: {exc}", file=sys.stderr)
        return 1
    print(
        f"[compile-runtime] ok version={manifest['version']} "
        f"gates={manifest['extracts']['gate_count']} "
        f"scales={','.join(manifest['extracts']['flow_scales'])}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
