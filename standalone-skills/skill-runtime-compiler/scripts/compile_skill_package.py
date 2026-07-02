#!/usr/bin/env python3
"""
compile_skill_package.py - compile a SKILL package into a compact runtime copy.

The source directory is preserved. By default the output is created next to the
source as <source-name>-compiled.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path
from typing import Any


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")


COMPILER_NAME = "skill-runtime-compiler"
COMPILER_SCRIPT = "compile_skill_package.py"
COMPILER_VERSION = "1"
MANIFEST_SCHEMA = "skill-runtime-compiler/v1"
LOAD_ORDER = [
    "runtime/boot.compact.md",
    "runtime/outline.compact.md",
]
GENERATED_FILES = [
    "runtime/boot.compact.md",
    "runtime/outline.compact.md",
    "runtime/fallback/SKILL.full.md",
    "runtime/manifest.json",
]
SKIP_DIRS = {".git", ".hg", ".svn", "__pycache__", ".pytest_cache", "node_modules"}


class CompileError(RuntimeError):
    """Raised when the source or output package is unsafe to compile."""


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _stable_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _yaml_scalar(value: str) -> str:
    if value and re.fullmatch(r"[A-Za-z0-9_.@/+ -]+", value) and not value.strip().startswith(("-", "{", "[")):
        return value
    return json.dumps(value, ensure_ascii=False)


def split_frontmatter(text: str) -> tuple[str, str]:
    if not text.startswith("---\n"):
        return "", text
    end = text.find("\n---", 4)
    if end < 0:
        return "", text
    after = end + len("\n---")
    if after < len(text) and text[after:after + 1] == "\n":
        after += 1
    return text[4:end], text[after:]


def parse_simple_frontmatter(text: str) -> dict[str, str]:
    frontmatter, _body = split_frontmatter(text)
    values: dict[str, str] = {}
    for line in frontmatter.splitlines():
        match = re.match(r"^([A-Za-z0-9_-]+):\s*(.*?)\s*$", line)
        if not match:
            continue
        raw = match.group(2).strip()
        if (raw.startswith('"') and raw.endswith('"')) or (raw.startswith("'") and raw.endswith("'")):
            raw = raw[1:-1]
        values[match.group(1)] = raw
    return values


def _is_true(value: str | None) -> bool:
    return str(value or "").strip().lower() in {"true", "yes", "1"}


def _title_from_body(body: str, fallback: str) -> str:
    for line in body.splitlines():
        match = re.match(r"^#\s+(.+?)\s*$", line)
        if match:
            return match.group(1).strip()
    return fallback


def _first_paragraph(body: str, limit: int = 600) -> str:
    lines: list[str] = []
    in_front_title = True
    for raw in body.splitlines():
        line = raw.strip()
        if in_front_title and (not line or line.startswith("#")):
            continue
        in_front_title = False
        if not line:
            if lines:
                break
            continue
        if line.startswith("```"):
            break
        lines.append(line)
    paragraph = " ".join(lines)
    if len(paragraph) <= limit:
        return paragraph
    return paragraph[: limit - 3].rstrip() + "..."


def extract_headings(body: str) -> list[dict[str, Any]]:
    headings: list[dict[str, Any]] = []
    for line_no, line in enumerate(body.splitlines(), start=1):
        match = re.match(r"^(#{1,6})\s+(.+?)\s*$", line)
        if not match:
            continue
        title = re.sub(r"\s+", " ", match.group(2)).strip()
        headings.append({"level": len(match.group(1)), "title": title, "line": line_no})
    return headings


def iter_source_files(source: Path) -> list[Path]:
    files: list[Path] = []
    for path in source.rglob("*"):
        rel_parts = path.relative_to(source).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        if path.is_file():
            files.append(path)
    return sorted(files, key=lambda p: p.relative_to(source).as_posix())


def collect_checksums(source: Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for path in iter_source_files(source):
        checksums[path.relative_to(source).as_posix()] = _sha256_bytes(path.read_bytes())
    return checksums


def resource_index(source: Path) -> dict[str, list[dict[str, Any]]]:
    index: dict[str, list[dict[str, Any]]] = {"scripts": [], "references": [], "assets": []}
    for section in index:
        root = source / section
        if not root.is_dir():
            continue
        for path in sorted((p for p in root.rglob("*") if p.is_file()), key=lambda p: p.as_posix()):
            rel = path.relative_to(source).as_posix()
            index[section].append({"path": rel, "bytes": path.stat().st_size, "sha256": _sha256_bytes(path.read_bytes())})
    return index


def _markdown_table(headers: list[str], rows: list[list[Any]]) -> str:
    def cell(value: Any) -> str:
        return str(value).replace("|", "\\|").replace("\n", " ").strip()

    lines = [
        "| " + " | ".join(cell(h) for h in headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(cell(v) for v in row) + " |")
    return "\n".join(lines)


def render_boot_compact(metadata: dict[str, str], runtime_fingerprint: str) -> str:
    load_order = "\n".join(f"{idx + 1}. `{path}`" for idx, path in enumerate(LOAD_ORDER))
    name = metadata.get("name", "unknown-skill")
    return f"""# Skill Runtime Boot Compact

- schema: {MANIFEST_SCHEMA}
- source_skill: {name}
- runtime_fingerprint: {runtime_fingerprint}
- deterministic: true

## Load Order

{load_order}

## Runtime Contract

- Use compact runtime slices first.
- Load `runtime/fallback/SKILL.full.md` only when compact slices do not contain enough detail.
- Treat the source package as the human-maintained master and this package as generated runtime output.
- Do not hand-edit `SKILL.md` or `runtime/**` inside the compiled package; re-run the compiler.
"""


def render_outline_compact(
    title: str,
    summary: str,
    headings: list[dict[str, Any]],
    resources: dict[str, list[dict[str, Any]]],
) -> str:
    heading_rows = [[h["level"], h["line"], h["title"]] for h in headings]
    resource_rows: list[list[Any]] = []
    for section in ("scripts", "references", "assets"):
        for item in resources[section]:
            resource_rows.append([section, item["path"], item["bytes"], item["sha256"][:12]])
    if not heading_rows:
        heading_rows.append(["", "", "(no markdown headings found)"])
    if not resource_rows:
        resource_rows.append(["", "(no bundled resources found)", "", ""])
    return (
        "# Skill Runtime Outline Compact\n\n"
        f"- title: {title}\n"
        f"- summary: {summary or '(none)'}\n"
        f"- heading_count: {len(headings)}\n"
        f"- resource_count: {sum(len(v) for v in resources.values())}\n\n"
        "## Headings\n\n"
        + _markdown_table(["level", "line", "title"], heading_rows)
        + "\n\n## Resources\n\n"
        + _markdown_table(["type", "path", "bytes", "sha256"], resource_rows)
        + "\n"
    )


def render_bootloader(metadata: dict[str, str], title: str, runtime_fingerprint: str) -> str:
    name = metadata.get("name") or "compiled-skill"
    description = metadata.get("description") or "Compiled SKILL runtime package; load compact runtime slices first and fallback only when needed."
    version = metadata.get("version")
    frontmatter = [
        "---",
        f"name: {_yaml_scalar(name)}",
        f"description: {_yaml_scalar(description)}",
    ]
    if version:
        frontmatter.append(f"version: {_yaml_scalar(version)}")
    frontmatter.extend(["compiled: true", "runtime: runtime/manifest.json", "---"])
    return "\n".join(frontmatter) + f"""

# {title} Compiled Runtime Entry

This is a generated runtime package. Do not treat it as the human-maintained source.

## Load

1. Read `runtime/manifest.json`.
2. Read `runtime/boot.compact.md`.
3. Read `runtime/outline.compact.md`.
4. Load `runtime/fallback/SKILL.full.md` only when compact runtime slices are insufficient.

## Authority

- Human-maintained source: sibling source package used as compiler input.
- Compiled runtime package: this directory.
- Full fallback source: `runtime/fallback/SKILL.full.md`.

runtime_fingerprint: {runtime_fingerprint}
"""


def _safe_remove_output(output: Path, force: bool) -> None:
    if not output.exists():
        return
    manifest = output / "runtime" / "manifest.json"
    generated = False
    if manifest.is_file():
        try:
            data = json.loads(manifest.read_text(encoding="utf-8"))
            generated = (
                data.get("schema") == MANIFEST_SCHEMA
                and data.get("compiled") is True
                and data.get("compiler", {}).get("name") == COMPILER_SCRIPT
            )
        except Exception:
            generated = False
    if not force and not generated:
        raise CompileError(f"output exists and is not a generated package; use --force: {output}")
    shutil.rmtree(output)


def _copy_source(source: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    for path in iter_source_files(source):
        rel = path.relative_to(source)
        target = output / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)


def _assert_safe_paths(source: Path, output: Path) -> None:
    source = source.resolve()
    output = output.resolve()
    if output == source:
        raise CompileError("output path must not equal source path")
    if source in output.parents:
        raise CompileError("output path must not be inside the source package")
    if not source.is_dir():
        raise CompileError(f"source is not a directory: {source}")
    if not (source / "SKILL.md").is_file():
        raise CompileError(f"source SKILL.md not found: {source / 'SKILL.md'}")


def compile_skill_package(source: Path, output: Path | None = None, force: bool = False) -> dict[str, Any]:
    source = source.resolve()
    output = output.resolve() if output else source.parent / f"{source.name}-compiled"
    _assert_safe_paths(source, output)

    source_skill = source / "SKILL.md"
    source_text = source_skill.read_text(encoding="utf-8")
    metadata = parse_simple_frontmatter(source_text)
    if _is_true(metadata.get("compiled")):
        raise CompileError("source SKILL.md already declares compiled: true; choose the uncompiled master package")

    checksums = collect_checksums(source)
    frontmatter, body = split_frontmatter(source_text)
    del frontmatter
    title = _title_from_body(body, metadata.get("name", source.name))
    summary = _first_paragraph(body)
    headings = extract_headings(body)
    resources = resource_index(source)

    fingerprint_payload = {
        "compiler": {"name": COMPILER_SCRIPT, "version": COMPILER_VERSION},
        "schema": MANIFEST_SCHEMA,
        "load_order": LOAD_ORDER,
        "metadata": metadata,
        "source_checksums": checksums,
        "title": title,
        "summary": summary,
        "headings": headings,
        "resources": resources,
    }
    runtime_fingerprint = _sha256_text(_stable_json(fingerprint_payload))

    _safe_remove_output(output, force=force)
    _copy_source(source, output)

    fallback_path = output / "runtime" / "fallback" / "SKILL.full.md"
    _write_text(fallback_path, source_text)
    runtime_files = {
        "runtime/boot.compact.md": render_boot_compact(metadata, runtime_fingerprint),
        "runtime/outline.compact.md": render_outline_compact(title, summary, headings, resources),
    }
    for rel, content in runtime_files.items():
        _write_text(output / rel, content)

    manifest: dict[str, Any] = {
        "schema": MANIFEST_SCHEMA,
        "compiled": True,
        "deterministic": True,
        "compiler": {"name": COMPILER_SCRIPT, "version": COMPILER_VERSION, "skill": COMPILER_NAME},
        "runtime_fingerprint": runtime_fingerprint,
        "entry": "SKILL.md",
        "load_order": LOAD_ORDER,
        "generated_files": GENERATED_FILES,
        "source": {
            "package_name": metadata.get("name", source.name),
            "directory_name": source.name,
            "skill_sha256": checksums.get("SKILL.md", _sha256_bytes(source_skill.read_bytes())),
            "file_count": len(checksums),
            "checksums": checksums,
        },
        "extracts": {
            "title": title,
            "heading_count": len(headings),
            "resource_count": sum(len(v) for v in resources.values()),
            "script_count": len(resources["scripts"]),
            "reference_count": len(resources["references"]),
            "asset_count": len(resources["assets"]),
        },
        "warnings": [],
    }
    _write_text(output / "runtime" / "manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    _write_text(output / "SKILL.md", render_bootloader(metadata, title, runtime_fingerprint))
    return manifest | {"package_path": str(output)}


def main() -> int:
    parser = argparse.ArgumentParser(description="Compile a SKILL package into a same-parent compact runtime package.")
    parser.add_argument("source", type=Path, help="Source SKILL directory containing SKILL.md")
    parser.add_argument("--output", type=Path, default=None, help="Output package directory; default is <source>-compiled")
    parser.add_argument("--force", action="store_true", help="Allow replacing an unrelated existing output directory")
    parser.add_argument("--json", action="store_true", help="Print the manifest/result as JSON")
    args = parser.parse_args()

    try:
        result = compile_skill_package(args.source, output=args.output, force=args.force)
    except Exception as exc:
        print(f"[skill-runtime-compiler] ERROR: {exc}", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print(
            "[skill-runtime-compiler] ok "
            f"path={result['package_path']} "
            f"fingerprint={result['runtime_fingerprint']} "
            f"headings={result['extracts']['heading_count']} "
            f"resources={result['extracts']['resource_count']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
