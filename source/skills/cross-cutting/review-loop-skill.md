---
name: review-loop
description: |
  Review Loop 公共协议（🆕 v3.4.3）— 所有 review/循环判定节点的公共骨架。
  统一 Review Batch v2：输入指纹、有效批次、风险策略、失败分类、硬预算和 Plan-first。旧 round/dryCounter 仅作兼容投影。
  各 review SKILL（story-review / dr-review / code-review / task-generate TR / proposal / story-generate）只定义自己的检查项与 Plan 载体，loop 骨架引用本协议。
  🆕 v3.4.3 废弃"每 N 轮暂停问人"——与退出条件矛盾，且把退出权交给人违反 Loop Engineering 自评估原则。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/review-loop-skill.full.md
source_fallback_sha256: 0b7b2b2934cea2fdb0d0d64e04da5032693cfcfcc34eea895e60b3bbdc3350d3
source_original_bytes: 10121
source_original_lines: 125
source_semantic_inventory_sha256: 1c08bc6db4e83eae450ef77ce56e4777542aab6bdb4e9a87525ac57503d265a4
source_slimmer: slim_source_skills.py@2
---

# Review Loop — 公共协议（所有 review 节点的 loop 骨架） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/review-loop-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/review-loop-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/review-loop-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/review-loop-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/review-loop-skill.full.md`
- fallback_sha256: `0b7b2b2934cea2fdb0d0d64e04da5032693cfcfcc34eea895e60b3bbdc3350d3`
- original_lines: 125
- original_bytes: 10121
- semantic_inventory_sha256: `1c08bc6db4e83eae450ef77ce56e4777542aab6bdb4e9a87525ac57503d265a4`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Review Loop 公共协议（🆕 v3.4.3）— 所有 review/循环判定节点的公共骨架。
统一 Review Batch v2：输入指纹、有效批次、风险策略、失败分类、硬预算和 Plan-first。旧 round/dryCounter 仅作兼容投影。
各 review SKILL（story-review / dr-review / code-review / task-generate TR / proposal / story-generate）只定义自己的检查项与 Plan 载体，loop 骨架引用本协议。
🆕 v3.4.3 废弃"每 N 轮暂停问人"——与退出条件矛盾，且把退出权交给人违反 Loop Engineering 自评估原则。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | keyword_hits: 19 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:21 📋 核心协议（Review Batch v2，所有 review 节点必须遵守）; L2:73 🔴 禁止条款（v3.4.3 废弃）; L3:75 禁止 1：禁止"每 N 轮暂停问人"; +1 more; keyword_hits: 20 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 3 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 7 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | keyword_hits: 3 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 9; refs: ae-sdd-skill.md; agent-orchestration-skill.md; code-review-skill.md; +6 more; keyword_hits: 23 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 7 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:119 📖 实施历史; keyword_hits: 10 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 10 | Review Loop — 公共协议（所有 review 节点的 loop 骨架） |
| 2 | 21 | 📋 核心协议（Review Batch v2，所有 review 节点必须遵守） |
| 3 | 23 | 协议 0：Review Batch 是权威对象 |
| 3 | 37 | 协议 1：退出条件 |
| 3 | 49 | 协议 2：循环上限 |
| 3 | 60 | 协议 3：Plan-first（有确认缺陷必先出 Plan） |
| 2 | 73 | 🔴 禁止条款（v3.4.3 废弃） |
| 3 | 75 | 禁止 1：禁止"每 N 轮暂停问人" |
| 3 | 86 | 禁止 2：禁止无预算循环 |
| 2 | 92 | 📌 各节点专属配置（本协议只管骨架，专属配置由各 SKILL 自定义） |
| 2 | 108 | 🔗 与其他 SKILL 的关系 |
| 2 | 119 | 📖 实施历史 |

## Inline References

| ref |
| --- |
| ae-sdd-skill.md |
| agent-orchestration-skill.md |
| code-review-skill.md |
| dr-review-skill.md |
| proposal-skill.md |
| requirement-analysis-skill.md |
| story-generate-skill.md |
| story-review-skill.md |
| task-generate-skill.md §5bis |
