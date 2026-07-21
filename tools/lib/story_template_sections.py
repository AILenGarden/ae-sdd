"""Pure Story template/guide/document section metadata parsing.

File discovery and content loading belong to ``document_storage``.  Every
function in this module accepts text and performs no filesystem I/O.
"""
from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Iterable


_SECTION_ID = r"[a-z0-9]+(?:-[a-z0-9]+)*"
_TEMPLATE_MARKER_RE = re.compile(
    rf"^<!-- ae-sdd:story-section id=(?P<id>{_SECTION_ID}) "
    r"layer=(?P<layer>primary|secondary) -->$"
)
_STORY_MARKER_RE = re.compile(
    rf"^<!-- ae-sdd:story-section id=(?P<id>{_SECTION_ID}) -->$"
)
_GUIDE_MARKER_RE = re.compile(
    rf"^<!-- ae-sdd:story-guide section-id=(?P<id>{_SECTION_ID}) -->$"
)
_ANCHOR_RE = re.compile(rf'^<a id="(?P<id>{_SECTION_ID})"></a>$')
_LOCAL_LINK_RE = re.compile(rf"\]\(#(?P<id>{_SECTION_ID})\)")
_H2_RE = re.compile(r"^##\s+(?P<title>.+?)\s*$")


@dataclass(frozen=True)
class StorySection:
    id: str
    title: str
    layer: str
    order: int
    line: int


class StorySectionMetadataError(ValueError):
    """Raised when section metadata cannot be interpreted unambiguously."""

    def __init__(self, issues: Iterable[str]):
        self.issues = tuple(issues)
        super().__init__("; ".join(self.issues))


def _location(source_path: str, line: int) -> str:
    return f"{source_path or '<text>'}:{line}"


def _visible_lines(text: str) -> list[tuple[int, str]]:
    """Return non-fenced lines with their original one-based line number."""
    visible: list[tuple[int, str]] = []
    in_fence = False
    for line_number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if not in_fence:
            visible.append((line_number, stripped))
    return visible


def _normalize_title(title: str) -> str:
    # Legacy Story headings may still contain old template guidance tags.
    normalized = re.sub(r"\s+`(?:必填|选填|仅有|🔖|🔴).*?`", "", title).strip()
    return re.sub(r"\s+", " ", normalized)


def validate_story_section_metadata(
    template_text: str, source_path: str = ""
) -> list[str]:
    """Return deterministic validation issues for template H2 metadata."""
    lines = _visible_lines(template_text)
    issues: list[str] = []
    sections: list[StorySection] = []
    marked_heading_lines: set[int] = set()

    for index, (line_number, stripped) in enumerate(lines):
        if not stripped.startswith("<!-- ae-sdd:story-section"):
            continue
        marker = _TEMPLATE_MARKER_RE.fullmatch(stripped)
        if marker is None:
            code = "invalid-layer" if " layer=" in stripped else "invalid-marker"
            issues.append(
                f"[{code}] {_location(source_path, line_number)} malformed template marker"
            )
            continue
        if index + 1 >= len(lines):
            issues.append(
                f"[orphan-marker] {_location(source_path, line_number)} marker has no H2"
            )
            continue
        heading_line, heading_text = lines[index + 1]
        heading = _H2_RE.fullmatch(heading_text)
        if heading is None:
            issues.append(
                f"[orphan-marker] {_location(source_path, line_number)} marker must immediately precede H2"
            )
            continue
        marked_heading_lines.add(heading_line)
        sections.append(
            StorySection(
                id=marker.group("id"),
                title=_normalize_title(heading.group("title")),
                layer=marker.group("layer"),
                order=len(sections),
                line=heading_line,
            )
        )

    for line_number, stripped in lines:
        if _H2_RE.fullmatch(stripped) and line_number not in marked_heading_lines:
            issues.append(
                f"[missing-marker] {_location(source_path, line_number)} H2 has no section marker"
            )

    ids: dict[str, StorySection] = {}
    titles: dict[str, StorySection] = {}
    for section in sections:
        if section.id in ids:
            issues.append(
                f"[duplicate-id] {_location(source_path, section.line)} id={section.id}"
            )
        else:
            ids[section.id] = section
        title_key = section.title.casefold()
        if title_key in titles:
            issues.append(
                f"[duplicate-title] {_location(source_path, section.line)} title={section.title}"
            )
        else:
            titles[title_key] = section
    return issues


def parse_story_sections(
    template_text: str, source_path: str = ""
) -> list[StorySection]:
    issues = validate_story_section_metadata(template_text, source_path)
    if issues:
        raise StorySectionMetadataError(issues)
    sections: list[StorySection] = []
    lines = _visible_lines(template_text)
    for index, (line_number, stripped) in enumerate(lines):
        marker = _TEMPLATE_MARKER_RE.fullmatch(stripped)
        if marker is None:
            continue
        heading_line, heading_text = lines[index + 1]
        heading = _H2_RE.fullmatch(heading_text)
        assert heading is not None  # validation above guarantees this
        sections.append(
            StorySection(
                id=marker.group("id"),
                title=_normalize_title(heading.group("title")),
                layer=marker.group("layer"),
                order=len(sections),
                line=heading_line,
            )
        )
    return sections


def get_primary_story_sections(
    template_text: str, source_path: str = ""
) -> list[StorySection]:
    return [
        section
        for section in parse_story_sections(template_text, source_path)
        if section.layer == "primary"
    ]


def get_secondary_story_sections(
    template_text: str, source_path: str = ""
) -> list[StorySection]:
    return [
        section
        for section in parse_story_sections(template_text, source_path)
        if section.layer == "secondary"
    ]


def validate_story_navigation(
    template_text: str, source_path: str = ""
) -> list[str]:
    """Validate stable anchors and local navigation links in a Story template."""
    sections = parse_story_sections(template_text, source_path)
    lines = _visible_lines(template_text)
    anchors: dict[str, int] = {}
    links: list[tuple[str, int]] = []
    issues: list[str] = []
    for line_number, stripped in lines:
        anchor = _ANCHOR_RE.fullmatch(stripped)
        if anchor is not None:
            anchor_id = anchor.group("id")
            if anchor_id in anchors:
                issues.append(
                    f"[duplicate-anchor] {_location(source_path, line_number)} id={anchor_id}"
                )
            else:
                anchors[anchor_id] = line_number
        links.extend(
            (match.group("id"), line_number)
            for match in _LOCAL_LINK_RE.finditer(stripped)
        )

    section_ids = {section.id for section in sections}
    for section in sections:
        anchor_line = anchors.get(section.id)
        if anchor_line is None:
            issues.append(
                f"[missing-anchor] {_location(source_path, section.line)} id={section.id}"
            )
            continue
        marker_line = next(
            (
                line_number
                for line_number, stripped in lines
                if line_number < section.line
                and stripped
                == f"<!-- ae-sdd:story-section id={section.id} layer={section.layer} -->"
                and line_number > anchor_line
            ),
            None,
        )
        if marker_line is None or marker_line != anchor_line + 1:
            issues.append(
                f"[anchor-marker-order] {_location(source_path, section.line)} id={section.id}"
            )

    linked_ids = {link_id for link_id, _ in links}
    for section_id in sorted(section_ids - linked_ids):
        issues.append(f"[missing-navigation-link] id={section_id}")
    for link_id, line_number in links:
        if link_id not in anchors:
            issues.append(
                f"[broken-navigation-link] {_location(source_path, line_number)} id={link_id}"
            )
    for anchor_id in sorted(set(anchors) - linked_ids):
        issues.append(f"[unlinked-anchor] id={anchor_id}")
    return issues


def validate_story_document_navigation(
    story_text: str, source_path: str = ""
) -> list[str]:
    """Validate navigation against the sections actually present in a Story."""
    section_ids = parse_story_document_section_ids(story_text, source_path)
    lines = _visible_lines(story_text)
    anchors: dict[str, int] = {}
    links: list[tuple[str, int]] = []
    issues: list[str] = []
    marker_lines: dict[str, int] = {}

    for line_number, stripped in lines:
        anchor = _ANCHOR_RE.fullmatch(stripped)
        if anchor is not None:
            anchor_id = anchor.group("id")
            if anchor_id in anchors:
                issues.append(
                    f"[duplicate-anchor] {_location(source_path, line_number)} id={anchor_id}"
                )
            else:
                anchors[anchor_id] = line_number
        marker = _STORY_MARKER_RE.fullmatch(stripped)
        if marker is not None:
            marker_lines[marker.group("id")] = line_number
        links.extend(
            (match.group("id"), line_number)
            for match in _LOCAL_LINK_RE.finditer(stripped)
        )

    for section_id in section_ids:
        anchor_line = anchors.get(section_id)
        marker_line = marker_lines.get(section_id)
        if anchor_line is None:
            issues.append(f"[missing-anchor] id={section_id}")
        elif marker_line != anchor_line + 1:
            issues.append(f"[anchor-marker-order] id={section_id}")

    linked_ids = {link_id for link_id, _ in links}
    for section_id in sorted(set(section_ids) - linked_ids):
        issues.append(f"[missing-navigation-link] id={section_id}")
    for link_id, line_number in links:
        if link_id not in anchors:
            issues.append(
                f"[broken-navigation-link] {_location(source_path, line_number)} id={link_id}"
            )
    for anchor_id in sorted(set(anchors) - linked_ids):
        issues.append(f"[unlinked-anchor] id={anchor_id}")
    return issues


def parse_story_document_section_ids(
    story_text: str, source_path: str = ""
) -> list[str]:
    """Parse id-only markers from a generated Story; partial metadata fails."""
    lines = _visible_lines(story_text)
    ids: list[str] = []
    issues: list[str] = []
    marked_heading_lines: set[int] = set()
    marker_count = 0
    for index, (line_number, stripped) in enumerate(lines):
        if not stripped.startswith("<!-- ae-sdd:story-section"):
            continue
        marker_count += 1
        marker = _STORY_MARKER_RE.fullmatch(stripped)
        if marker is None:
            issues.append(
                f"[invalid-story-marker] {_location(source_path, line_number)} expected id-only marker"
            )
            continue
        if index + 1 >= len(lines) or _H2_RE.fullmatch(lines[index + 1][1]) is None:
            issues.append(
                f"[orphan-marker] {_location(source_path, line_number)} marker must immediately precede H2"
            )
            continue
        heading_line = lines[index + 1][0]
        marked_heading_lines.add(heading_line)
        section_id = marker.group("id")
        if section_id in ids:
            issues.append(
                f"[duplicate-story-id] {_location(source_path, line_number)} id={section_id}"
            )
        ids.append(section_id)
    if marker_count:
        for line_number, stripped in lines:
            if _H2_RE.fullmatch(stripped) and line_number not in marked_heading_lines:
                issues.append(
                    f"[missing-story-id] {_location(source_path, line_number)} H2 has no id marker"
                )
    if issues:
        raise StorySectionMetadataError(issues)
    return ids


def classify_story_section_ids(
    template_text: str,
    section_ids: Iterable[str],
    source_path: str = "",
) -> dict[str, list[StorySection]]:
    sections = parse_story_sections(template_text, source_path)
    by_id = {section.id: section for section in sections}
    result: dict[str, list[StorySection]] = {"primary": [], "secondary": []}
    issues: list[str] = []
    for section_id in section_ids:
        section = by_id.get(str(section_id))
        if section is None:
            issues.append(f"[unknown-section-id] {source_path or '<text>'} id={section_id}")
            continue
        result[section.layer].append(section)
    if issues:
        raise StorySectionMetadataError(issues)
    return result


def resolve_story_document_section_ids(
    template_text: str,
    story_text: str,
    *,
    template_source_path: str = "",
    story_source_path: str = "",
) -> tuple[list[str], bool]:
    """Return Story section IDs and whether exact-title legacy migration was used."""
    ids = parse_story_document_section_ids(story_text, story_source_path)
    if ids:
        classify_story_section_ids(template_text, ids, template_source_path)
        return ids, False

    sections = parse_story_sections(template_text, template_source_path)
    by_title = {section.title.casefold(): section.id for section in sections}
    migrated: list[str] = []
    issues: list[str] = []
    for line_number, stripped in _visible_lines(story_text):
        heading = _H2_RE.fullmatch(stripped)
        if heading is None:
            continue
        title = _normalize_title(heading.group("title"))
        section_id = by_title.get(title.casefold())
        if section_id is None:
            issues.append(
                f"[unknown-section-title] {_location(story_source_path, line_number)} title={title}"
            )
        else:
            migrated.append(section_id)
    if issues:
        raise StorySectionMetadataError(issues)
    return migrated, True


def parse_story_guide_section_ids(
    guide_text: str, source_path: str = ""
) -> list[str]:
    ids: list[str] = []
    issues: list[str] = []
    for line_number, stripped in _visible_lines(guide_text):
        if not stripped.startswith("<!-- ae-sdd:story-guide"):
            continue
        marker = _GUIDE_MARKER_RE.fullmatch(stripped)
        if marker is None:
            issues.append(
                f"[invalid-guide-marker] {_location(source_path, line_number)} malformed guide marker"
            )
            continue
        section_id = marker.group("id")
        if section_id in ids:
            issues.append(
                f"[duplicate-guide-id] {_location(source_path, line_number)} id={section_id}"
            )
        ids.append(section_id)
    if issues:
        raise StorySectionMetadataError(issues)
    return ids


def validate_story_guide_coverage(
    template_text: str,
    guide_text: str,
    *,
    template_source_path: str = "",
    guide_source_path: str = "",
) -> list[str]:
    template_ids = {section.id for section in parse_story_sections(template_text, template_source_path)}
    guide_ids = set(parse_story_guide_section_ids(guide_text, guide_source_path))
    issues = [f"[missing-guide-section] id={value}" for value in sorted(template_ids - guide_ids)]
    issues.extend(f"[orphan-guide-section] id={value}" for value in sorted(guide_ids - template_ids))
    return issues


def render_story_section_marker(section_id: str) -> str:
    if re.fullmatch(_SECTION_ID, section_id) is None:
        raise StorySectionMetadataError([f"[invalid-section-id] id={section_id}"])
    return f"<!-- ae-sdd:story-section id={section_id} -->"
