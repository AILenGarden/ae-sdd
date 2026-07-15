---
name: document-storage
description: 文档存放横切 SKILL — 所有 SKILL 写入文档前必调。提供 ae-sdd-doc/ 统一目录、8 类流程分类、迭代目录、版本号、关联性分析、gitignore 自动生成、存量迁移能力。当 SKILL 涉及"存放/保存/写入文档"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md
source_fallback_sha256: c04a2c28e1ff66667617291df9d06bf262b8922c8ec01a78534520ff93025b15
source_original_bytes: 75515
source_original_lines: 1351
source_semantic_inventory_sha256: c4052ce157ba3dbf8c6074ef8aebcf6fc9e6b506a84f06e0c1f171af4fa6016f
source_slimmer: slim_source_skills.py@2
---

# Document Storage — 文档存放标准 Skill（AE 体系横切依赖） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/document-storage-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md`
- fallback_sha256: `c04a2c28e1ff66667617291df9d06bf262b8922c8ec01a78534520ff93025b15`
- original_lines: 1351
- original_bytes: 75515
- semantic_inventory_sha256: `c4052ce157ba3dbf8c6074ef8aebcf6fc9e6b506a84f06e0c1f171af4fa6016f`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 文档存放横切 SKILL — 所有 SKILL 写入文档前必调。提供 ae-sdd-doc/ 统一目录、8 类流程分类、迭代目录、版本号、关联性分析、gitignore 自动生成、存量迁移能力。当 SKILL 涉及"存放/保存/写入文档"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L3:397 🆕 4.0 CLI 入口（v3.7.2 激活，推荐 LLM 使用）; L3:951 7.3 调用时机; L2:1030 9. 横切调用规范; +6 more; keyword_hits: 77 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:630 5. 重入流程与文档演进; L3:878 6.5 choose_iteration() 流程; keyword_hits: 41 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:565 4.10 intent 枚举表（🔴 save_doc / resolve_path 的 intent 参数必须取自此表）; L2:1137 11. 禁止事项; keyword_hits: 92 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L2:393 4. 动态定位 API 契约（🆕 唯一 SSOT）; L3:397 🆕 4.0 CLI 入口（v3.7.2 激活，推荐 LLM 使用）; L3:423 4.1 核心 API：`resolve_path()`; +10 more; keyword_hits: 80 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:707 5.2 文档状态码与生命周期; L2:1205 附录 A. PRD 级 `state.json` schema（参考）; keyword_hits: 79 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L1:6 Document Storage — 文档存放标准 Skill（AE 体系横切依赖）; L2:48 1. 文档分类与目录结构; L3:50 1.1 文档分类（8 类）; +15 more; keyword_hits: 387 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 120; refs: # ae-sdd generated docs\nae-sdd-doc/; ...CodeReview.md; ...CodingReport.md; +117 more; keyword_hits: 238 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | keyword_hits: 190 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L3:535 4.7 ChangeLog 读取 API：`get_changelog()`; L2:762 6. ChangeLog 与迭代关联; L3:764 6.1 ChangeLog 机制（🔴 强制）; +2 more; keyword_hits: 114 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Document Storage — 文档存放标准 Skill（AE 体系横切依赖） |
| 2 | 36 | 0. 目标 |
| 2 | 48 | 1. 文档分类与目录结构 |
| 3 | 50 | 1.1 文档分类（8 类） |
| 3 | 70 | 1.2 统一根目录 |
| 3 | 96 | 1.3 路径模板总表 |
| 3 | 139 | 1.4 资产类路径模板（🔴 资产路径 SSOT） |
| 3 | 190 | 1.5 迭代目录结构 |
| 3 | 224 | 1.6 旧路径兼容层（⚠️ deprecated） |
| 2 | 242 | 2. 命名与版本号规则 |
| 3 | 244 | 2.1 命名模板（统一格式） |
| 3 | 262 | 2.2 版本号使用规则（🔴 SSOT，与代码一致） |
| 3 | 281 | 2.3 版本号含义 |
| 3 | 289 | 2.4 版本号递增 SOP（🔴 限定事件类报告） |
| 2 | 307 | 3. 工程解耦定位原则（🆕 🔴 2026-06-10 硬约束） |
| 3 | 311 | 3.1 五维定位模型 + WorkItem 隔离键（🆕 v4.1 四维→五维，新增"业务线根"；原 v3.4.0 为四维） |
| 3 | 336 | 3.2 动态定位算法 |
| 3 | 355 | 3.3 项目资产依赖（🔴 资产路径单一权威源 — 🆕 v4.1） |
| 3 | 383 | 3.4 硬约束 |
| 2 | 393 | 4. 动态定位 API 契约（🆕 唯一 SSOT） |
| 3 | 397 | 🆕 4.0 CLI 入口（v3.7.2 激活，推荐 LLM 使用） |
| 3 | 423 | 4.1 核心 API：`resolve_path()` |
| 3 | 468 | 4.2 工具 API（定位原语） |
| 4 | 479 | 4.2.1 原生 StoryName 解析与绑定 |
| 3 | 493 | 4.3 统一保存 API：`save_doc()` |
| 3 | 509 | 4.4 索引维护 API：`update_storing_index()` |
| 3 | 518 | 4.5 关联性 API |
| 3 | 528 | 4.6 版本查询 API：`get_latest_version()` |
| 3 | 535 | 4.7 ChangeLog 读取 API：`get_changelog()` |
| 3 | 542 | 4.8 .gitignore 维护 API：`check_and_update_gitignore()` |
| 3 | 553 | 4.9 存量迁移 API：`migrate_old_docs()` |
| 3 | 565 | 4.10 intent 枚举表（🔴 save_doc / resolve_path 的 intent 参数必须取自此表） |
| 3 | 608 | 4.11 错误码 |
| 2 | 630 | 5. 重入流程与文档演进 |
| 3 | 634 | 5.1 重入 SOP（5 步判定） |
| 3 | 707 | 5.2 文档状态码与生命周期 |
| 3 | 741 | 5.3 交叉引用规则 |
| 2 | 762 | 6. ChangeLog 与迭代关联 |
| 3 | 764 | 6.1 ChangeLog 机制（🔴 强制） |
| 1 | 788 | ChangeLog - {doc-id} |
| 3 | 811 | 6.2 迭代目录命名 |
| 3 | 817 | 6.3 关联性算法（🔴 唯一权威定义） |
| 4 | 821 | 业务关联（B1-B4，任一命中=1） |
| 4 | 845 | 逻辑关联（L1-L4，任一命中=1） |
| 3 | 869 | 6.4 关联等级判定 |
| 3 | 878 | 6.5 choose_iteration() 流程 |
| 3 | 897 | 6.6 业务/逻辑标签采集 |
| 2 | 924 | 7. .gitignore 自动生成（🔴 强制） |
| 3 | 928 | 7.1 check_and_update_gitignore() 行为 |
| 1 | 932 | ae-sdd generated docs |
| 1 | 938 | ae-sdd generated docs |
| 3 | 942 | 7.2 幂等性保证 |
| 3 | 951 | 7.3 调用时机 |
| 2 | 963 | 8. 存量迁移 |
| 3 | 967 | 8.1 migrate_old_docs() 行为 |
| 3 | 983 | 8.2 MigrationReport 格式 |
| 1 | 986 | Migration Report - {projectKey} - {YYYY-MM-DD} |
| 2 | 988 | 扫描结果 |
| 2 | 997 | 迁移计划 |
| 2 | 1004 | 注意事项 |
| 3 | 1010 | 8.3 默认不执行 + 用户确认 |
| 2 | 1030 | 9. 横切调用规范 |
| 3 | 1032 | 9.1 调用矩阵（🔴 单一权威表） |
| 3 | 1053 | 9.2 标准调用段（🔴 各 SKILL 必加） |
| 2 | 1058 | 📦 文档存放前置调用（🔴 横切依赖） |
| 3 | 1074 | 调用示例（按本 SKILL 实际文档类型填） |
| 3 | 1082 | 9.3 调用时机（🔴 必在文档落地前调用） |
| 3 | 1104 | 9.4 不调用本 SKILL 的反模式 |
| 2 | 1118 | 10. 与其他 SKILL 的衔接 |
| 2 | 1137 | 11. 禁止事项 |
| 2 | 1157 | 12. 执行清单 |
| 2 | 1180 | 13. 维护 |
| 2 | 1205 | 附录 A. PRD 级 `state.json` schema（参考） |

## Inline References

| ref |
| --- |
| # ae-sdd generated docs\nae-sdd-doc/ |
| ...CodeReview.md |
| ...CodingReport.md |
| ...Compliance-r1.md |
| ...GeneratePlan-r1.md |
| ...Impact-r1.md |
| ...ImplPlan.md |
| ...Report.md |
| ...ReviewCompare-v1-to-v2.md |
| ...StoryReviewReport-r1.md |
| ...TaskReview-r1.md |
| ...TaskWriterReport-r1.md |
| ...TestCaseReview-r1.md |
| ...UpdatePlan-r1.md |
| ...WriterReport-r1.md |
| .ae-plan/ |
| .ae-sdd/assets/{key}.*.assets.md |
| .ae-task/ |
| .auto-engineering/*/state.json |
| .auto-engineering/{PRD-ID}/state.json |
| .auto-engineering/{PRD-ID}/state.md |
| .auto-engineering/{PRD-ID}/summary.md |
| .auto-engineering/{WORKITEM-ID}/state.json |
| .md |
| .spec/ |
| .spec/iterations/ |
| .spec/iterations/*/*.md |
| / |
| 0 \\| 1 |
| BUG-LIFE-001-CodingPlan.md |
| BUG-LIFE-001-testcase.md |
| BUG-LIFE-001.md |
| Coding/STORY-004-BE/ |
| Coding/Story-004/ |
| DR-LIFE-001.md |
| PRD-001.md |
| PRD-LIFE-001.md |
| RA-LIFE-001.md |
| SKILL.md |
| SKILL.md §1.2 |
| SKILL.md §1.3 |
| STORY-001-BE-CodingReport.md |
| STORY-001-BE-Proposal-1.md |
| STORY-001-BE-ReviewCompare-v1-to-v2.md |
| STORY-001-BE-StoryReviewReport-r1.md |
| STORY-001-BE.md |
| STORY-LIFE-001-BE-Supplement.md |
| STORY-LIFE-001-BE.md |
| StoryName + .md |
| Task/BUG-LIFE-001/TASK-001.md |
| Task/{story_id} |
| \ |
| ae-sdd doc |
| ae-sdd doc finalize --path P --intent X |
| ae-sdd doc resolve --intent X |
| ae-sdd doc save --intent X --content-file F |
| ae-sdd memory enter/write/exit |
| ae-sdd runtime compact |
| ae-sdd state bind-story-doc |
| ae-sdd state bind-story-doc --work-item W --story S --story-name N |
| ae-sdd state new ... --story-name N |
| ae-sdd state prd-complete |
| ae-sdd state prd-init |
| ae-sdd state write |
| ae-sdd-doc/ |
| ae-sdd-doc/CR/ |
| ae-sdd-doc/Coding/ |
| ae-sdd-doc/DR/ |
| ae-sdd-doc/PRD/ |
| ae-sdd-doc/RA/ |
| ae-sdd-doc/STORING.md |
| ae-sdd-doc/Story/ |
| ae-sdd-doc/Task/ |
| ae-sdd-doc/Test/ |
| ae-sdd-doc/iterations/{date}/{DocType}/ |
| ae-sdd-doc/{DocType}/{WORKITEM-ID}/{doc-id}-v{N}-r{M}.md |
| ae-sdd-doc/{DocType}/{doc-id}.md |
| ae-sdd-update-skill.md |
| archive/Proposal-001.md |
| archive/{DOC-ID}.md |
| archive/{date}/ |
| assets.md |
| assets.md §1 docWorkspacePath |
| assets.md §1 gitPath |
| assets/{projectKey}/ |
| code-review-skill.md |
| coding-report-skill.md |
| coding-skill.md |
| constraints/ |
| d:\Item\icec-cloud-boss\ae-sdd-doc\ |
| d:\Item\icec-cloud-boss\icec-cloud-life-cs-service |
| design/ |
| design/dr/{projectKey}/*.md |
| design/story/be/*.md |
| design/story/be/coding/*/*.md |
| design/story/be/task/*/*.md |
| design/testcase/be/*/*.md |
| docWorkspace/constraints/ |
| document-storage-skill.md |
| document-storage-skill.md §9.1 |
| dr-generate-skill.md |
| dr-update-skill.md |
| icec-cloud-boss.assets.md |
| icec-cloud-boss.update-log.md |
| intent=TASK/CODING_PLAN/CODING_REPORT/TESTCASE/TEST_REPORT/CODE_REVIEW/... |
| intent=TASK_SMALL/PLAN_MICRO |
| iterations/{date}/DR/ |
| life.assets.md |
| project-assets-schema.md |
| project-assets-update-skill.md |
| project-assets-update-skill.md §3 生成动作 |
| proposal-skill.md |
| requirement-analysis-skill.md |
| save_doc(intent="CODING_REPORT"/"TRACE_MATRIX", workItemId, storyId?, version={v, r}) |
| save_doc(intent="PRD"/"RA"/"RA_GENERATE_PLAN"/"RA_IMPACT") |
| save_doc(intent="STORY"/"STORY_SUPPLEMENT"/"STORY_GENERATE_PLAN", version=None) |
| save_doc(intent="TASK"/"TASK_SUPPLEMENT"/"CODING_PLAN", workItemId, storyId?, version=None) |
| skills/ae-sdd/assets/{projectKey}/{projectKey}.assets.md |
| standards/thinking/be-coding-thinking-engine.md |
| story-generate-skill.md |
