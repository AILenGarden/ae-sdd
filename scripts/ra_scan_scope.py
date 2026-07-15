#!/usr/bin/env python3
"""Resolve authoritative Requirement Analysis documents for RA scanners.

Two modes are supported:

* ``file``: the caller supplies one or more authoritative Markdown files.
* ``root``: the resolver discovers formal RA documents while excluding generated
  event records, reports, templates, references, and build artefacts.

Keeping this policy in one module prevents the four RA scanners from drifting
back to independent ``rglob`` heuristics.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, Sequence


FORMAL_RA_FILENAME = re.compile(r"^RA[-_].+\.md$", re.IGNORECASE)
GENERATED_RA_FILENAME = re.compile(
    r"(?:-GeneratePlan-r\d+|-Impact-r\d+|-ReverseIssues(?:-r\d+)?|"
    r"-Review(?:-r\d+)?|-Report(?:-r\d+)?|-ChangeLog)$",
    re.IGNORECASE,
)
EXCLUDED_DIRECTORY_NAMES = {
    ".git",
    ".ae-sdd",
    ".auto-engineering",
    ".hermes",
    ".pytest_cache",
    ".venv",
    "__pycache__",
    "build",
    "changelog",
    "dist",
    "node_modules",
    "reference",
    "references",
    "reports",
    "template",
    "templates",
    "vendor",
}
LEGACY_RA_PARENT_NAMES = {"design", "ra"}


class RAScanScopeError(ValueError):
    """Raised when an explicit scan target is invalid or escapes the root."""


@dataclass(frozen=True)
class ExcludedRAFile:
    path: str
    reason: str


@dataclass(frozen=True)
class RAScanScope:
    mode: str
    root: Path
    files: tuple[Path, ...]
    excluded: tuple[ExcludedRAFile, ...] = field(default_factory=tuple)

    @property
    def selected_files(self) -> list[str]:
        return [relative_path(path, self.root) for path in self.files]

    @property
    def excluded_files(self) -> list[dict[str, str]]:
        return [
            {"path": item.path, "reason": item.reason}
            for item in self.excluded
        ]


def relative_path(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _generated_reason(path: Path, root: Path) -> str | None:
    relative = path.relative_to(root)
    directory_names = {part.casefold() for part in relative.parts[:-1]}
    excluded_dirs = sorted(directory_names & EXCLUDED_DIRECTORY_NAMES)
    if excluded_dirs:
        return f"excluded-directory:{excluded_dirs[0]}"

    if path.name.casefold() in {"changelog.md", "ra-template.md"}:
        return "generated-or-template-filename"

    if GENERATED_RA_FILENAME.search(path.stem):
        return "generated-ra-event"

    return None


def _is_formal_ra_location(path: Path, root: Path) -> bool:
    relative = path.relative_to(root)
    parent_parts = [part.casefold() for part in relative.parts[:-1]]

    for index in range(len(parent_parts) - 1):
        if parent_parts[index] == "ae-sdd-doc" and parent_parts[index + 1] == "ra":
            return True

    if any(part in LEGACY_RA_PARENT_NAMES for part in parent_parts):
        return True

    return False


def classify_formal_ra(path: Path, root: Path) -> tuple[bool, str]:
    """Return whether ``path`` is a formal root-discovery candidate and why."""
    resolved_root = root.resolve()
    resolved_path = path.resolve()
    if not _is_within(resolved_path, resolved_root):
        return False, "outside-scan-root"
    if not _is_within(path, resolved_root):
        return False, "outside-scan-root"

    if path.suffix.casefold() != ".md" or not FORMAL_RA_FILENAME.match(path.name):
        return False, "not-formal-ra-filename"

    generated_reason = _generated_reason(path, root)
    if generated_reason:
        return False, generated_reason

    if not _is_formal_ra_location(path, root):
        return False, "non-authoritative-location"

    return True, "formal-ra"


def _deduplicate(paths: Iterable[Path]) -> tuple[Path, ...]:
    unique = {str(path).casefold(): path for path in paths}
    return tuple(sorted(unique.values(), key=lambda item: item.as_posix().casefold()))


def resolve_ra_scan_scope(
    root: Path,
    explicit_files: Sequence[str | Path] | None = None,
) -> RAScanScope:
    """Resolve the exact files a scanner is allowed to inspect."""
    resolved_root = root.resolve()
    requested = list(explicit_files or [])

    if requested:
        selected: list[Path] = []
        for raw_path in requested:
            candidate = Path(raw_path)
            if not candidate.is_absolute():
                candidate = resolved_root / candidate
            candidate = candidate.resolve()

            if not _is_within(candidate, resolved_root):
                raise RAScanScopeError(
                    f"explicit RA file escapes scan root: {candidate} (root={resolved_root})"
                )
            if not candidate.is_file():
                raise RAScanScopeError(f"explicit RA file does not exist: {candidate}")
            if candidate.suffix.casefold() != ".md":
                raise RAScanScopeError(f"explicit RA file must be Markdown: {candidate}")
            selected.append(candidate)

        return RAScanScope(
            mode="file",
            root=resolved_root,
            files=_deduplicate(selected),
        )

    selected = []
    excluded: list[ExcludedRAFile] = []
    for candidate in sorted(resolved_root.rglob("*.md"), key=lambda item: item.as_posix().casefold()):
        accepted, reason = classify_formal_ra(candidate, resolved_root)
        if accepted:
            selected.append(candidate.resolve())
        elif FORMAL_RA_FILENAME.match(candidate.name):
            excluded.append(
                ExcludedRAFile(
                    path=relative_path(candidate, resolved_root),
                    reason=reason,
                )
            )

    return RAScanScope(
        mode="root",
        root=resolved_root,
        files=_deduplicate(selected),
        excluded=tuple(excluded),
    )


def ra_scan_scope_error_payload(
    error: Exception,
    root: Path,
    explicit_files: Sequence[str | Path] | None = None,
) -> dict:
    """Return the stable machine-readable error contract used by all scanners."""
    return {
        "root": str(root.resolve()),
        "scopeMode": "file" if explicit_files else "root",
        "selectedFiles": [],
        "excludedFiles": [],
        "status": "ERROR",
        "raFiles": 0,
        "error": {
            "code": "INVALID_RA_SCAN_SCOPE",
            "message": str(error),
        },
    }
