---
name: postman-tool
description: Postman platform adapter for ae-sdd. When the user provides a test-env URL, ae-sdd builds AC scenarios into a Postman collection, runs the monitor, and records http-external-supplemental evidence. Does not replace the L2 local HTTP main chain.
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/postman-tool-skill.full.md
source_fallback_sha256: afe264248d5c2a6db66fa63d6eba787e097f1c888df0907fa4b62348b9e3aa05
source_original_bytes: 2729
source_original_lines: 71
source_semantic_inventory_sha256: baed3d7127d7172efc854c5f2ac481d635a27a8df265875f65585c3ec3347fa7
source_slimmer: slim_source_skills.py@2
---

# Postman Tool Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/postman-tool-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/postman-tool-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/postman-tool-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/postman-tool-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/postman-tool-skill.full.md`
- fallback_sha256: `afe264248d5c2a6db66fa63d6eba787e097f1c888df0907fa4b62348b9e3aa05`
- original_lines: 71
- original_bytes: 2729
- semantic_inventory_sha256: `baed3d7127d7172efc854c5f2ac481d635a27a8df265875f65585c3ec3347fa7`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Postman platform adapter for ae-sdd. When the user provides a test-env URL, ae-sdd builds AC scenarios into a Postman collection, runs the monitor, and records http-external-supplemental evidence. Does not replace the L2 local HTTP main chain.

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:51 4. Phase Rules; keyword_hits: 2 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | keyword_hits: 3 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 4 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L2:51 4. Phase Rules; keyword_hits: 4 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | keyword_hits: 2 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 3; refs: dist/; postman-profile.schema.md; source/; keyword_hits: 6 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Postman Tool Skill |
| 2 | 8 | 1. Purpose |
| 2 | 21 | 2. Profile Location |
| 2 | 34 | 3. Capability |
| 2 | 51 | 4. Phase Rules |
| 2 | 61 | 5. Hard Rules |

## Inline References

| ref |
| --- |
| dist/ |
| postman-profile.schema.md |
| source/ |
