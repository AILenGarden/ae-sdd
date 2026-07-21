from pathlib import Path

import pytest

from tools.lib import story_template_sections as sections


REPO_ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = REPO_ROOT / "source/templates/design/story-template.md"
GUIDE = REPO_ROOT / "source/standards/story/story-writing-guide.md"


def test_formal_template_has_17_primary_and_4_secondary_sections() -> None:
    text = TEMPLATE.read_text(encoding="utf-8")
    primary = sections.get_primary_story_sections(text, str(TEMPLATE))
    secondary = sections.get_secondary_story_sections(text, str(TEMPLATE))

    assert len(primary) == 17
    assert len(secondary) == 4
    assert [item.order for item in primary + secondary] != list(range(21))
    assert "main-flow" in {item.id for item in primary}
    assert "metadata" in {item.id for item in secondary}
    assert [item.id for item in primary[:8]] == [
        "user-story",
        "scope",
        "prerequisites",
        "affected-projects",
        "trigger-entry",
        "main-flow",
        "exception-flow",
        "dependencies-risks",
    ]
    assert [item.id for item in primary[-2:]] == [
        "implementation-design",
        "acceptance-criteria",
    ]


def test_layer_change_is_data_driven() -> None:
    template = """\
<!-- ae-sdd:story-section id=one layer=primary -->
## One
<!-- ae-sdd:story-section id=two layer=secondary -->
## Two
"""
    changed = template.replace("id=two layer=secondary", "id=two layer=primary")

    assert [item.id for item in sections.get_primary_story_sections(template)] == ["one"]
    assert [item.id for item in sections.get_primary_story_sections(changed)] == ["one", "two"]


@pytest.mark.parametrize(
    ("template", "code"),
    [
        ("## Missing\n", "missing-marker"),
        ("<!-- ae-sdd:story-section id=x layer=other -->\n## X\n", "invalid-layer"),
        ("<!-- ae-sdd:story-section id=x layer=primary -->\ntext\n", "orphan-marker"),
        (
            "<!-- ae-sdd:story-section id=x layer=primary -->\n## X\n"
            "<!-- ae-sdd:story-section id=x layer=secondary -->\n## Y\n",
            "duplicate-id",
        ),
    ],
)
def test_invalid_template_metadata_fails_closed(template: str, code: str) -> None:
    with pytest.raises(sections.StorySectionMetadataError, match=code):
        sections.parse_story_sections(template)


def test_generated_story_uses_id_not_title_for_classification() -> None:
    template = """\
<!-- ae-sdd:story-section id=main-flow layer=primary -->
## Renamed Main Flow
"""
    story = """\
<!-- ae-sdd:story-section id=main-flow -->
## Original Main Flow
content
"""

    ids, migrated = sections.resolve_story_document_section_ids(template, story)
    classified = sections.classify_story_section_ids(template, ids)

    assert migrated is False
    assert [item.id for item in classified["primary"]] == ["main-flow"]


def test_legacy_story_allows_only_exact_title_migration() -> None:
    template = """\
<!-- ae-sdd:story-section id=main-flow layer=primary -->
## Main Flow
"""
    ids, migrated = sections.resolve_story_document_section_ids(template, "## Main Flow\n")
    assert (ids, migrated) == (["main-flow"], True)

    with pytest.raises(sections.StorySectionMetadataError, match="unknown-section-title"):
        sections.resolve_story_document_section_ids(template, "## Similar Flow\n")


def test_partial_story_markers_fail_closed() -> None:
    story = """\
<!-- ae-sdd:story-section id=one -->
## One
## Two
"""
    with pytest.raises(sections.StorySectionMetadataError, match="missing-story-id"):
        sections.parse_story_document_section_ids(story)


def test_writing_guide_covers_every_template_id() -> None:
    template = TEMPLATE.read_text(encoding="utf-8")
    guide = GUIDE.read_text(encoding="utf-8")
    assert sections.validate_story_guide_coverage(
        template,
        guide,
        template_source_path=str(TEMPLATE),
        guide_source_path=str(GUIDE),
    ) == []


def test_formal_template_navigation_has_complete_stable_anchors() -> None:
    text = TEMPLATE.read_text(encoding="utf-8")
    assert sections.validate_story_navigation(text, str(TEMPLATE)) == []
    assert "| SPI | SPI-1 |" in text
    assert "| REST | REST-1 |" in text
    assert "\n---\n\n<a id=\"spi-1\"></a>" in text
    assert "\n---\n\n<a id=\"rest-1\"></a>" in text


def test_navigation_rejects_missing_or_broken_links() -> None:
    template = """\
**目录** [跳转](#missing)
<a id=one></a>
<!-- ae-sdd:story-section id=one layer=primary -->
## One
"""
    issues = sections.validate_story_navigation(template)
    assert any("missing-anchor" in issue for issue in issues)
    assert any("broken-navigation-link" in issue for issue in issues)


def test_generated_story_navigation_uses_actual_sections_and_interface_anchors() -> None:
    story = """\
# Story
**目录** [主流程](#main-flow)
**接口目录** [REST-1](#rest-1)
<a id="main-flow"></a>
<!-- ae-sdd:story-section id=main-flow -->
## 主流程
<a id="rest-1"></a>
### REST-1
"""
    assert sections.validate_story_document_navigation(story) == []


def test_generated_story_navigation_rejects_unlinked_interface_anchor() -> None:
    story = """\
# Story
**目录** [主流程](#main-flow)
<a id="main-flow"></a>
<!-- ae-sdd:story-section id=main-flow -->
## 主流程
<a id="rest-1"></a>
### REST-1
"""
    issues = sections.validate_story_document_navigation(story)
    assert any("unlinked-anchor" in issue for issue in issues)


def test_parser_module_has_no_file_io() -> None:
    source = (REPO_ROOT / "tools/lib/story_template_sections.py").read_text(encoding="utf-8")
    for forbidden in ("read_text(", "write_text(", ".glob(", ".rglob(", "open("):
        assert forbidden not in source
