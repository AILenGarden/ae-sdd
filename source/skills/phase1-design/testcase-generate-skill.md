---
name: testcase-generate
description: TestCase 系列 Step 2 generateSkill。采用「假设驱动·覆盖兜底」范式——先挖掘缺陷假设（主线），再用三层覆盖矩阵查漏（兜底）。根据已通过 Story Review 的 Story 生成测试用例文档，覆盖 AC、全场景和 L1/L2/L3/L4 分层。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md
source_fallback_sha256: 0bf95b956aa13f3994341aa59b62d1e4670a95d08fb026f3911b10d1cdcacee7
source_original_bytes: 13322
source_original_lines: 261
source_semantic_inventory_sha256: 585463c8cd32e15d69863c0b53e734259364944ac83227502832395a19aeca7a
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
- fallback_sha256: `0bf95b956aa13f3994341aa59b62d1e4670a95d08fb026f3911b10d1cdcacee7`
- original_lines: 261
- original_bytes: 13322
- semantic_inventory_sha256: `585463c8cd32e15d69863c0b53e734259364944ac83227502832395a19aeca7a`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: TestCase 系列 Step 2 generateSkill。采用「假设驱动·覆盖兜底」范式——先挖掘缺陷假设（主线），再用三层覆盖矩阵查漏（兜底）。根据已通过 Story Review 的 Story 生成测试用例文档，覆盖 AC、全场景和 L1/L2/L3/L4 分层。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; keyword_hits: 13 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 26 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:237 禁止事项; keyword_hits: 32 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 25 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 20 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:169 输出; keyword_hits: 26 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 23; refs: ae-sdd assets read testcase --project <projectKey>; ae-sdd assets section §10; ae-sdd assets section §14; +20 more; keyword_hits: 25 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 33 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
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
| 2 | 21 | 第零步：TestCase 准入检查 |
| 2 | 45 | 输入 |
| 2 | 70 | 生成规则 |
| 3 | 72 | 0. 缺陷假设挖掘（🆕 v3.6.3 主线环节） |
| 3 | 124 | 1. Story 类型识别 |
| 3 | 138 | 2. 用例生成（假设驱动 + 覆盖兜底） |
| 3 | 159 | 3. 测试真实性预埋 |
| 2 | 169 | 输出 |
| 2 | 179 | 合规性校验 |
| 3 | 197 | TC-G11 假设覆盖率专项校验（🆕 v3.6.3） |
| 2 | 237 | 禁止事项 |
| 2 | 251 | 执行清单 |

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
| constraints/*.md |
| constraints/assets/Story |
| document-storage-skill.md |
| get_constraints/get_assets |
| review-loop-skill.md |
| source/standards/constraints/testing.md |
| source/standards/testing/be-testcase-strategy.md |
| source/templates/testcase/be-testcase-template.md |
| src/test/java/...#method |
| testcase-generate-skill.md |
| testcase-review-skill.md |
