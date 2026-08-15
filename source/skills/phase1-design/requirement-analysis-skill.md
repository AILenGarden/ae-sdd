---
name: requirement-analysis
description: 需求分析 SKILL — ae-sdd 的首个业务分析阶段。只回答"需求是什么、为什么、边界是什么、如何验收、风险和未决是什么、需求规模多大"，产出唯一一份随需求规模自适应的需求规格说明书（SRS），并以纯需求事实裁定规模；RA 关闭冲突并获得用户一次批准后，daemon 才冻结唯一 EngineeringRoute。当用户说"分析需求"/"从 PRD 开始"/"需求拆解"/"需求分析"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md
source_fallback_sha256: a1fb753293a1fcdbd4ede93a3c8bd4d84d0f2f11af24992798b504d2a199b34a
source_original_bytes: 16221
source_original_lines: 295
source_semantic_inventory_sha256: a3f465bdc0f4ce9e28902269056e4b5b05ca65e709e02288c0a4c13da9a2015a
---

# Requirement Analysis — 需求分析 SKILL（RA-first 首个业务阶段） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not hand-edit generated slim sections. Refresh from the fallback with `ae-sdd-build source-slim --source source --skill skills/phase1-design/requirement-analysis-skill.md --refresh`.
- Validate canonical rendered bytes with `ae-sdd-build source-slim --source source --skill skills/phase1-design/requirement-analysis-skill.md --validate`.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/requirement-analysis-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md`
- fallback_sha256: `a1fb753293a1fcdbd4ede93a3c8bd4d84d0f2f11af24992798b504d2a199b34a`
- original_lines: 295
- original_bytes: 16221
- semantic_inventory_sha256: `a1fb753293a1fcdbd4ede93a3c8bd4d84d0f2f11af24992798b504d2a199b34a`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 需求分析 SKILL — ae-sdd 的首个业务分析阶段。只回答"需求是什么、为什么、边界是什么、如何验收、风险和未决是什么、需求规模多大"，产出唯一一份随需求规模自适应的需求规格说明书（SRS），并以纯需求事实裁定规模；RA 关闭冲突并获得用户一次批准后，daemon 才冻结唯一 EngineeringRoute。当用户说"分析需求"/"从 PRD 开始"/"需求拆解"/"需求分析"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:51 📦 文档存放前置调用（横切依赖）; L3:148 第三步：适用性判定（七个条件维度）; keyword_hits: 20 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L1:6 Requirement Analysis — 需求分析 SKILL（RA-first 首个业务阶段）; L2:123 4. 分析流程; keyword_hits: 35 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | critical: G-RA-1, G-RA-2, G-RA-3, G-RA-4, G-RA-FLOW-VIOLATION; keyword_hits: 50 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | critical: ae-sdd, ae-sdd doc save, ae-sdd doc save --intent; keyword_hits: 8 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | critical: DocumentId, analysisState, contentDigest, documentId, revision, routeCandidateDigest; keyword_hits: 7 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | critical: SKILL.md, coding-skill.md, document-storage-skill.md, dr-generate-skill.md, draft.md, requirement-analysis-skill.md, story-generate-skill.md, templates/design/ra-template.md; headings: L2:51 📦 文档存放前置调用（横切依赖）; L3:192 第八步：保存 SRS; keyword_hits: 27 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | critical: .ae-sdd/tmp/{doc-id}-draft.md, SKILL.md, ae-sdd doc save, ae-sdd doc save --intent RA, ae-sdd-ra-srs/v2, applicable / not_applicable / unknown, coding-skill.md, dr-generate-skill.md, requirement-analysis-skill.md, story-generate-skill.md, templates/design/ra-template.md; inline_refs: 11; refs: .ae-sdd/tmp/{doc-id}-draft.md; SKILL.md; ae-sdd doc save; +8 more; +1 more | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 49 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 5 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry through the native `ae-sdd-build source-slim` command; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, `source_semantic_inventory_sha256`, and canonical-byte equality.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Requirement Analysis — 需求分析 SKILL（RA-first 首个业务阶段） |
| 2 | 39 | 0. 与现有 SKILL 的分工 |
| 2 | 51 | 📦 文档存放前置调用（横切依赖） |
| 3 | 55 | 写入 SOP（2 步） |
| 2 | 71 | 1. 纯需求分析立场（RA 不越界做什么） |
| 2 | 93 | 2. 上下文规则（全局选填，结论依赖时条件必需） |
| 2 | 107 | 3. 输入与启动 |
| 3 | 109 | 3.1 输入 |
| 3 | 117 | 3.2 启动条件 |
| 2 | 123 | 4. 分析流程 |
| 3 | 125 | 第一步：分离 source fact / interpretation / assumption |
| 3 | 135 | 第二步：写 SRS Core |
| 3 | 148 | 第三步：适用性判定（七个条件维度） |
| 3 | 169 | 第四步：只加载所需上下文，只展开 applicable 章节 |
| 3 | 175 | 第五步：仅针对阻断性冲突/歧义提问 |
| 3 | 181 | 第六步：校验 REQ 来源、REQ-AC 覆盖、风险和 gap |
| 3 | 188 | 第七步：纯需求规模裁定 |
| 3 | 192 | 第八步：保存 SRS |
| 2 | 202 | 5. 纯需求规模算法 |
| 2 | 225 | 6. REQ/AC/REF/GAP 与 traceability 规则 |
| 3 | 227 | 6.1 ID 契约 |
| 3 | 234 | 6.2 traceability |
| 3 | 240 | 6.3 一次最终批准 |
| 2 | 248 | 7. correction 与重入 |
| 2 | 257 | 8. 出闸条件（何时 RA 完成） |
| 2 | 275 | 9. SRS 内容合同速查 |
| 2 | 285 | 10. 常见错误（避免） |

## Inline References

| ref |
| --- |
| .ae-sdd/tmp/{doc-id}-draft.md |
| SKILL.md |
| ae-sdd doc save |
| ae-sdd doc save --intent RA |
| ae-sdd-ra-srs/v2 |
| applicable / not_applicable / unknown |
| coding-skill.md |
| dr-generate-skill.md |
| requirement-analysis-skill.md |
| story-generate-skill.md |
| templates/design/ra-template.md |
