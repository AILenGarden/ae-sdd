---
name: testcase-generate
description: TestCase 系列 Step 2 generateSkill。采用「假设驱动·覆盖兜底」范式——先挖掘缺陷假设（主线），再用三层覆盖矩阵查漏（兜底）。根据已通过 Story Review 的 Story 生成测试用例文档，覆盖 AC、全场景和 L1/L2/L3/L4 分层。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md
source_fallback_sha256: 632b332480f2ba2bc25b44be50eba2cf680d634e884c7692d5f7ae8777dd4771
source_original_bytes: 12272
source_original_lines: 237
source_semantic_inventory_sha256: c3a0be861be068566a442194d3a1f7cb9d8f72aade449466497ad2a646977fb5
source_slimmer: slim_source_skills.py@2
---

# TestCase Generate — 测试用例生成 SKILL Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/testcase-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md`
- fallback_sha256: `632b332480f2ba2bc25b44be50eba2cf680d634e884c7692d5f7ae8777dd4771`
- original_lines: 237
- original_bytes: 12272
- semantic_inventory_sha256: `c3a0be861be068566a442194d3a1f7cb9d8f72aade449466497ad2a646977fb5`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: TestCase 系列 Step 2 generateSkill。采用「假设驱动·覆盖兜底」范式——先挖掘缺陷假设（主线），再用三层覆盖矩阵查漏（兜底）。根据已通过 Story Review 的 Story 生成测试用例文档，覆盖 AC、全场景和 L1/L2/L3/L4 分层。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; keyword_hits: 12 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 26 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:213 禁止事项; keyword_hits: 22 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 19 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 20 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:145 输出; keyword_hits: 23 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 20; refs: ae-sdd assets read testcase --project <projectKey>; ae-sdd assets section §10; ae-sdd assets section §14; +17 more; keyword_hits: 23 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 32 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
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
| 1 | 6 | TestCase Generate — 测试用例生成 SKILL |
| 2 | 8 | 与监管器 4 步的关系 |
| 2 | 21 | 输入 |
| 2 | 46 | 生成规则 |
| 3 | 48 | 0. 缺陷假设挖掘（🆕 v3.6.3 主线环节） |
| 3 | 100 | 1. Story 类型识别 |
| 3 | 114 | 2. 用例生成（假设驱动 + 覆盖兜底） |
| 3 | 135 | 3. 测试真实性预埋 |
| 2 | 145 | 输出 |
| 2 | 155 | 合规性校验 |
| 3 | 173 | TC-G11 假设覆盖率专项校验（🆕 v3.6.3） |
| 2 | 213 | 禁止事项 |
| 2 | 227 | 执行清单 |

## Inline References

| ref |
| --- |
| ae-sdd assets read testcase --project <projectKey> |
| ae-sdd assets section §10 |
| ae-sdd assets section §14 |
| ae-sdd assets section §6.9 |
| ae-sdd doc resolve --intent STORY --story-id {S} |
| ae-sdd doc save --intent TESTCASE --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent TESTCASE_COMPLIANCE_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent TESTCASE_REVIEW --work-item {W} --story-id {S?} --content-file 草稿.md |
| be-testcase-strategy.md |
| be-testcase-strategy.md §通用缺陷假设库 |
| be-testcase-template.md |
| be-testcase-template.md §缺陷假设清单 |
| document-storage-skill.md |
| review-loop-skill.md |
| source/standards/constraints/testing.md |
| source/standards/testing/be-testcase-strategy.md |
| source/templates/testcase/be-testcase-template.md |
| src/test/java/...#method |
| testcase-generate-skill.md |
| testcase-review-skill.md |
