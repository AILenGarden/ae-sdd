#!/usr/bin/env python3
"""Validate ALSpec registration metadata without parsing specification bodies."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any

import yaml

SCHEMA_VERSION = 2
SPEC_TYPES = {"dr", "story", "testcase"}
SCOPES = {"global", "project"}
ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
URI_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
TOP_LEVEL_FIELDS = {"schema_version", "description", "specs"}
ENTRY_FIELDS = {"id", "name", "type", "scope", "project_id", "path", "enabled", "description", "tags", "metadata"}


class Validation:
    def __init__(self, root: Path) -> None:
        self.root = root.resolve()
        self.errors: list[str] = []
        self.warnings: list[str] = []

    def error(self, message: str) -> None:
        self.errors.append(message)

    def warning(self, message: str) -> None:
        self.warnings.append(message)


def is_reference(raw_path: str) -> bool:
    if re.match(r"^[A-Za-z]:[\\/]", raw_path) or raw_path.startswith("\\\\"):
        return False
    return bool(URI_RE.match(raw_path))


def resolve_path(root: Path, raw_path: str) -> Path:
    path = Path(raw_path).expanduser()
    return path if path.is_absolute() else root / path


def load_registry(root: Path, validation: Validation) -> dict[str, Any] | None:
    path = root / "framework" / "registry.yaml"
    try:
        value = yaml.safe_load(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, yaml.YAMLError) as exc:
        validation.error(f"framework/registry.yaml: cannot parse YAML: {exc}")
        return None
    if not isinstance(value, dict):
        validation.error("framework/registry.yaml: top-level value must be a mapping")
        return None
    return value


def validate_entry(entry: dict[str, Any], prefix: str, validation: Validation) -> None:
    for field in sorted(set(entry) - ENTRY_FIELDS):
        validation.error(f"{prefix}: unknown field {field!r}")
    for field in ("id", "name", "type", "scope", "path", "enabled"):
        if field not in entry:
            validation.error(f"{prefix}: missing required field {field!r}")
    spec_id = entry.get("id")
    if not isinstance(spec_id, str) or not ID_RE.fullmatch(spec_id):
        validation.error(f"{prefix}: id must be lowercase kebab-case")
    name = entry.get("name")
    if not isinstance(name, str) or not name.strip():
        validation.error(f"{prefix}: name must be a non-empty string")
    if entry.get("type") not in SPEC_TYPES:
        validation.error(f"{prefix}: type must be one of {sorted(SPEC_TYPES)}")
    scope = entry.get("scope")
    if scope not in SCOPES:
        validation.error(f"{prefix}: scope must be one of {sorted(SCOPES)}")
    project_id = entry.get("project_id")
    if scope == "project":
        if not isinstance(project_id, str) or not ID_RE.fullmatch(project_id):
            validation.error(f"{prefix}: project_id must be lowercase kebab-case when scope is project")
    elif project_id is not None:
        validation.error(f"{prefix}: project_id is only allowed when scope is project")
    path = entry.get("path")
    if not isinstance(path, str) or not path.strip():
        validation.error(f"{prefix}: path must be a non-empty string")
    if not isinstance(entry.get("enabled"), bool):
        validation.error(f"{prefix}: enabled must be boolean")
    if "description" in entry and not isinstance(entry["description"], str):
        validation.error(f"{prefix}: description must be a string")
    if "tags" in entry and (
        not isinstance(entry["tags"], list)
        or not all(isinstance(tag, str) and tag.strip() for tag in entry["tags"])
    ):
        validation.error(f"{prefix}: tags must be a list of non-empty strings")
    if "metadata" in entry and not isinstance(entry["metadata"], dict):
        validation.error(f"{prefix}: metadata must be a mapping")


def validate_registry_data(registry: dict[str, Any], root: Path, validation: Validation) -> None:
    for field in sorted(set(registry) - TOP_LEVEL_FIELDS):
        validation.error(f"framework/registry.yaml: unknown top-level field {field!r}")
    if registry.get("schema_version") != SCHEMA_VERSION:
        validation.error(f"framework/registry.yaml: schema_version must be {SCHEMA_VERSION}")
    if "description" in registry and not isinstance(registry["description"], str):
        validation.error("framework/registry.yaml: description must be a string")
    entries = registry.get("specs")
    if not isinstance(entries, list):
        validation.error("framework/registry.yaml: specs must be a list")
        return
    ids: dict[str, int] = {}
    names: dict[str, int] = {}
    for index, entry in enumerate(entries, start=1):
        prefix = f"framework/registry.yaml: specs[{index}]"
        if not isinstance(entry, dict):
            validation.error(f"{prefix} must be a mapping")
            continue
        validate_entry(entry, prefix, validation)
        spec_id = entry.get("id")
        if isinstance(spec_id, str) and ID_RE.fullmatch(spec_id):
            key = spec_id.casefold()
            if key in ids:
                validation.error(f"{prefix}: duplicate id {spec_id!r}; first occurrence is specs[{ids[key]}]")
            ids[key] = index
        name = entry.get("name")
        if isinstance(name, str) and name.strip():
            key = name.strip().casefold()
            if key in names:
                validation.error(f"{prefix}: duplicate name {name!r}; first occurrence is specs[{names[key]}]")
            names[key] = index
        raw_path = entry.get("path")
        if isinstance(raw_path, str) and raw_path.strip() and not is_reference(raw_path.strip()):
            if not resolve_path(root, raw_path.strip()).is_file():
                validation.warning(f"{prefix}: referenced specification is not available: {raw_path}")


def validate_registry(root: Path, validation: Validation) -> dict[str, Any] | None:
    registry = load_registry(root, validation)
    if registry is not None:
        validate_registry_data(registry, root, validation)
    return registry


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate the ALSpec registry")
    parser.add_argument("root", nargs="?", default=".", help="ALSpec skill root")
    args = parser.parse_args(argv)
    validation = Validation(Path(args.root))
    validate_registry(validation.root, validation)
    for warning in validation.warnings:
        print(f"WARN: {warning}")
    for error in validation.errors:
        print(f"ERROR: {error}")
    print(f"validated: errors={len(validation.errors)} warnings={len(validation.warnings)}")
    return 1 if validation.errors else 0


if __name__ == "__main__":
    sys.exit(main())
