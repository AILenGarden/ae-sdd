"""
runtime_verify.py - compiled runtime package verifier.

This module validates the generated dist/installed ae-sdd runtime package. It is
intentionally read-only and does not rebuild or modify the package.
"""
from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


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

    return RuntimeVerifyResult(str(package), not issues, issues, warnings, manifest)
