#!/usr/bin/env python3
"""Register and index author-owned ALSpec references."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path
from typing import Any

import yaml

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from validate_registry import (  # noqa: E402
    ID_RE,
    SCOPES,
    SPEC_TYPES,
    Validation,
    is_reference,
    resolve_path,
    validate_registry,
    validate_registry_data,
)

SPEC_TYPE_CHOICES = tuple(sorted(SPEC_TYPES))
SCOPE_CHOICES = tuple(sorted(SCOPES))
COMMANDS = {"list", "ls", "register", "add", "update", "remove", "unregister", "enable", "disable", "show"}


class RegistryError(RuntimeError):
    """Raised when the registry cannot be safely read or changed."""


def registry_path(root: str | Path) -> Path:
    return Path(root).resolve() / "framework" / "registry.yaml"


def load_registry(root: str | Path) -> dict[str, Any]:
    root_path = Path(root).resolve()
    validation = Validation(root_path)
    registry = validate_registry(root_path, validation)
    if validation.errors or registry is None:
        raise RegistryError("; ".join(validation.errors) or "framework/registry.yaml could not be loaded")
    return registry


def _identity(value: str) -> str:
    return value.strip().casefold()


def _find_entry(entries: list[dict[str, Any]], spec_id: str) -> dict[str, Any]:
    key = _identity(spec_id)
    for entry in entries:
        if _identity(str(entry.get("id", ""))) == key:
            return entry
    raise RegistryError(f"specification id not found: {spec_id}")


def _check_identity(entries: list[dict[str, Any]], spec_id: str, name: str,
                    current: dict[str, Any] | None = None) -> None:
    if not ID_RE.fullmatch(spec_id):
        raise RegistryError("id must be lowercase kebab-case")
    if not name.strip():
        raise RegistryError("name must be non-empty")
    for entry in entries:
        if entry is current:
            continue
        if _identity(str(entry.get("id", ""))) == _identity(spec_id):
            raise RegistryError(f"duplicate specification id: {spec_id}")
        if _identity(str(entry.get("name", ""))) == _identity(name):
            raise RegistryError(f"duplicate specification name: {name}")


def _normalize_scope(scope: str | None, project_id: str | None) -> tuple[str, str | None]:
    normalized_scope = (scope or "global").strip().lower()
    if normalized_scope not in SCOPES:
        raise RegistryError(f"scope must be one of {list(SCOPE_CHOICES)}")
    normalized_project_id = project_id.strip().lower() if isinstance(project_id, str) else None
    if normalized_scope == "project":
        if not normalized_project_id or not ID_RE.fullmatch(normalized_project_id):
            raise RegistryError("project_id must be lowercase kebab-case when scope is project")
        return normalized_scope, normalized_project_id
    if normalized_project_id:
        raise RegistryError("project_id is only allowed when scope is project")
    return normalized_scope, None


def _validate_before_write(root: Path, registry: dict[str, Any]) -> None:
    validation = Validation(root)
    validate_registry_data(registry, root, validation)
    if validation.errors:
        raise RegistryError("; ".join(validation.errors))


def _write_registry(root: Path, registry: dict[str, Any]) -> None:
    target = registry_path(root)
    target.parent.mkdir(parents=True, exist_ok=True)
    payload = yaml.safe_dump(registry, allow_unicode=True, sort_keys=False, default_flow_style=False)
    fd, temp_name = tempfile.mkstemp(prefix=f".{target.name}.", suffix=".tmp", dir=target.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_name, target)
    except OSError as exc:
        try:
            os.unlink(temp_name)
        except OSError:
            pass
        raise RegistryError(f"cannot atomically write {target}: {exc}") from exc


def register_spec(root: str | Path, *, spec_id: str, name: str, spec_type: str,
                  path: str, enabled: bool = True, scope: str = "global",
                  project_id: str | None = None, description: str | None = None,
                  tags: list[str] | None = None,
                  metadata: dict[str, Any] | None = None) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entries = registry["specs"]
    spec_id, name, path = spec_id.strip(), name.strip(), path.strip()
    _check_identity(entries, spec_id, name)
    if spec_type not in SPEC_TYPES:
        raise RegistryError(f"type must be one of {list(SPEC_TYPE_CHOICES)}")
    normalized_scope, normalized_project_id = _normalize_scope(scope, project_id)
    if not path:
        raise RegistryError("path must be non-empty")
    entry: dict[str, Any] = {
        "id": spec_id,
        "name": name,
        "type": spec_type,
        "scope": normalized_scope,
        "path": path,
        "enabled": enabled,
    }
    if normalized_project_id is not None:
        entry["project_id"] = normalized_project_id
    if description is not None:
        entry["description"] = description
    if tags:
        entry["tags"] = tags
    if metadata is not None:
        entry["metadata"] = metadata
    entries.append(entry)
    _validate_before_write(root_path, registry)
    _write_registry(root_path, registry)
    return entry


def update_spec(root: str | Path, spec_id: str, changes: dict[str, Any]) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entries = registry["specs"]
    entry = _find_entry(entries, spec_id)
    candidate_id = str(changes.get("id", entry["id"])).strip()
    candidate_name = str(changes.get("name", entry["name"])).strip()
    _check_identity(entries, candidate_id, candidate_name, current=entry)
    requested_scope = changes.get("scope", entry["scope"])
    requested_project_id = changes.get("project_id", entry.get("project_id"))
    if "scope" in changes and str(requested_scope).strip().lower() == "global" and "project_id" not in changes:
        requested_project_id = None
    candidate_scope, candidate_project_id = _normalize_scope(requested_scope, requested_project_id)
    if "id" in changes:
        entry["id"] = candidate_id
    if "name" in changes:
        entry["name"] = candidate_name
    if "type" in changes:
        if changes["type"] not in SPEC_TYPES:
            raise RegistryError(f"type must be one of {list(SPEC_TYPE_CHOICES)}")
        entry["type"] = changes["type"]
    if "scope" in changes or "project_id" in changes:
        entry["scope"] = candidate_scope
        if candidate_project_id is None:
            entry.pop("project_id", None)
        else:
            entry["project_id"] = candidate_project_id
    if "path" in changes:
        if not isinstance(changes["path"], str) or not changes["path"].strip():
            raise RegistryError("path must be non-empty")
        entry["path"] = changes["path"].strip()
    for key in ("description", "tags", "metadata", "enabled"):
        if key in changes:
            entry[key] = changes[key]
    _validate_before_write(root_path, registry)
    _write_registry(root_path, registry)
    return entry


def remove_spec(root: str | Path, spec_id: str) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entry = _find_entry(registry["specs"], spec_id)
    registry["specs"].remove(entry)
    _validate_before_write(root_path, registry)
    _write_registry(root_path, registry)
    return entry


def reference_status(root: Path, raw_path: str) -> tuple[str, bool | None]:
    if is_reference(raw_path):
        return "reference", None
    return "file", resolve_path(root, raw_path).is_file()


def build_index(root: str | Path, include_disabled: bool = False,
                project_id: str | None = None) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    selected_project = project_id.strip().lower() if isinstance(project_id, str) and project_id.strip() else None
    if selected_project is not None and not ID_RE.fullmatch(selected_project):
        raise RegistryError("project_id must be lowercase kebab-case")
    categories: dict[str, list[dict[str, Any]]] = {key: [] for key in SPEC_TYPE_CHOICES}
    for entry in registry["specs"]:
        if not include_disabled and not entry["enabled"]:
            continue
        scope = entry["scope"]
        if scope == "project" and (selected_project is None or entry.get("project_id") != selected_project):
            continue
        kind, available = reference_status(root_path, entry["path"])
        item = {
            "id": entry["id"],
            "name": entry["name"],
            "type": entry["type"],
            "scope": scope,
            "path": entry["path"],
            "enabled": entry["enabled"],
            "reference_kind": kind,
            "available": available,
        }
        if "project_id" in entry:
            item["project_id"] = entry["project_id"]
        for key in ("description", "tags", "metadata"):
            if key in entry:
                item[key] = entry[key]
        categories[entry["type"]].append(item)
    return {"schema_version": registry["schema_version"], "project_id": selected_project, "categories": categories}


def _print_index(index: dict[str, Any], as_json: bool) -> None:
    if as_json:
        print(json.dumps(index, ensure_ascii=False, indent=2))
        return
    for spec_type in SPEC_TYPE_CHOICES:
        print(f"[{spec_type}]")
        for entry in index["categories"][spec_type]:
            state = "enabled" if entry["enabled"] else "disabled"
            availability = entry["reference_kind"] if entry["reference_kind"] == "reference" else ("available" if entry["available"] else "missing")
            project = f"/{entry['project_id']}" if entry.get("project_id") else ""
            print(f"- {entry['id']} | {entry['name']} | {entry['scope']}{project} | {state} | {availability} | {entry['path']}")


def _metadata_from_json(raw: str | None) -> dict[str, Any] | None:
    if raw is None:
        return None
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise RegistryError(f"metadata must be valid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise RegistryError("metadata must be a JSON object")
    return value


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Register and index ALSpec references")
    parser.add_argument("--root", dest="global_root", default=".", help="ALSpec skill root")
    subparsers = parser.add_subparsers(dest="command")

    list_parser = subparsers.add_parser("list", aliases=["ls"], help="list registered specifications")
    list_parser.add_argument("--root", default=None)
    list_parser.add_argument("--include-disabled", action="store_true")
    list_parser.add_argument("--project-id")
    list_parser.add_argument("--json", action="store_true", dest="as_json")

    register_parser = subparsers.add_parser("register", aliases=["add"], help="register a specification")
    register_parser.add_argument("--root", default=None)
    register_parser.add_argument("--id", required=True)
    register_parser.add_argument("--name", required=True)
    register_parser.add_argument("--type", required=True, choices=SPEC_TYPE_CHOICES)
    register_parser.add_argument("--scope", choices=SCOPE_CHOICES, default="global")
    register_parser.add_argument("--project-id")
    register_parser.add_argument("--path", required=True)
    register_parser.add_argument("--description")
    register_parser.add_argument("--tag", action="append", dest="tags")
    register_parser.add_argument("--metadata-json")
    state = register_parser.add_mutually_exclusive_group()
    state.add_argument("--enabled", action="store_true", dest="enabled")
    state.add_argument("--disabled", action="store_false", dest="enabled")
    register_parser.set_defaults(enabled=True)

    update_parser = subparsers.add_parser("update", help="update a registered specification")
    update_parser.add_argument("spec_id")
    update_parser.add_argument("--root", default=None)
    update_parser.add_argument("--id")
    update_parser.add_argument("--name")
    update_parser.add_argument("--type", choices=SPEC_TYPE_CHOICES)
    update_parser.add_argument("--scope", choices=SCOPE_CHOICES)
    update_parser.add_argument("--project-id")
    update_parser.add_argument("--path")
    update_parser.add_argument("--description")
    update_parser.add_argument("--tag", action="append", dest="tags")
    update_parser.add_argument("--clear-tags", action="store_true")
    update_parser.add_argument("--metadata-json")
    update_state = update_parser.add_mutually_exclusive_group()
    update_state.add_argument("--enabled", action="store_true", dest="enabled")
    update_state.add_argument("--disabled", action="store_false", dest="enabled")
    update_parser.set_defaults(enabled=None)

    for command, alias in (("remove", "unregister"), ("enable", None), ("disable", None), ("show", None)):
        parser_kwargs = {"aliases": [alias]} if alias else {}
        command_parser = subparsers.add_parser(command, **parser_kwargs)
        command_parser.add_argument("spec_id")
        command_parser.add_argument("--root", default=None)
        if command == "show":
            command_parser.add_argument("--json", action="store_true", dest="as_json")
    return parser


def _root(args: argparse.Namespace) -> str:
    sub_root = getattr(args, "root", None)
    return sub_root if sub_root is not None else args.global_root


def _normalize_argv(argv: list[str] | None) -> list[str]:
    raw = list(sys.argv[1:] if argv is None else argv)
    if raw and not raw[0].startswith("-") and raw[0] not in COMMANDS:
        root, rest = raw[0], raw[1:]
        return ["--root", root, "list", *rest]
    if not any(item in COMMANDS for item in raw):
        list_flags = {"--json", "--include-disabled", "--project-id"}
        insert_at = next((index for index, item in enumerate(raw) if item in list_flags), len(raw))
        return [*raw[:insert_at], "list", *raw[insert_at:]]
    return raw


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(_normalize_argv(argv))
    command = args.command or "list"
    try:
        if command in {"list", "ls"}:
            _print_index(build_index(_root(args), args.include_disabled, args.project_id), args.as_json)
        elif command in {"register", "add"}:
            entry = register_spec(
                _root(args), spec_id=args.id, name=args.name, spec_type=args.type, path=args.path,
                enabled=args.enabled, scope=args.scope, project_id=args.project_id,
                description=args.description, tags=args.tags, metadata=_metadata_from_json(args.metadata_json),
            )
            print(f"registered: {entry['id']} | {entry['name']}")
        elif command == "update":
            changes: dict[str, Any] = {}
            for key in ("id", "name", "type", "scope", "project_id", "path", "description"):
                value = getattr(args, key)
                if value is not None:
                    changes[key] = value
            if args.enabled is not None:
                changes["enabled"] = args.enabled
            if args.clear_tags:
                changes["tags"] = args.tags or []
            elif args.tags is not None:
                changes["tags"] = args.tags
            metadata = _metadata_from_json(args.metadata_json)
            if metadata is not None:
                changes["metadata"] = metadata
            if not changes:
                raise RegistryError("update requires at least one field")
            entry = update_spec(_root(args), args.spec_id, changes)
            print(f"updated: {entry['id']} | {entry['name']}")
        elif command in {"remove", "unregister"}:
            entry = remove_spec(_root(args), args.spec_id)
            print(f"removed: {entry['id']} | {entry['name']}")
        elif command in {"enable", "disable"}:
            entry = update_spec(_root(args), args.spec_id, {"enabled": command == "enable"})
            print(f"{command}d: {entry['id']}")
        elif command == "show":
            root = Path(_root(args)).resolve()
            entry = _find_entry(load_registry(root)["specs"], args.spec_id)
            kind, available = reference_status(root, entry["path"])
            result = dict(entry, reference_kind=kind, available=available)
            if args.as_json:
                print(json.dumps(result, ensure_ascii=False, indent=2))
            else:
                for key, value in result.items():
                    print(f"{key}: {value}")
        return 0
    except (OSError, RegistryError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
