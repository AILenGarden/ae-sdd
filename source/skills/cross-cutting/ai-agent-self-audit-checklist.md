---
name: ai-agent-self-audit-checklist
description: AI Agent 任务开始前自审清单 SOP（🔴 强制，每任务必跑）。覆盖 5 步骤（识别任务类型 / 识别输入类型 / 识别最小必跑流程硬卡片 / 用户催促反模式处理 / 自审完成声明）。🆕 2026-06-27 新建，解决"AI Agent 撞到出文档类任务直接动笔跳过 RA skill"的系统性违规（实测案例：2026-06-27 AI Agent 直接读 PDF 出 36KB proposal 未走 RA skill 完整 7 步）。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md
source_fallback_sha256: 46b64b2c50992bdc1718a9ce27efaf97e851e7227c72568c41fd935a749b37cd
source_original_bytes: 7783
source_original_lines: 133
source_semantic_inventory_sha256: 4af18754df745887d6b5209ba596a0e91d8d5715b6ef5be821e7359c7d9df061
source_slimmer: slim_source_skills.py@2
---

# AI Agent 自审清单 SOP — 任务开始前强制自审 Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/ai-agent-self-audit-checklist.md`
- fallback: `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md`
- fallback_sha256: `46b64b2c50992bdc1718a9ce27efaf97e851e7227c72568c41fd935a749b37cd`
- original_lines: 133
- original_bytes: 7783
- semantic_inventory_sha256: `4af18754df745887d6b5209ba596a0e91d8d5715b6ef5be821e7359c7d9df061`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: AI Agent 任务开始前自审清单 SOP（🔴 强制，每任务必跑）。覆盖 5 步骤（识别任务类型 / 识别输入类型 / 识别最小必跑流程硬卡片 / 用户催促反模式处理 / 自审完成声明）。🆕 2026-06-27 新建，解决"AI Agent 撞到出文档类任务直接动笔跳过 RA skill"的系统性违规（实测案例：2026-06-27 AI Agent 直接读 PDF 出 36KB proposal 未走 RA skill 完整 7 步）。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:19 触发条件; L3:46 步骤 2：识别输入类型 + 触发 SKILL; keyword_hits: 16 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L3:59 步骤 3：识别最小必跑流程的"硬卡片"; keyword_hits: 20 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:109 反模式（本 SOP 禁止的 4 类行为）; keyword_hits: 19 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 5 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 5 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | keyword_hits: 20 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 7; refs: SKILL.md; ae-sdd enter <projectKey> --story <STORY-ID>; document-storage-skill.md; +4 more; keyword_hits: 9 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 13 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
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
| 1 | 6 | AI Agent 自审清单 SOP — 任务开始前强制自审 |
| 2 | 19 | 触发条件 |
| 2 | 32 | 5 步骤自审清单（🔴 强制顺序） |
| 3 | 34 | 步骤 1：识别任务类型 |
| 3 | 46 | 步骤 2：识别输入类型 + 触发 SKILL |
| 3 | 59 | 步骤 3：识别最小必跑流程的"硬卡片" |
| 3 | 72 | 步骤 4：用户催促的反模式处理 |
| 3 | 82 | 步骤 5：自审完成声明 |
| 2 | 98 | 与其他 SKILL 的关系 |
| 2 | 109 | 反模式（本 SOP 禁止的 4 类行为） |
| 2 | 120 | 维护 |

## Inline References

| ref |
| --- |
| SKILL.md |
| ae-sdd enter <projectKey> --story <STORY-ID> |
| document-storage-skill.md |
| proposal-skill.md |
| requirement-analysis-skill.md |
| scripts/flow_violation_scan.py |
| tools/lib/document_storage.py:check_ra_prerequisites |
