---
name: testcase-review
description: 审查 testcase-generate-skill 产出的测试用例，按 TC-1~TC-10 检查口径核对 AC 覆盖率、全场景覆盖度、L1-L4 分层完整性、缺陷假设覆盖率，挖掘遗漏用例与冗余用例。当 TestCase 生成后自动触发、或开发者说"审查测试用例"、"检查测试覆盖"、"TestCase Review"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/testcase-review-skill.full.md
source_fallback_sha256: 85b9a7daf9d1e9bfb7a21d47e88771cc9ed1e46fe308f78eabbef6586dfad0e6
source_original_bytes: 7279
source_original_lines: 134
source_semantic_inventory_sha256: 3272a8f0820f8bf5217e474f27dd2ebc87a22773ff5ace00af1c5fa728bad9f4
source_slimmer: slim_source_skills.py@2
---

# TestCase Review — 测试用例缺陷挖掘 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/testcase-review-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/testcase-review-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/testcase-review-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/testcase-review-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/testcase-review-skill.full.md`
- fallback_sha256: `85b9a7daf9d1e9bfb7a21d47e88771cc9ed1e46fe308f78eabbef6586dfad0e6`
- original_lines: 134
- original_bytes: 7279
- semantic_inventory_sha256: `3272a8f0820f8bf5217e474f27dd2ebc87a22773ff5ace00af1c5fa728bad9f4`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 审查 testcase-generate-skill 产出的测试用例，按 TC-1~TC-10 检查口径核对 AC 覆盖率、全场景覆盖度、L1-L4 分层完整性、缺陷假设覆盖率，挖掘遗漏用例与冗余用例。当 TestCase 生成后自动触发、或开发者说"审查测试用例"、"检查测试覆盖"、"TestCase Review"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:22 文档存放前置调用; keyword_hits: 13 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:54 流程; keyword_hits: 10 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:127 禁止事项; keyword_hits: 27 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 12 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 1 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:22 文档存放前置调用; L2:104 输出要求; keyword_hits: 15 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 9; refs: ae-sdd doc resolve --intent STORY --story-id {S}; ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?}; ae-sdd doc save --intent TESTCASE_REVIEW --work-item {W} --story-id {S?} --content-file 草稿.md; +6 more; keyword_hits: 16 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 8 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | TestCase Review — 测试用例缺陷挖掘 Skill |
| 2 | 8 | 与监管器 4 步的关系 |
| 2 | 12 | 目标 |
| 2 | 16 | 依赖标准 |
| 2 | 22 | 文档存放前置调用 |
| 2 | 30 | 第零步：TestCase Review 准入检查 |
| 2 | 54 | 流程 |
| 2 | 66 | 检查口径（TC-1~TC-10） |
| 3 | 81 | TC-10 缺陷假设覆盖率专项校验（🆕 v3.6.3） |
| 2 | 97 | 核心原则 |
| 2 | 104 | 输出要求 |
| 2 | 110 | 与 generate 假设驱动的衔接（🆕 v3.6.3） |
| 2 | 123 | 退出条件 |
| 2 | 127 | 禁止事项 |

## Inline References

| ref |
| --- |
| ae-sdd doc resolve --intent STORY --story-id {S} |
| ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?} |
| ae-sdd doc save --intent TESTCASE_REVIEW --work-item {W} --story-id {S?} --content-file 草稿.md |
| constraints/assets/Story |
| document-storage-skill.get_assets/get_constraints(projectKey) |
| document-storage-skill.md |
| get_constraints/get_assets |
| review-loop-skill.md |
| testcase-generate-skill.md |
