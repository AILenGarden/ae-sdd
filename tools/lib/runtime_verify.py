"""
runtime_verify.py - compiled runtime package verifier.

This module validates the generated dist/installed ae-sdd runtime package. It is
intentionally read-only and does not rebuild or modify the package.
"""
from __future__ import annotations

import json
import hashlib
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_SUBSKILL_SCHEMA = "ae-sdd-subskill-runtime/v1"

# Anchor regex aligned with compile_skill_runtime.py:_is_core_structural (the
# single source of truth for which tokens are "structural anchors" that a
# core.compact.md fast-path must not silently drop via max_lines truncation).
_CORE_ANCHOR_RE = re.compile(r"\b(G-[A-Z0-9-]+|RA-G\d+|TR-\d+|TC-\d+|TV-\d+)\b")


def _extract_anchors(text: str) -> set[str]:
    """Return the set of structural anchor tokens present in *text*."""
    return set(_CORE_ANCHOR_RE.findall(text))


@dataclass
class RuntimeVerifyResult:
    package_path: str
    ok: bool
    issues: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    manifest: dict[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "package_path": self.package_path,
            "ok": self.ok,
            "issues": self.issues,
            "warnings": self.warnings,
            "manifest": self.manifest,
        }


def _read_frontmatter(text: str) -> dict[str, str]:
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    data: dict[str, str] = {}
    for line in text[3:end].splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        data[key.strip()] = value.strip().strip('"').strip("'")
    return data


def _is_sha256(value: Any) -> bool:
    return isinstance(value, str) and bool(_SHA256_RE.fullmatch(value))


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def verify_runtime_package(package_path: str | Path) -> RuntimeVerifyResult:
    """Verify a compiled ae-sdd runtime package.

    The verifier checks package shape, bootloader metadata, manifest integrity,
    load-order files, generated files, fallback preservation, and stable hash
    fields. It does not compare against the current source tree; update-check
    handles compile idempotency separately.
    """
    package = Path(package_path).expanduser().resolve()
    issues: list[str] = []
    warnings: list[str] = []
    manifest: dict[str, Any] | None = None

    def issue(message: str) -> None:
        issues.append(message)

    def warn(message: str) -> None:
        warnings.append(message)

    if not package.is_dir():
        issue(f"package directory does not exist: {package}")
        return RuntimeVerifyResult(str(package), False, issues, warnings, manifest)

    skill_md = package / "SKILL.md"
    if not skill_md.is_file():
        issue("SKILL.md is missing")
        return RuntimeVerifyResult(str(package), False, issues, warnings, manifest)

    try:
        skill_text = skill_md.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        issue(f"SKILL.md is unreadable: {exc}")
        return RuntimeVerifyResult(str(package), False, issues, warnings, manifest)

    frontmatter = _read_frontmatter(skill_text)
    if frontmatter.get("compiled", "").lower() != "true":
        issue("SKILL.md frontmatter must contain compiled: true")
    if frontmatter.get("runtime") != "runtime/manifest.json":
        issue("SKILL.md frontmatter must contain runtime: runtime/manifest.json")
    skill_version = frontmatter.get("version")
    if not skill_version:
        issue("SKILL.md frontmatter version is missing")

    manifest_path = package / "runtime" / "manifest.json"
    if not manifest_path.is_file():
        issue("runtime/manifest.json is missing")
        return RuntimeVerifyResult(str(package), False, issues, warnings, manifest)

    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        issue(f"runtime/manifest.json is unreadable or invalid JSON: {exc}")
        return RuntimeVerifyResult(str(package), False, issues, warnings, manifest)

    if manifest.get("schema") != "ae-sdd-runtime/v1":
        issue("manifest.schema must be ae-sdd-runtime/v1")
    if manifest.get("compiled") is not True:
        issue("manifest.compiled must be true")
    if manifest.get("deterministic") is not True:
        issue("manifest.deterministic must be true")

    manifest_version = manifest.get("version")
    if skill_version and manifest_version and skill_version != manifest_version:
        issue(f"version mismatch: SKILL.md={skill_version} manifest={manifest_version}")
    if not manifest_version:
        issue("manifest.version is missing")

    compiler = manifest.get("compiler")
    if not isinstance(compiler, dict) or not compiler.get("name") or not compiler.get("version"):
        issue("manifest.compiler.name/version are required")

    runtime_fingerprint = manifest.get("runtime_fingerprint")
    if not _is_sha256(runtime_fingerprint):
        issue("manifest.runtime_fingerprint must be a 64-char lowercase sha256")
    skill_fp = re.search(r"^runtime_fingerprint:\s*([0-9a-f]{64})\s*$", skill_text, re.MULTILINE)
    if runtime_fingerprint and skill_fp and skill_fp.group(1) != runtime_fingerprint:
        issue("SKILL.md runtime_fingerprint differs from manifest.runtime_fingerprint")

    entry = manifest.get("entry")
    if entry != "SKILL.md":
        issue("manifest.entry must be SKILL.md")

    load_order = manifest.get("load_order")
    if not isinstance(load_order, list) or not load_order:
        issue("manifest.load_order must be a non-empty list")
    else:
        for rel in load_order:
            if not isinstance(rel, str):
                issue(f"manifest.load_order contains non-string item: {rel!r}")
                continue
            if not (package / rel).is_file():
                issue(f"load_order file missing: {rel}")

    generated_files = manifest.get("generated_files")
    if not isinstance(generated_files, list) or not generated_files:
        issue("manifest.generated_files must be a non-empty list")
    else:
        for rel in generated_files:
            if not isinstance(rel, str):
                issue(f"manifest.generated_files contains non-string item: {rel!r}")
                continue
            if not (package / rel).is_file():
                issue(f"generated file missing: {rel}")

    source = manifest.get("source")
    if not isinstance(source, dict):
        issue("manifest.source must be an object")
    else:
        if not _is_sha256(source.get("skill_sha256")):
            issue("manifest.source.skill_sha256 must be a sha256")
        if not _is_sha256(source.get("fallback_sha256")):
            issue("manifest.source.fallback_sha256 must be a sha256")
        checksums = source.get("checksums")
        if not isinstance(checksums, dict) or not checksums:
            issue("manifest.source.checksums must be a non-empty object")

    extracts = manifest.get("extracts")
    if not isinstance(extracts, dict):
        issue("manifest.extracts must be an object")
    else:
        gate_count = extracts.get("gate_count")
        if not isinstance(gate_count, int) or gate_count <= 0:
            issue("manifest.extracts.gate_count must be a positive integer")
        flow_scales = extracts.get("flow_scales")
        if not isinstance(flow_scales, list) or not {"大", "中", "小", "微"}.issubset(set(flow_scales)):
            issue("manifest.extracts.flow_scales must include 大/中/小/微")

    fallback = package / "runtime" / "fallback" / "SKILL.full.md"
    if not fallback.is_file():
        issue("runtime/fallback/SKILL.full.md is missing")
    else:
        try:
            fallback_text = fallback.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            issue(f"fallback SKILL.full.md is unreadable: {exc}")
        else:
            if "# ae-sdd Compiled Runtime Entry" in fallback_text and "compiled: true" in fallback_text[:400]:
                issue("fallback SKILL.full.md appears to contain the generated bootloader")
            if len(fallback_text.strip()) < 100:
                warn("fallback SKILL.full.md is unexpectedly short")

    source_checksums = {}
    if isinstance(source, dict) and isinstance(source.get("checksums"), dict):
        source_checksums = source["checksums"]
    source_skill_entries = sorted(
        rel
        for rel in source_checksums
        if isinstance(rel, str) and rel.startswith("skills/") and rel.endswith(".md")
    )

    subskills = manifest.get("subskills") if manifest else None
    if source_skill_entries:
        if "runtime/subskills.compact.md" not in (load_order or []):
            issue("manifest.load_order must include runtime/subskills.compact.md when child SKILL files exist")
        if not (package / "runtime" / "subskills.compact.md").is_file():
            issue("runtime/subskills.compact.md is missing")
        if not isinstance(subskills, list) or not subskills:
            issue("manifest.subskills must list compiled child SKILL entries")
            subskills = []

        if isinstance(extracts, dict) and extracts.get("subskill_count") != len(source_skill_entries):
            issue(
                "manifest.extracts.subskill_count must equal source child SKILL count: "
                f"{extracts.get('subskill_count')} != {len(source_skill_entries)}"
            )

        by_entry: dict[str, dict[str, Any]] = {}
        for item in subskills:
            if not isinstance(item, dict):
                issue(f"manifest.subskills contains non-object item: {item!r}")
                continue
            entry_name = item.get("entry")
            if not isinstance(entry_name, str):
                issue(f"manifest.subskills item missing entry: {item!r}")
                continue
            by_entry[entry_name] = item

        missing = [entry for entry in source_skill_entries if entry not in by_entry]
        if missing:
            issue(f"manifest.subskills missing entries for source child SKILL files: {missing[:5]}")

        for entry in source_skill_entries:
            record = by_entry.get(entry)
            if not record:
                continue

            entry_path = package / entry
            if not entry_path.is_file():
                issue(f"compiled child SKILL entry missing: {entry}")
                continue
            try:
                entry_text = entry_path.read_text(encoding="utf-8", errors="replace")
            except OSError as exc:
                issue(f"compiled child SKILL entry unreadable: {entry}: {exc}")
                continue

            entry_frontmatter = _read_frontmatter(entry_text)
            manifest_rel = record.get("manifest")
            if entry_frontmatter.get("compiled", "").lower() != "true":
                issue(f"child SKILL entry must be compiled: true: {entry}")
            if entry_frontmatter.get("runtime") != manifest_rel:
                issue(f"child SKILL entry runtime mismatch: {entry}")
            if "# " in entry_text and "Compiled Sub-SKILL Entry" not in entry_text[:800]:
                issue(f"child SKILL entry appears to contain uncompiled source: {entry}")

            for key in ("manifest", "boot", "outline", "core", "fallback"):
                rel = record.get(key)
                if not isinstance(rel, str):
                    issue(f"manifest.subskills[{entry}].{key} must be a path string")
                    continue
                if not (package / rel).is_file():
                    issue(f"child SKILL generated file missing: {rel}")

            child_manifest = None
            if isinstance(manifest_rel, str) and (package / manifest_rel).is_file():
                try:
                    child_manifest = json.loads((package / manifest_rel).read_text(encoding="utf-8"))
                except (OSError, json.JSONDecodeError) as exc:
                    issue(f"child SKILL manifest invalid: {manifest_rel}: {exc}")

            if child_manifest is not None:
                if child_manifest.get("schema") != _SUBSKILL_SCHEMA:
                    issue(f"child SKILL manifest schema mismatch: {manifest_rel}")
                if child_manifest.get("compiled") is not True:
                    issue(f"child SKILL manifest compiled must be true: {manifest_rel}")
                if child_manifest.get("deterministic") is not True:
                    issue(f"child SKILL manifest deterministic must be true: {manifest_rel}")
                if child_manifest.get("entry") != entry:
                    issue(f"child SKILL manifest entry mismatch: {manifest_rel}")
                child_fp = child_manifest.get("runtime_fingerprint")
                if not _is_sha256(child_fp):
                    issue(f"child SKILL manifest runtime_fingerprint must be sha256: {manifest_rel}")
                if record.get("runtime_fingerprint") != child_fp:
                    issue(f"manifest.subskills fingerprint differs from child manifest: {entry}")
                for rel in child_manifest.get("load_order", []):
                    if not isinstance(rel, str) or not (package / rel).is_file():
                        issue(f"child SKILL load_order file missing: {rel!r}")

            fallback_rel = record.get("fallback")
            child_fallback_text = ""
            if isinstance(fallback_rel, str) and (package / fallback_rel).is_file():
                try:
                    child_fallback_text = (package / fallback_rel).read_text(encoding="utf-8", errors="replace")
                except OSError as exc:
                    issue(f"child SKILL fallback unreadable: {fallback_rel}: {exc}")
                else:
                    fallback_hash = _sha256_text(child_fallback_text)
                    expected_fallback_hash = record.get("fallback_sha256")
                    if expected_fallback_hash and expected_fallback_hash != fallback_hash:
                        issue(f"child SKILL fallback hash mismatch: {entry}")
                    if "Compiled Sub-SKILL Entry" in child_fallback_text[:800] and "compiled: true" in child_fallback_text[:400]:
                        issue(f"child SKILL fallback appears to contain generated entry: {fallback_rel}")

            # core.compact.md: existence + sha256 (issue) + anchor coverage (warning).
            # The core is the executable fast-path extracted by render_core_compact
            # via sectionize/score/max_lines truncation; anchors dropped by that
            # truncation are surfaced here as warnings so the loss stays visible.
            core_rel = record.get("core")
            if isinstance(core_rel, str):
                core_path = package / core_rel
                if not core_path.is_file():
                    issue(f"child SKILL core.compact.md missing: {core_rel}")
                else:
                    try:
                        core_text = core_path.read_text(encoding="utf-8", errors="replace")
                    except OSError as exc:
                        issue(f"child SKILL core unreadable: {core_rel}: {exc}")
                    else:
                        expected_core_hash = record.get("core_sha256")
                        if expected_core_hash:
                            actual_core_hash = _sha256_text(core_text)
                            if expected_core_hash != actual_core_hash:
                                issue(f"child SKILL core hash mismatch: {entry}")
                        if child_fallback_text:
                            fb_anchors = _extract_anchors(child_fallback_text)
                            core_anchors = _extract_anchors(core_text)
                            lost = fb_anchors - core_anchors
                            if lost:
                                preview = sorted(lost)[:10]
                                suffix = " ..." if len(lost) > 10 else ""
                                warn(
                                    f"core lost {len(lost)} anchors vs fallback: "
                                    f"{entry}: {preview}{suffix}"
                                )

    return RuntimeVerifyResult(str(package), not issues, issues, warnings, manifest)
