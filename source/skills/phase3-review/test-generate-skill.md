---
name: test-generate
description: Test 系列 Step 2 generateSkill。Coding 完成后运行编译、启动与 L1/L2/L3/L4 测试，把真实命令和 artifact 写入 evidence manifest。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase3-review/test-generate-skill.full.md
source_fallback_sha256: 38ff60885978ec9899e0c843808da206c2feb34d093fbbe24b35c30d9647b26e
source_original_bytes: 5706
source_original_lines: 100
source_semantic_inventory_sha256: 0d2ce3cf40b62fbbdb30d944c12b17ed4ee4b0021f3ba51ee1098171fba50c6a
---

# Test Generate — 测试运行与 Evidence SKILL Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase3-review/test-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md`
- fallback_sha256: `38ff60885978ec9899e0c843808da206c2feb34d093fbbe24b35c30d9647b26e`
- original_lines: 100
- original_bytes: 5706
- semantic_inventory_sha256: `0d2ce3cf40b62fbbdb30d944c12b17ed4ee4b0021f3ba51ee1098171fba50c6a`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Test 系列 Step 2 generateSkill。Coding 完成后运行编译、启动与 L1/L2/L3/L4 测试，把真实命令和 artifact 写入 evidence manifest。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; keyword_hits: 1 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:32 流程; keyword_hits: 14 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:80 禁止事项; keyword_hits: 25 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:49 2. 执行验证命令; keyword_hits: 22 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 8 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | keyword_hits: 12 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 10; refs: .auto-engineering/{WORKITEM-ID}/evidence/; ae-sdd evidence finalize --story {STORY-ID}; ae-sdd evidence lookup; +7 more; keyword_hits: 7 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 2 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 2 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Test Generate — 测试运行与 Evidence SKILL |
| 2 | 10 | 与监管器 4 步的关系 |
| 2 | 23 | 输入 |
| 2 | 32 | 流程 |
| 3 | 34 | 0. 先执行场景合同 |
| 3 | 38 | 1. 制定执行矩阵 |
| 3 | 49 | 2. 执行验证命令 |
| 3 | 63 | 3. 记录 Evidence |
| 3 | 72 | 4. 初步判定 |
| 2 | 80 | 禁止事项 |
| 2 | 91 | 执行清单 |

## Inline References

| ref |
| --- |
| .auto-engineering/{WORKITEM-ID}/evidence/ |
| ae-sdd evidence finalize --story {STORY-ID} |
| ae-sdd evidence lookup |
| ae-sdd evidence record |
| ae-sdd gates check --only G-09 |
| ae-sdd verify plan --story {STORY-ID} --changed <paths> |
| ae-sdd gates check --only G-09 |
| source/standards/constraints/testing.md |
| test-generate-skill.md |
| test-review-skill.md |
