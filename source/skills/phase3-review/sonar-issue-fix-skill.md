---
name: sonar-issue-fix
description: Sonar 问题修复 SKILL。接收 SonarQube、SonarCloud、SonarQube for IDE、MCP 或导出报告中的 issue，在上游 TextEdit、保守硬编码规则、推理修复和人工处理之间做唯一分类，执行防陈旧/防越界校验，并用编译、测试和 Sonar 复扫闭环。由 CodeReview 在收尾闸门前每个评审会话恰好调用一次；用户要求修复 Sonar、质量门失败、处理 rule key 或清理静态分析问题时也触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase3-review/sonar-issue-fix-skill.full.md
source_fallback_sha256: 81029ea3aafd9395b9bc8fb1dad468e09cf1b206983b738aad107846e2c0d1de
source_original_bytes: 9584
source_original_lines: 157
source_semantic_inventory_sha256: 6d607ceb9a91160dbb22b3414c4c510cdd037e7e558ad0af8a6b663e1e447a65
---

# Sonar Issue Fix Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase3-review/sonar-issue-fix-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase3-review/sonar-issue-fix-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase3-review/sonar-issue-fix-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase3-review/sonar-issue-fix-skill.md`
- fallback: `skill-fallbacks/skills/phase3-review/sonar-issue-fix-skill.full.md`
- fallback_sha256: `81029ea3aafd9395b9bc8fb1dad468e09cf1b206983b738aad107846e2c0d1de`
- original_lines: 157
- original_bytes: 9584
- semantic_inventory_sha256: `6d607ceb9a91160dbb22b3414c4c510cdd037e7e558ad0af8a6b663e1e447a65`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Sonar 问题修复 SKILL。接收 SonarQube、SonarCloud、SonarQube for IDE、MCP 或导出报告中的 issue，在上游 TextEdit、保守硬编码规则、推理修复和人工处理之间做唯一分类，执行防陈旧/防越界校验，并用编译、测试和 Sonar 复扫闭环。由 CodeReview 在收尾闸门前每个评审会话恰好调用一次；用户要求修复 Sonar、质量门失败、处理 rule key 或清理静态分析问题时也触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L3:99 1. 建立调用上下文; keyword_hits: 12 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:97 执行流程; keyword_hits: 1 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:147 禁止事项; keyword_hits: 41 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 9 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 7 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | keyword_hits: 8 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 3; refs: N/A; fixed/skipped/manual/unverified/failed; sonar-issue-fix-rules.md; keyword_hits: 9 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
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
| 1 | 6 | Sonar Issue Fix |
| 2 | 8 | 定位与边界 |
| 2 | 23 | 输入契约 |
| 2 | 43 | 唯一分类 |
| 2 | 58 | EditPlan 协议 |
| 2 | 97 | 执行流程 |
| 3 | 99 | 1. 建立调用上下文 |
| 3 | 103 | 2. 归一化与分类 |
| 3 | 107 | 3. 形成并校验 EditPlan |
| 3 | 116 | 4. 应用最小修改 |
| 3 | 120 | 5. 验证闭环 |
| 3 | 132 | 6. 返回结果 |
| 2 | 136 | CodeReview 收尾协议 |
| 2 | 147 | 禁止事项 |

## Inline References

| ref |
| --- |
| N/A |
| fixed/skipped/manual/unverified/failed |
| sonar-issue-fix-rules.md |
