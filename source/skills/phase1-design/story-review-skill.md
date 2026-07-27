---
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/story-review-skill.full.md
source_fallback_sha256: f1c6b661bc4027fe99c277048dd0f8d606fc5d43b04b143f70926346d86de436
source_original_bytes: 2596
source_original_lines: 54
source_semantic_inventory_sha256: 5159f99b1e67b8d0156f0f2b14b3818da841b5cc8070fa3ec435d205e59621ac
---

# Story Review - Story 缺陷挖掘 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/story-review-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/story-review-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/story-review-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/story-review-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/story-review-skill.full.md`
- fallback_sha256: `f1c6b661bc4027fe99c277048dd0f8d606fc5d43b04b143f70926346d86de436`
- original_lines: 54
- original_bytes: 2596
- semantic_inventory_sha256: `5159f99b1e67b8d0156f0f2b14b3818da841b5cc8070fa3ec435d205e59621ac`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 基于当前权威 Story 模板和撰写指南审查 Story。Document Storage 返回资源正文与 sha256；`story_template_sections` 按 section ID 提供章节和层级；本 Skill 只编排 Review。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | keyword_hits: 4 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 1 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:48 禁止事项; keyword_hits: 11 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 2 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 3 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L2:30 ID 与历史文档; L2:44 输出; keyword_hits: 7 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 1; refs: state.review.status/findings | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| fallback_only_detail | headings: L2:30 ID 与历史文档; keyword_hits: 3 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 1 | Story Review - Story 缺陷挖掘 Skill |
| 2 | 3 | 目标 |
| 2 | 7 | 资源与准入 |
| 2 | 14 | Review scope |
| 3 | 16 | `scope=primary` |
| 3 | 23 | `scope=full` |
| 2 | 30 | ID 与历史文档 |
| 2 | 36 | 检查维度 |
| 2 | 44 | 输出 |
| 2 | 48 | 禁止事项 |

## Inline References

| ref |
| --- |
| state.review.status/findings |
