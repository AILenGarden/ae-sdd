---
name: git-insight
description: Read-only Git insight toolset for ae-sdd. Produces structured status, diff, log, blame, and impact evidence for CodingPlan, CodingReport, CodeReview, and postmortem.
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/git-insight-skill.full.md
source_fallback_sha256: 0867561448c77a987ebf5c1ea79139e8456de99f907fc5d0217c521ee768da7d
source_original_bytes: 1410
source_original_lines: 56
source_semantic_inventory_sha256: 8c8f53675271995dc5f84bb1171e75fce86cb72a0cd0ee6e69b0290bd2399ac4
source_slimmer: slim_source_skills.py@2
---

# Git Insight Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/git-insight-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/git-insight-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/git-insight-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/git-insight-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/git-insight-skill.full.md`
- fallback_sha256: `0867561448c77a987ebf5c1ea79139e8456de99f907fc5d0217c521ee768da7d`
- original_lines: 56
- original_bytes: 1410
- semantic_inventory_sha256: `8c8f53675271995dc5f84bb1171e75fce86cb72a0cd0ee6e69b0290bd2399ac4`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Read-only Git insight toolset for ae-sdd. Produces structured status, diff, log, blame, and impact evidence for CodingPlan, CodingReport, CodeReview, and postmortem.

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| gate_constraint | keyword_hits: 2 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 8 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| output_doc_contract | keyword_hits: 2 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 1; refs: ae-sdd git impact | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Git Insight Skill |
| 2 | 8 | 1. Purpose |
| 2 | 20 | 2. Commands |
| 2 | 30 | 3. Read-Only Boundary |
| 2 | 36 | 4. Required Usage |
| 2 | 45 | 5. Risk Hints |

## Inline References

| ref |
| --- |
| ae-sdd git impact |
