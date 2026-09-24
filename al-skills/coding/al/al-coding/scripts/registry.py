#!/usr/bin/env python3
"""Register and index author-owned SKILL references for ALCoding.

ALCoding stores registration metadata only. The referenced SKILL is never
parsed, rewritten, merged, or executed by this module.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path
from typing import Any

import yaml

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

import re

ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
URI_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")

def is_reference(raw_path: str) -> bool:
    if re.match(r"^[A-Za-z]:[\\/]", raw_path) or raw_path.startswith("\\\\"):
        return False
    return bool(URI_RE.match(raw_path))



SKILL_TYPES = ("global", "language", "architecture", "project")
SCOPES = ("global", "project")
MANAGED_SKILL_DIR = "registered-skills"
MAX_UPLOAD_BYTES = 10 * 1024 * 1024
COMMANDS = {"list", "register", "add", "update", "remove", "unregister", "enable", "disable", "show"}


class RegistryError(RuntimeError):
    """Raised when the registration table cannot be safely used or changed."""


def registry_path(root: str | Path) -> Path:
    return Path(root).resolve() / "framework" / "registry.yaml"


def load_registry(root: str | Path, *, validate: bool = True) -> dict[str, Any]:
    root_path = Path(root).resolve()
    path = registry_path(root_path)
    if validate:
        try:
            value = yaml.safe_load(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, yaml.YAMLError) as exc:
            raise RegistryError(f"cannot read {path}: {exc}") from exc
        if not isinstance(value, dict) or not isinstance(value.get("skills"), list):
            raise RegistryError("registry.yaml must contain a skills list")
        _validate_before_write(root_path, value)
        return value
    try:
        value = yaml.safe_load(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, yaml.YAMLError) as exc:
        raise RegistryError(f"cannot read {path}: {exc}") from exc
    if not isinstance(value, dict) or not isinstance(value.get("skills"), list):
        raise RegistryError("registry.yaml must contain a skills list")
    return value


def resolve_path(root: Path, raw_path: str) -> Path:
    path = Path(raw_path).expanduser()
    return path if path.is_absolute() else root / path


def managed_skill_path(root: str | Path, skill_id: str) -> Path:
    """Return the path used for a SKILL uploaded through the local UI."""
    if not ID_RE.fullmatch(skill_id):
        raise RegistryError("id must be lowercase kebab-case")
    return Path(root).resolve() / MANAGED_SKILL_DIR / skill_id / "SKILL.md"


def _write_uploaded_content(root: Path, skill_id: str, content: bytes) -> Path:
    if not isinstance(content, bytes):
        raise RegistryError("uploaded SKILL content must be bytes")
    if len(content) > MAX_UPLOAD_BYTES:
        raise RegistryError(f"uploaded SKILL exceeds {MAX_UPLOAD_BYTES} bytes")
    target = managed_skill_path(root, skill_id)
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, temp_name = tempfile.mkstemp(prefix=f".{target.name}.", suffix=".tmp", dir=target.parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_name, target)
    except OSError as exc:
        try:
            os.unlink(temp_name)
        except OSError:
            pass
        raise RegistryError(f"cannot save uploaded SKILL {target}: {exc}") from exc
    return target


def reference_status(root: Path, raw_path: str) -> tuple[str, bool | None]:
    """Classify a reference without imposing a format on its target."""
    if is_reference(raw_path):
        return "reference", None
    return "file", resolve_path(root, raw_path).is_file()


def build_index(
    root: str | Path, include_disabled: bool = False, project_id: str | None = None
) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    selected_project = project_id.strip().lower() if isinstance(project_id, str) and project_id.strip() else None
    categories: dict[str, list[dict[str, Any]]] = {key: [] for key in SKILL_TYPES}
    for entry in registry["skills"]:
        enabled = entry["enabled"] is True
        if not include_disabled and not enabled:
            continue
        skill_type = entry["type"]
        scope = entry.get("scope", "global")
        if selected_project is not None and scope == "project" and entry.get("project_id") != selected_project:
            continue
        kind, available = reference_status(root_path, entry["path"])
        item = {
            "id": entry["id"],
            "name": entry["name"],
            "type": skill_type,
            "scope": scope,
            "path": entry["path"],
            "enabled": enabled,
            "reference_kind": kind,
            "available": available,
        }
        if "project_id" in entry:
            item["project_id"] = entry["project_id"]
        for key in ("description", "tags", "metadata"):
            if key in entry:
                item[key] = entry[key]
        categories[skill_type].append(item)
    return {
        "schema_version": registry["schema_version"],
        "project_id": selected_project,
        "categories": categories,
    }


def _identity_key(value: str) -> str:
    return value.strip().casefold()


def _find_entry(entries: list[dict[str, Any]], skill_id: str) -> dict[str, Any]:
    key = _identity_key(skill_id)
    for entry in entries:
        if _identity_key(str(entry.get("id", ""))) == key:
            return entry
    raise RegistryError(f"SKILL id not found: {skill_id}")


def _check_identity(entries: list[dict[str, Any]], skill_id: str, name: str, current: dict[str, Any] | None = None) -> None:
    id_key, name_key = _identity_key(skill_id), _identity_key(name)
    if not ID_RE.fullmatch(skill_id):
        raise RegistryError("id must be lowercase kebab-case")
    if not name.strip():
        raise RegistryError("name must be non-empty")
    for entry in entries:
        if entry is current:
            continue
        if _identity_key(str(entry.get("id", ""))) == id_key:
            raise RegistryError(f"duplicate SKILL id: {skill_id}")
        if _identity_key(str(entry.get("name", ""))) == name_key:
            raise RegistryError(f"duplicate SKILL name: {name}")


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


def _validate_before_write(root: Path, registry: dict[str, Any]) -> None:
    if registry.get("schema_version") not in (2, 3):
        raise RegistryError("schema_version must be 2 or 3")
    entries = registry.get("skills")
    if not isinstance(entries, list):
        raise RegistryError("registry.yaml must contain a skills list")
    ids: set[str] = set()
    names: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise RegistryError(f"skills[{index}] must be a mapping")
        skill_id = entry.get("id")
        if not isinstance(skill_id, str) or not ID_RE.fullmatch(skill_id):
            raise RegistryError(f"skills[{index}].id must be lowercase kebab-case")
        if skill_id in ids:
            raise RegistryError(f"duplicate skill id: {skill_id}")
        ids.add(skill_id)
        name = entry.get("name")
        if not isinstance(name, str) or not name.strip():
            raise RegistryError(f"skills[{index}].name must be non-empty")
        if name.casefold() in names:
            raise RegistryError(f"duplicate skill name: {name}")
        names.add(name.casefold())
        if entry.get("type") not in SKILL_TYPES:
            raise RegistryError(f"skills[{index}].type must be one of {list(SKILL_TYPES)}")
        if entry.get("scope", "global") not in SCOPES:
            raise RegistryError(f"skills[{index}].scope must be one of {list(SCOPES)}")
        if not isinstance(entry.get("path"), str) or not entry["path"].strip():
            raise RegistryError(f"skills[{index}].path must be non-empty")


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


def _normalize_scope(scope: str | None, project_id: str | None) -> tuple[str, str | None]:
    normalized_scope = (scope or "global").strip().lower()
    if normalized_scope not in SCOPES:
        raise RegistryError(f"scope must be one of {list(SCOPES)}")
    normalized_project_id = project_id.strip().lower() if isinstance(project_id, str) else None
    if normalized_scope == "project":
        if not normalized_project_id or not ID_RE.fullmatch(normalized_project_id):
            raise RegistryError("project_id must be lowercase kebab-case when scope is project")
        return normalized_scope, normalized_project_id
    if normalized_project_id:
        raise RegistryError("project_id is only allowed when scope is project")
    return normalized_scope, None


def register_skill(root: str | Path, *, skill_id: str, name: str, skill_type: str, path: str,
                   enabled: bool = True, description: str | None = None,
                   tags: list[str] | None = None, metadata: dict[str, Any] | None = None,
                   scope: str = "global", project_id: str | None = None) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entries = registry["skills"]
    skill_id, name, path = skill_id.strip(), name.strip(), path.strip()
    _check_identity(entries, skill_id, name)
    if skill_type not in SKILL_TYPES:
        raise RegistryError(f"type must be one of {list(SKILL_TYPES)}")
    normalized_scope, normalized_project_id = _normalize_scope(scope, project_id)
    entry: dict[str, Any] = {
        "id": skill_id, "name": name, "type": skill_type,
        "scope": normalized_scope, "path": path, "enabled": enabled,
    }
    if normalized_project_id is not None:
        entry["project_id"] = normalized_project_id
    if not path:
        raise RegistryError("path must be non-empty")
    if description is not None:
        entry["description"] = description
    if tags:
        entry["tags"] = tags
    if metadata is not None:
        entry["metadata"] = metadata
    entries.append(entry)
    if registry.get("schema_version", 0) < 3:
        registry["schema_version"] = 3
    _validate_before_write(root_path, registry)
    _write_registry(root_path, registry)
    return entry


def register_uploaded_skill(root: str | Path, *, skill_id: str, name: str, skill_type: str,
                            content: bytes, enabled: bool = True,
                            description: str | None = None, tags: list[str] | None = None,
                            metadata: dict[str, Any] | None = None, scope: str = "global",
                            project_id: str | None = None) -> dict[str, Any]:
    """Persist an uploaded SKILL copy and register its metadata.

    The file is stored under ``registered-skills/<id>/SKILL.md``. Its contents
    remain opaque to ALCoding. If registry validation or the registry write
    fails, the newly created file is removed so the operation is all-or-nothing.
    """
    root_path = Path(root).resolve()
    skill_id = skill_id.strip()
    target = managed_skill_path(root_path, skill_id)
    if target.exists():
        raise RegistryError(f"managed SKILL already exists: {skill_id}")
    _write_uploaded_content(root_path, skill_id, content)
    relative_path = target.relative_to(root_path).as_posix()
    try:
        return register_skill(
            root_path, skill_id=skill_id, name=name, skill_type=skill_type,
            path=relative_path, enabled=enabled, description=description,
            tags=tags, metadata=metadata, scope=scope, project_id=project_id,
        )
    except Exception:
        try:
            target.unlink()
            target.parent.rmdir()
            target.parent.parent.rmdir()
        except OSError:
            pass
        raise


def replace_uploaded_skill(root: str | Path, skill_id: str, content: bytes) -> Path:
    """Replace a managed uploaded SKILL without inspecting its contents."""
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entry = _find_entry(registry["skills"], skill_id)
    target = managed_skill_path(root_path, str(entry["id"]))
    expected = target.relative_to(root_path).as_posix()
    if entry.get("path") != expected:
        raise RegistryError("only SKILLs stored by the ALCoding upload UI can be replaced")
    _write_uploaded_content(root_path, str(entry["id"]), content)
    return target


def update_skill(root: str | Path, skill_id: str, changes: dict[str, Any]) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entries = registry["skills"]
    entry = _find_entry(entries, skill_id)
    candidate_id = str(changes.get("id", entry["id"])).strip()
    candidate_name = str(changes.get("name", entry["name"])).strip()
    _check_identity(entries, candidate_id, candidate_name, current=entry)
    requested_scope = changes.get("scope", entry.get("scope", "global"))
    requested_project_id = changes.get("project_id", entry.get("project_id"))
    if "scope" in changes and str(requested_scope).strip().lower() == "global" and "project_id" not in changes:
        requested_project_id = None
    candidate_scope, candidate_project_id = _normalize_scope(requested_scope, requested_project_id)
    if "id" in changes:
        entry["id"] = candidate_id
    if "name" in changes:
        entry["name"] = candidate_name
    if "type" in changes:
        if changes["type"] not in SKILL_TYPES:
            raise RegistryError(f"type must be one of {list(SKILL_TYPES)}")
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
    if "scope" in changes or "project_id" in changes:
        if registry.get("schema_version", 0) < 3:
            registry["schema_version"] = 3
    _validate_before_write(root_path, registry)
    _write_registry(root_path, registry)
    return entry


def remove_skill(root: str | Path, skill_id: str) -> dict[str, Any]:
    root_path = Path(root).resolve()
    registry = load_registry(root_path)
    entries = registry["skills"]
    entry = _find_entry(entries, skill_id)
    entries.remove(entry)
    _validate_before_write(root_path, registry)
    _write_registry(root_path, registry)
    return entry


def _print_index(index: dict[str, Any], as_json: bool) -> None:
    if as_json:
        print(json.dumps(index, ensure_ascii=False, indent=2))
        return
    for skill_type in SKILL_TYPES:
        print(f"[{skill_type}]")
        for entry in index["categories"][skill_type]:
            state = "enabled" if entry["enabled"] else "disabled"
            availability = entry["reference_kind"] if entry["reference_kind"] == "reference" else ("available" if entry["available"] else "missing")
            scope = entry.get("scope", "global")
            project = f"/{entry['project_id']}" if entry.get("project_id") else ""
            print(f"- {entry['id']} | {entry['name']} | {scope}{project} | {state} | {availability} | {entry['path']}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Register and index ALCoding SKILL references")
    parser.add_argument("--root", dest="global_root", default=".", help="ALCoding root")
    subparsers = parser.add_subparsers(dest="command")

    list_parser = subparsers.add_parser("list", aliases=["ls"], help="list registered SKILLs")
    list_parser.add_argument("--root", default=None)
    list_parser.add_argument("--include-disabled", action="store_true")
    list_parser.add_argument("--json", action="store_true", dest="as_json")
    list_parser.add_argument("--project-id", help="include only global entries and entries for this project")

    register_parser = subparsers.add_parser("register", aliases=["add"], help="register a SKILL")
    register_parser.add_argument("--root", default=None)
    register_parser.add_argument("--id", required=True)
    register_parser.add_argument("--name", required=True)
    register_parser.add_argument("--type", required=True, choices=SKILL_TYPES)
    register_parser.add_argument("--scope", choices=SCOPES, default="global")
    register_parser.add_argument("--project-id", dest="project_id")
    register_parser.add_argument("--path", required=True)
    register_parser.add_argument("--description")
    register_parser.add_argument("--tag", action="append", dest="tags")
    register_parser.add_argument("--metadata-json")
    state = register_parser.add_mutually_exclusive_group()
    state.add_argument("--enabled", action="store_true", dest="enabled")
    state.add_argument("--disabled", action="store_false", dest="enabled")
    register_parser.set_defaults(enabled=True)

    update_parser = subparsers.add_parser("update", help="update a registered SKILL")
    update_parser.add_argument("skill_id")
    update_parser.add_argument("--root", default=None)
    update_parser.add_argument("--id")
    update_parser.add_argument("--name")
    update_parser.add_argument("--type", choices=SKILL_TYPES)
    update_parser.add_argument("--scope", choices=SCOPES)
    update_parser.add_argument("--project-id", dest="project_id")
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
        parser_kwargs = {"help": f"{command} a registered SKILL"}
        if alias:
            parser_kwargs["aliases"] = [alias]
        command_parser = subparsers.add_parser(command, **parser_kwargs)
        command_parser.add_argument("skill_id")
        command_parser.add_argument("--root", default=None)
        if command == "show":
            command_parser.add_argument("--json", action="store_true", dest="as_json")
    return parser


def _root(args: argparse.Namespace) -> str:
    sub_root = getattr(args, "root", None)
    return sub_root if sub_root is not None else args.global_root


def _normalize_argv(argv: list[str] | None) -> list[str]:
    """Keep the previous ``registry.py ROOT --json`` list syntax working."""
    raw = list(sys.argv[1:] if argv is None else argv)
    if raw and not raw[0].startswith("-") and raw[0] not in COMMANDS:
        root, rest = raw[0], raw[1:]
        return ["--root", root, "list", *rest]
    if not any(item in COMMANDS for item in raw):
        list_flags = {"--json", "--include-disabled"}
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
            entry = register_skill(
                _root(args), skill_id=args.id, name=args.name, skill_type=args.type, path=args.path,
                enabled=args.enabled, description=args.description, tags=args.tags,
                metadata=_metadata_from_json(args.metadata_json), scope=args.scope, project_id=args.project_id,
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
            entry = update_skill(_root(args), args.skill_id, changes)
            print(f"updated: {entry['id']} | {entry['name']}")
        elif command in {"remove", "unregister"}:
            entry = remove_skill(_root(args), args.skill_id)
            print(f"removed: {entry['id']} | {entry['name']}")
        elif command in {"enable", "disable"}:
            entry = update_skill(_root(args), args.skill_id, {"enabled": command == "enable"})
            print(f"{command}d: {entry['id']}")
        elif command == "show":
            registry = load_registry(_root(args))
            entry = _find_entry(registry["skills"], args.skill_id)
            kind, available = reference_status(Path(_root(args)).resolve(), entry["path"])
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
