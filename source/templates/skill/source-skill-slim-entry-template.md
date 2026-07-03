# Source SKILL Slim Entry Template

This template is rendered by `scripts/slim_source_skills.py`; source slim entries must not be hand-edited.

```markdown
---
<original frontmatter without source_* fields>
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: <fallback path>
source_fallback_sha256: <sha256 of full fallback text>
source_original_bytes: <byte count>
source_original_lines: <line count>
source_semantic_inventory_sha256: <sha256 of semantic inventory JSON>
source_slimmer: slim_source_skills.py@2
---

# <title> Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `<fallback path>` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `<fallback path>` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `<fallback path>`, not this slim entry.

## Summary

- source: `<source path>`
- fallback: `<fallback path>`
- fallback_sha256: `<sha256>`
- original_lines: <line count>
- original_bytes: <byte count>
- semantic_inventory_sha256: `<sha256>`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: <frontmatter description or first paragraph>

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | <detected evidence> | <design docs> | <fallback rule> |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| <level> | <line> | <heading title> |

## Inline References

| ref |
| --- |
| <inline reference> |
```
