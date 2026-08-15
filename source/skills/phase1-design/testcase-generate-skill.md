---
name: testcase-generate
description: TestCase 系列 Step 2 generateSkill。采用有界风险驱动范式：先建立有限风险登记，再按行为等价类和最低充分层级选择用例，以停止条件和预算例外限制无价值边界扩张。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md
source_fallback_sha256: 0035513bce917c0c58258dd47ac5a1b621c6f1c913b665c49246dd4c7bbbc13d
source_original_bytes: 15221
source_original_lines: 241
source_semantic_inventory_sha256: 5139ee2f103175d9ad7648c328115de382c52d337f4003eaef28d1cd164a7e54
---

# TestCase Generate — 测试设计能力（Story 路线必经） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not hand-edit generated slim sections. Refresh from the fallback with `ae-sdd-build source-slim --source source --skill skills/phase1-design/testcase-generate-skill.md --refresh`.
- Validate canonical rendered bytes with `ae-sdd-build source-slim --source source --skill skills/phase1-design/testcase-generate-skill.md --validate`.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/testcase-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/testcase-generate-skill.full.md`
- fallback_sha256: `0035513bce917c0c58258dd47ac5a1b621c6f1c913b665c49246dd4c7bbbc13d`
- original_lines: 241
- original_bytes: 15221
- semantic_inventory_sha256: `5139ee2f103175d9ad7648c328115de382c52d337f4003eaef28d1cd164a7e54`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: TestCase 系列 Step 2 generateSkill。采用有界风险驱动范式：先建立有限风险登记，再按行为等价类和最低充分层级选择用例，以停止条件和预算例外限制无价值边界扩张。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; keyword_hits: 15 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 22 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:215 禁止事项; keyword_hits: 39 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 29 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 18 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L2:176 输出; keyword_hits: 24 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 24; refs: ae-sdd assets read testcase --project <projectKey>; ae-sdd assets section §10; ae-sdd assets section §14; +21 more; keyword_hits: 26 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L1:6 TestCase Generate — 测试设计能力（Story 路线必经）; keyword_hits: 18 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 8 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry through the native `ae-sdd-build source-slim` command; do not hand-edit generated sections.
4. Validate `source_fallback_sha256`, required sections, `source_semantic_inventory_sha256`, and canonical-byte equality.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | TestCase Generate — 测试设计能力（Story 路线必经） |
| 2 | 10 | 与监管器 4 步的关系 |
| 2 | 23 | 第零步：TestCase 准入检查 |
| 2 | 47 | 输入 |
| 3 | 49 | 场景推导前置 |
| 2 | 76 | 生成规则 |
| 3 | 78 | 0. 有限风险登记与缺陷假设挖掘 |
| 3 | 129 | 1. Story 类型识别 |
| 3 | 143 | 2. 用例生成（风险驱动 + 有界查漏） |
| 3 | 166 | 3. 测试真实性预埋 |
| 2 | 176 | 输出 |
| 2 | 186 | 合规性校验 |
| 3 | 204 | TC-G11 测试组合价值与有界性专项校验 |
| 2 | 215 | 禁止事项 |
| 2 | 231 | 执行清单 |

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
| constraints/ |
| constraints/*.md |
| constraints/README.md |
| constraints/assets/Story |
| document-storage-skill.md |
| exclude/defer |
| review-loop-skill.md |
| source/standards/constraints/testing.md |
| source/standards/testing/be-testcase-strategy.md |
| source/templates/testcase/be-testcase-template.md |
| src/test/java/...#method |
| testcase-generate-skill.md |
| testcase-review-skill.md |
