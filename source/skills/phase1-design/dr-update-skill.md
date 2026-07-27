---
name: dr-update
description: 根据 DR 补充说明文档和模板更新 DR 主文档。当 Story Update SKILL 发现 DR 设计缺陷时自动触发，或开发者说"更新 DR"、"同步 DR 补充说明"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/dr-update-skill.full.md
source_fallback_sha256: 6f5b2b83651c01d62afcfaeb01fa903229f90af9886d016f76667a79fe43f493
source_original_bytes: 7516
source_original_lines: 202
source_semantic_inventory_sha256: 6b5ae5def3c08004c2ccccfa306a99c281f24c9b6d2a95c1569a7b56399f0181
---

# DR Update — DR 文档更新 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/dr-update-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/dr-update-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/dr-update-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/dr-update-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/dr-update-skill.full.md`
- fallback_sha256: `6f5b2b83651c01d62afcfaeb01fa903229f90af9886d016f76667a79fe43f493`
- original_lines: 202
- original_bytes: 7516
- semantic_inventory_sha256: `6b5ae5def3c08004c2ccccfa306a99c281f24c9b6d2a95c1569a7b56399f0181`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 根据 DR 补充说明文档和模板更新 DR 主文档。当 Story Update SKILL 发现 DR 设计缺陷时自动触发，或开发者说"更新 DR"、"同步 DR 补充说明"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:8 📦 文档存放前置调用（🔴 横切依赖）; keyword_hits: 11 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:44 整体流程; keyword_hits: 3 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:179 禁止事项; keyword_hits: 14 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 20 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 11 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L1:6 DR Update — DR 文档更新 Skill; L2:8 📦 文档存放前置调用（🔴 横切依赖）; L2:64 第一步：读取 DR 补充说明文档; +2 more; keyword_hits: 46 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 11; refs: ae-sdd doc resolve --intent DR --doc-id {drId}; ae-sdd doc save; ae-sdd doc save --intent DR --doc-id {drId} --content-file 草稿.md; +8 more; keyword_hits: 13 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 6 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:64 第一步：读取 DR 补充说明文档; L2:157 {N}、DR 变更通知 - {日期}; L3:159 变更来源; keyword_hits: 27 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | DR Update — DR 文档更新 Skill |
| 2 | 8 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 38 | 目标 |
| 2 | 44 | 整体流程 |
| 2 | 64 | 第一步：读取 DR 补充说明文档 |
| 2 | 75 | 第二步：读取 DR 模板 |
| 2 | 83 | 第三步：更新 DR 主文档 |
| 3 | 87 | 3.1 更新规则 |
| 3 | 96 | 3.2 常见修改章节 |
| 3 | 106 | 3.3 更新后标记 |
| 2 | 112 | 第四步：评估影响范围 |
| 2 | 127 | 第四步 bis：DR 校验 |
| 2 | 150 | 第五步：通知受影响的 Story |
| 2 | 157 | {N}、DR 变更通知 - {日期} |
| 3 | 159 | 变更来源 |
| 3 | 164 | 对本 Story 的影响 |
| 3 | 169 | 处理建议 |
| 2 | 179 | 禁止事项 |
| 2 | 189 | 执行清单（逐项执行，不可跳过） |

## Inline References

| ref |
| --- |
| ae-sdd doc resolve --intent DR --doc-id {drId} |
| ae-sdd doc save |
| ae-sdd doc save --intent DR --doc-id {drId} --content-file 草稿.md |
| ae-sdd doc save --intent DR ... |
| ae-sdd doc save --intent DR_SUPPLEMENT --doc-id {drId} --content-file 草稿.md |
| ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md |
| document-storage-skill.md |
| document-storage-skill.md §9 |
| dr-template.md |
| templates/design/dr-template.md |
| {dr-prefix}-Supplement*.md |
