---
name: test-review
description: Test 系列 Step 3 reviewSkill。由 test-verifier 独立复核 immutable evidence、真实性扫描、HTTP 双阶段与 AC 覆盖，决定是否回到 test-generate。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase3-review/test-review-skill.full.md
source_fallback_sha256: 9e9287743b0530e14c9d3f8c3eaa0ef187f2a7aea9900c338ca68a1feba97769
source_original_bytes: 6137
source_original_lines: 110
source_semantic_inventory_sha256: 4e4c10a7f75dbd0784fe97b84a544d7d71874f59d44229a7ae6695bef9a29998
source_slimmer: slim_source_skills.py@2
---

# Test Review — 测试真实性复核 SKILL Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase3-review/test-review-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase3-review/test-review-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase3-review/test-review-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase3-review/test-review-skill.md`
- fallback: `skill-fallbacks/skills/phase3-review/test-review-skill.full.md`
- fallback_sha256: `9e9287743b0530e14c9d3f8c3eaa0ef187f2a7aea9900c338ca68a1feba97769`
- original_lines: 110
- original_bytes: 6137
- semantic_inventory_sha256: `4e4c10a7f75dbd0784fe97b84a544d7d71874f59d44229a7ae6695bef9a29998`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Test 系列 Step 3 reviewSkill。由 test-verifier 独立复核 immutable evidence、真实性扫描、HTTP 双阶段与 AC 覆盖，决定是否回到 test-generate。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 7 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:54 G-09 work-item scope 规则; L2:91 禁止事项; keyword_hits: 41 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 9 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 9 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:65 输出; keyword_hits: 14 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 9; refs: .auto-engineering/{WORKITEM-ID}/evidence/manifest.json; ae-sdd evidence finalize --story {STORY-ID}; ae-sdd evidence record; +6 more; keyword_hits: 3 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 2 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 5 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Test Review — 测试真实性复核 SKILL |
| 2 | 8 | 与监管器 4 步的关系 |
| 2 | 12 | 强制独立性 |
| 2 | 25 | 输入 |
| 2 | 37 | 检查口径（TV-1~TV-10） |
| 3 | 54 | G-09 work-item scope 规则 |
| 2 | 65 | 输出 |
| 2 | 81 | 缺陷处理 |
| 2 | 91 | 禁止事项 |
| 2 | 101 | 执行清单 |

## Inline References

| ref |
| --- |
| .auto-engineering/{WORKITEM-ID}/evidence/manifest.json |
| ae-sdd evidence finalize --story {STORY-ID} |
| ae-sdd evidence record |
| ae-sdd gates check --only G-09,G-10 |
| ae-sdd verify plan --story {STORY-ID} --work-item {WORK-ITEM} --changed <paths> --persist |
| agent-orchestration-skill.md |
| review-loop-skill.md |
| state.review.status/findings |
| test-generate-skill.md |
