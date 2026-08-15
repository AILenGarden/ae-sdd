---
name: ai-agent-self-audit-checklist
description: AI Agent 任务开始前自审清单 SOP（🔴 强制，每任务必跑）。覆盖 5 步骤（识别任务类型 / 识别输入类型 / 识别最小必跑流程硬卡片 / 用户催促反模式处理 / 自审完成声明）。🆕 2026-06-27 新建，解决"AI Agent 撞到出文档类任务直接动笔跳过 RA skill"的系统性违规（实测案例：2026-06-27 AI Agent 直接读 PDF 出 36KB proposal 未走 RA skill 完整 7 步）。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md
source_fallback_sha256: 258889aa6032a6da08b7b9d056844a03617db34985bd7d961a6532f910fb9fe0
source_original_bytes: 7842
source_original_lines: 131
source_semantic_inventory_sha256: c50517ccd7f92ba87ae76d35098d19e31e018b21fbaa6001011da3c4b6e33d9b
---

# AI Agent 自审清单 SOP — 任务开始前强制自审 Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not hand-edit generated slim sections. Refresh from the fallback with `ae-sdd-build source-slim --source source --skill skills/cross-cutting/ai-agent-self-audit-checklist.md --refresh`.
- Validate canonical rendered bytes with `ae-sdd-build source-slim --source source --skill skills/cross-cutting/ai-agent-self-audit-checklist.md --validate`.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/ai-agent-self-audit-checklist.md`
- fallback: `skill-fallbacks/skills/cross-cutting/ai-agent-self-audit-checklist.full.md`
- fallback_sha256: `258889aa6032a6da08b7b9d056844a03617db34985bd7d961a6532f910fb9fe0`
- original_lines: 131
- original_bytes: 7842
- semantic_inventory_sha256: `258889aa6032a6da08b7b9d056844a03617db34985bd7d961a6532f910fb9fe0`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: AI Agent 任务开始前自审清单 SOP（🔴 强制，每任务必跑）。覆盖 5 步骤（识别任务类型 / 识别输入类型 / 识别最小必跑流程硬卡片 / 用户催促反模式处理 / 自审完成声明）。🆕 2026-06-27 新建，解决"AI Agent 撞到出文档类任务直接动笔跳过 RA skill"的系统性违规（实测案例：2026-06-27 AI Agent 直接读 PDF 出 36KB proposal 未走 RA skill 完整 7 步）。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:19 触发条件; L3:46 步骤 2：识别输入类型 + 触发 SKILL; keyword_hits: 17 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L3:59 步骤 3：识别最小必跑流程的"硬卡片"; keyword_hits: 18 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | critical: G-RA-1, G-RA-4, G-RA-PLAN; headings: L2:108 反模式（本 SOP 禁止的 4 类行为）; keyword_hits: 18 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | critical: ae-sdd, ae-sdd doc save --intent, ae-sdd enter, ae-sdd flow-violation-scan; keyword_hits: 4 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | critical: analysisState, projectKey; keyword_hits: 5 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | critical: AGENTS.md, SKILL.md, document-storage-skill.md, proposal-skill.md, requirement-analysis-skill.md; keyword_hits: 19 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | critical: SKILL.md, ae-sdd doc save --intent RA, ae-sdd enter <projectKey> --story <STORY-ID>, ae-sdd flow-violation-scan, crates/ae-sdd-resources/src/document.rs (RA 前置检查), document-storage-skill.md, proposal-skill.md, requirement-analysis-skill.md; inline_refs: 8; refs: SKILL.md; ae-sdd doc save --intent RA; ae-sdd enter <projectKey> --story <STORY-ID>; +5 more; +1 more | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 20 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 2 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry through the native `ae-sdd-build source-slim` command; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, `source_semantic_inventory_sha256`, and canonical-byte equality.
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
| 3 | 81 | 步骤 5：自审完成声明 |
| 2 | 97 | 与其他 SKILL 的关系 |
| 2 | 108 | 反模式（本 SOP 禁止的 4 类行为） |
| 2 | 119 | 维护 |

## Inline References

| ref |
| --- |
| SKILL.md |
| ae-sdd doc save --intent RA |
| ae-sdd enter <projectKey> --story <STORY-ID> |
| ae-sdd flow-violation-scan |
| crates/ae-sdd-resources/src/document.rs (RA 前置检查) |
| document-storage-skill.md |
| proposal-skill.md |
| requirement-analysis-skill.md |
