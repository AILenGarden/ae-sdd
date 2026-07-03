---
name: database-tool
description: Local-profile database toolset for ae-sdd. AI writes SQL, ae-sdd db reads local connection profiles, enforces read-first policy, executes supported adapters, and returns auditable evidence.
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/database-tool-skill.full.md
source_fallback_sha256: 4b290b533a7c19b2dcb8518b9f465f760bf60fcc0b64f2c78e88d6f948a8f3fc
source_original_bytes: 1871
source_original_lines: 63
source_semantic_inventory_sha256: 3372d78780bf98ec0215da8e1d6af424ba0e4e21e1c559fd1b580cae9e6db1fb
source_slimmer: slim_source_skills.py@2
---

# Database Tool Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/database-tool-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/database-tool-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/database-tool-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/database-tool-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/database-tool-skill.full.md`
- fallback_sha256: `4b290b533a7c19b2dcb8518b9f465f760bf60fcc0b64f2c78e88d6f948a8f3fc`
- original_lines: 63
- original_bytes: 1871
- semantic_inventory_sha256: `3372d78780bf98ec0215da8e1d6af424ba0e4e21e1c559fd1b580cae9e6db1fb`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Local-profile database toolset for ae-sdd. AI writes SQL, ae-sdd db reads local connection profiles, enforces read-first policy, executes supported adapters, and returns auditable evidence.

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:46 4. Phase Rules; keyword_hits: 2 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | keyword_hits: 3 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 8 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L2:46 4. Phase Rules; keyword_hits: 4 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | keyword_hits: 2 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 1; refs: db-connection-profile.schema.md; keyword_hits: 3 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Database Tool Skill |
| 2 | 8 | 1. Purpose |
| 2 | 21 | 2. Profile Location |
| 2 | 32 | 3. Commands |
| 2 | 46 | 4. Phase Rules |
| 2 | 56 | 5. Hard Rules |

## Inline References

| ref |
| --- |
| db-connection-profile.schema.md |
