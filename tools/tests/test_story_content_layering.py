"""Source contracts for template-driven Story primary/secondary workflow."""
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
FALLBACK_ROOT = REPO_ROOT / "source/skill-fallbacks/skills/phase1-design"
SKILLS = [
    FALLBACK_ROOT / "story-generate-skill.full.md",
    FALLBACK_ROOT / "story-review-skill.full.md",
    FALLBACK_ROOT / "story-update-skill.full.md",
]


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def test_story_skills_use_document_storage_content_and_shared_parser() -> None:
    for path in SKILLS:
        text = _read(path)
        assert "STORY_TEMPLATE" in text, path.name
        assert "STORY_WRITING_GUIDE" in text, path.name
        assert "content" in text, path.name
        assert "story_template_sections" in text, path.name
        assert "source/templates/design/story-template.md" not in text, path.name
        assert "C-01~C-09" not in text, path.name


def test_generate_keeps_id_only_markers_and_primary_first() -> None:
    text = _read(SKILLS[0])
    assert "<!-- ae-sdd:story-section id={section.id} -->" in text
    assert "Review(scope=primary)" in text
    assert "Review(scope=full)" in text
    assert text.index("Review(scope=primary)") < text.index("Review(scope=full)")


def test_review_and_update_classify_by_id_with_exact_legacy_migration() -> None:
    for path in SKILLS[1:]:
        text = _read(path)
        assert "section ID" in text, path.name
        assert "标题精确" in text, path.name
        assert "语义猜测" in text, path.name


def test_layering_standard_has_no_fixed_section_inventory() -> None:
    text = _read(REPO_ROOT / "source/standards/story/story-content-layering-standard.md")
    assert "C-01" not in text
    assert "19 个" not in text
    assert "11 个" not in text
    assert "get_primary_story_sections" in text
    assert "get_secondary_story_sections" in text


def test_template_layout_standard_allows_story_external_writing_guide_mode() -> None:
    text = _read(REPO_ROOT / "source/standards/templates/template-layout-standard.md")
    assert "独立撰写指南模式" in text
    assert "Story 模板采用本模式" in text
    assert "其他模板未显式声明采用本模式时" in text
