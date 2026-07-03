---
name: test-generate
description: Test 系列 Step 2 generateSkill。Coding 完成后运行编译、启动与 L1/L2/L3/L4 测试，生成带原始证据链的测试报告。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase3-review/test-generate-skill.full.md
source_fallback_sha256: 22dbf658b7a1da340a0225dd89b3389962b8c68184fd50e79275f79c0ded121f
source_original_bytes: 4136
source_original_lines: 96
source_semantic_inventory_sha256: 4539a1db0994118829139c3e564d897d82703022ee044c59b628c360ce862098
source_slimmer: slim_source_skills.py@2
---

# Test Generate — 测试运行与报告 SKILL Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase3-review/test-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase3-review/test-generate-skill.full.md`
- fallback_sha256: `22dbf658b7a1da340a0225dd89b3389962b8c68184fd50e79275f79c0ded121f`
- original_lines: 96
- original_bytes: 4136
- semantic_inventory_sha256: `4539a1db0994118829139c3e564d897d82703022ee044c59b628c360ce862098`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Test 系列 Step 2 generateSkill。Coding 完成后运行编译、启动与 L1/L2/L3/L4 测试，生成带原始证据链的测试报告。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; keyword_hits: 1 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:31 流程; keyword_hits: 10 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:77 禁止事项; keyword_hits: 15 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:44 2. 执行验证命令; keyword_hits: 12 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| output_doc_contract | keyword_hits: 6 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 8; refs: .auto-engineering/{WORKITEM-ID}/evidence/; ae-sdd doc save --intent TEST_REPORT --work-item {WORKITEM-ID} --story-id {S?} --version "v1-r1" --content-file 草稿.md; ae-sdd gates check --only G-09; +5 more; keyword_hits: 11 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 3 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 1 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Test Generate — 测试运行与报告 SKILL |
| 2 | 8 | 与监管器 4 步的关系 |
| 2 | 21 | 输入 |
| 2 | 31 | 流程 |
| 3 | 33 | 1. 制定执行矩阵 |
| 3 | 44 | 2. 执行验证命令 |
| 3 | 57 | 3. 生成测试报告 |
| 3 | 69 | 4. 初步判定 |
| 2 | 77 | 禁止事项 |
| 2 | 87 | 执行清单 |

## Inline References

| ref |
| --- |
| .auto-engineering/{WORKITEM-ID}/evidence/ |
| ae-sdd doc save --intent TEST_REPORT --work-item {WORKITEM-ID} --story-id {S?} --version "v1-r1" --content-file 草稿.md |
| ae-sdd gates check --only G-09 |
| scripts/test_authenticity_scan.py |
| source/standards/constraints/testing.md |
| source/templates/testcase/be-testcase-report-template.md |
| test-generate-skill.md |
| test-review-skill.md |
