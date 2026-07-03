---
name: requirement-analysis
description: 需求分析 SKILL — ae-sdd Phase 1 起点。从 PRD/Issue/对话需求生成需求分析（RA）文档：RequirementAnalysisModel 12 维决策 + 需求风险预判 + 8 维度并行挖掘 + 实现视角七要素（数据源/数据流/定义/复用/成本反驳/开发疑问/DR交接）+ 5 问自检 + 缺口管理 + 5 维规模裁定 + 16 道 RA 门禁 + 路由决策。当用户说"分析需求"/"从 PRD 开始"/"需求拆解"/"需求分析"时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md
source_fallback_sha256: d894cdfa1f71f748be02f34bae14810531ff9a99876f19d72b1311e9854c3e16
source_original_bytes: 114217
source_original_lines: 2001
source_semantic_inventory_sha256: 5235504f16f83ad1f8ab1ea71d708d3e74ce982b4d408b994fb890b1a25fcd8a
source_slimmer: slim_source_skills.py@2
---

# Requirement Analysis — 需求分析 SKILL（ae-sdd Phase 1 起点） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/requirement-analysis-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/requirement-analysis-skill.full.md`
- fallback_sha256: `d894cdfa1f71f748be02f34bae14810531ff9a99876f19d72b1311e9854c3e16`
- original_lines: 2001
- original_bytes: 114217
- semantic_inventory_sha256: `5235504f16f83ad1f8ab1ea71d708d3e74ce982b4d408b994fb890b1a25fcd8a`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 需求分析 SKILL — ae-sdd Phase 1 起点。从 PRD/Issue/对话需求生成需求分析（RA）文档：RequirementAnalysisModel 12 维决策 + 需求风险预判 + 8 维度并行挖掘 + 实现视角七要素（数据源/数据流/定义/复用/成本反驳/开发疑问/DR交接）+ 5 问自检 + 缺口管理 + 5 维规模裁定 + 16 道 RA 门禁 + 路由决策。当用户说"分析需求"/"从 PRD 开始"/"需求拆解"/"需求分析"时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:39 📦 文档存放前置调用（🔴 横切依赖）; L2:87 🧠 阶段记忆强制调用（🔴 横切依赖）; L2:222 触发条件; +3 more; keyword_hits: 150 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L1:6 Requirement Analysis — 需求分析 SKILL（ae-sdd Phase 1 起点）; L2:87 🧠 阶段记忆强制调用（🔴 横切依赖）; L2:308 整体流程; +17 more; keyword_hits: 235 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:26 🟠 门禁强度声明（v3.5.11 AA 诚实降级）; L3:129 标尺 1：穷举优于抽样（🔴 "覆盖所有"必须先有穷举清单）; L3:150 标尺 3：冲突显性化（🔴 冲突必须摆到台面讨论）; +6 more; keyword_hits: 247 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L1:6 Requirement Analysis — 需求分析 SKILL（ae-sdd Phase 1 起点）; L4:842 H.5.3 配套工具调用; L4:898 H.6.3 配套工具调用; keyword_hits: 163 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L1:6 Requirement Analysis — 需求分析 SKILL（ae-sdd Phase 1 起点）; L3:618 阶段 E.5：状态变更衍生规则强制追问（🔴 状态机需求必跑）; L3:725 阶段 G.5：状态变更衍生 AC 强制覆盖（🔴 状态机需求必跑）; +2 more; keyword_hits: 176 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:39 📦 文档存放前置调用（🔴 横切依赖）; L3:60 本 SKILL 产出文档 × intent 对照; L3:186 RAModel 决策记录模板（RA §0.5 必填）; +6 more; keyword_hits: 175 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 77; refs: .ae-plan/; .ae-sdd/tmp/RA-001-draft.md; .ae-sdd/tmp/{doc-id}-draft.md; +74 more; keyword_hits: 59 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:186 RAModel 决策记录模板（RA §0.5 必填）; L2:189 §0.5 RequirementAnalysisModel 决策记录; L3:272 2.5 实现视角七要素 SOP（G-RA-6）; +22 more; keyword_hits: 390 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L3:618 阶段 E.5：状态变更衍生规则强制追问（🔴 状态机需求必跑）; L3:725 阶段 G.5：状态变更衍生 AC 强制覆盖（🔴 状态机需求必跑）; L3:860 阶段 H.6：跨域级联效应 Checklist（🔴 状态变更类需求必跑）; keyword_hits: 131 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Requirement Analysis — 需求分析 SKILL（ae-sdd Phase 1 起点） |
| 2 | 26 | 🟠 门禁强度声明（v3.5.11 AA 诚实降级） |
| 2 | 39 | 📦 文档存放前置调用（🔴 横切依赖） |
| 3 | 43 | 写入 SOP（3 步） |
| 3 | 60 | 本 SKILL 产出文档 × intent 对照 |
| 2 | 87 | 🧠 阶段记忆强制调用（🔴 横切依赖） |
| 2 | 113 | 0. 目标 |
| 2 | 127 | 总则（🔴 4 条标尺，贯穿全 SKILL，违反 = 结论无效） |
| 3 | 129 | 标尺 1：穷举优于抽样（🔴 "覆盖所有"必须先有穷举清单） |
| 3 | 135 | 标尺 2：证据优于假设（🔴 每条结论可追溯到事实来源） |
| 3 | 150 | 标尺 3：冲突显性化（🔴 冲突必须摆到台面讨论） |
| 3 | 157 | 标尺 4：缺口不掩盖（🔴 未确认问题必须列入"未决问题"） |
| 2 | 165 | 🔴 RequirementAnalysisModel（12 维需求分析决策模型） |
| 3 | 169 | RAModel 12 维 |
| 3 | 186 | RAModel 决策记录模板（RA §0.5 必填） |
| 2 | 189 | §0.5 RequirementAnalysisModel 决策记录 |
| 3 | 203 | 需求风险预判（必须先于 8 维度挖掘） |
| 2 | 222 | 触发条件 |
| 2 | 237 | Plan-first 生成原则（🔴 写 RA 前硬前置） |
| 2 | 249 | RAGeneratePlan - {RA-ID} - {YYYY-MM-DD HH:mm} |
| 3 | 251 | 1. 输入识别 |
| 3 | 257 | 2. 8 维度挖掘 SOP |
| 3 | 272 | 2.5 实现视角七要素 SOP（G-RA-6） |
| 3 | 281 | 3. 5 问自检清单（每条结论） |
| 3 | 288 | 4. 缺口管理策略 |
| 3 | 294 | 5. 规模裁定算法 |
| 3 | 299 | 6. 与下游 SKILL 衔接 |
| 2 | 308 | 整体流程 |
| 2 | 354 | 第 -1 步：需求来源识别 |
| 3 | 358 | -1.1 输入类型判定子流程 |
| 3 | 369 | -1.2 多轮对话 SOP |
| 3 | 385 | -1.3 输入沉淀模板 |
| 3 | 395 | -1.4 第 -1 步产出 |
| 2 | 398 | 需求来源识别记录 |
| 2 | 415 | 第零步：RA 准入检查（🔴 硬门禁，未通过禁止进入挖掘） |
| 2 | 437 | RA 准入检查记录 - {RA-ID} - {YYYY-MM-DD HH:mm} |
| 2 | 456 | 第 0.5 步：RAModel 决策 + 需求风险预判（🔴 硬门禁） |
| 3 | 460 | 0.5.1 RAModel 执行 SOP |
| 3 | 468 | 0.5.2 需求风险预判表（RA §0.6 必填） |
| 2 | 471 | §0.6 需求风险预判 |
| 3 | 485 | 0.5.3 出闸条件 |
| 2 | 497 | 第一步：8 维度并行挖掘 |
| 3 | 503 | 阶段 A：角色分析（→ RA §2） |
| 3 | 528 | 阶段 B：场景分析（→ RA §3） |
| 3 | 547 | 阶段 C：业务流程（→ RA §4） |
| 3 | 573 | 阶段 D：数据要素（→ RA §5） |
| 3 | 598 | 阶段 E：业务规则与约束（→ RA §6） |
| 3 | 618 | 阶段 E.5：状态变更衍生规则强制追问（🔴 状态机需求必跑） |
| 4 | 626 | E.5.1 衍生规则强制追问 SOP |
| 4 | 641 | E.5.2 衍生规则登记表（🔴 RA §6.5 必填） |
| 2 | 644 | 衍生规则登记表（RA §6.5） |
| 4 | 667 | E.5.3 衍生规则编号约定 |
| 3 | 675 | 阶段 F：设计方向论证（→ RA §7） |
| 3 | 699 | 阶段 G：验收标准雏形（→ RA §8） |
| 3 | 725 | 阶段 G.5：状态变更衍生 AC 强制覆盖（🔴 状态机需求必跑） |
| 4 | 733 | G.5.1 衍生 AC 强制覆盖 SOP |
| 4 | 748 | G.5.2 衍生 AC 登记表（🔴 RA §8.5 必填） |
| 2 | 751 | 衍生 AC 登记表（RA §8.5） |
| 4 | 771 | G.5.3 衍生 AC × 主 AC 覆盖率计算 |
| 2 | 774 | 衍生覆盖率（RA §8.6） |
| 3 | 789 | 阶段 H：隐性假设与验证（→ RA §9） |
| 3 | 807 | 阶段 H.5：通用业务模式 Checklist（🔴 必跑穷举，不许跳过） |
| 4 | 814 | H.5.1 六大通用业务模式 + 衍生影响穷举 |
| 4 | 825 | H.5.2 业务模式匹配表（🔴 RA 文档必填章节） |
| 2 | 828 | 业务模式匹配表（RA §9-bis） |
| 4 | 842 | H.5.3 配套工具调用 |
| 1 | 845 | 业务模式匹配表自动生成（伪代码） |
| 3 | 860 | 阶段 H.6：跨域级联效应 Checklist（🔴 状态变更类需求必跑） |
| 4 | 868 | H.6.1 跨域级联 5 问（🔴 每条必答，答不出 → 🔴 缺口） |
| 4 | 878 | H.6.2 跨域级联效应表（🔴 RA §9-ter 必填） |
| 2 | 881 | 跨域级联效应表（RA §9-ter） |
| 4 | 898 | H.6.3 配套工具调用 |
| 1 | 901 | 跨域级联效应反查（伪代码） |
| 4 | 923 | H.6.4 与 §阶段 E/G 的联动 |
| 3 | 931 | 阶段 H 与下游维度的强关联（2026-06-23 强化版总图） |
| 2 | 949 | 第一步 ter：实现视角七要素（🔴 G-RA-6 机器校验） |
| 3 | 955 | I1 数据源清单 |
| 3 | 971 | I2 数据流链路 |
| 3 | 985 | I3 术语 / 定义 / 不变量 |
| 3 | 997 | I4 现有实现 / 复用证据 |
| 3 | 1008 | I5 高成本 / 难实现设计反驳 |
| 3 | 1018 | I6 开发者疑问答复矩阵 |
| 3 | 1029 | I7 DR 生成交接包 |
| 2 | 1044 | 第一步 bis：每条结论 5 问自检 |
| 3 | 1049 | 5 问清单 |
| 3 | 1059 | 5 问自检记录模板 |
| 2 | 1062 | 5 问自检记录 - {RA-ID} - {YYYY-MM-DD HH:mm} |
| 2 | 1077 | 第二步：缺口汇总 + 循环迭代 |
| 3 | 1081 | 2.1 缺口分级标准 |
| 3 | 1090 | 2.2 缺口处理方式 |
| 3 | 1098 | 2.3 循环迭代算法 |
| 3 | 1132 | 2.4 5 轮迭代轮次 |
| 3 | 1142 | 2.5 缺口管理产出 |
| 2 | 1145 | 缺口管理记录 - {RA-ID} - {YYYY-MM-DD HH:mm} |
| 2 | 1163 | 第三步：写入 RA 文档 |
| 3 | 1167 | 3.1 写入 SOP |
| 3 | 1183 | 3.2 关联性分析调用 |
| 1 | 1232 | 返回：{ date: "2026-06-15", strength: "strong"/"medium"/"none", reasoning: "..." } |
| 1 | 1233 | 强/中关联 → 直接归入 |
| 1 | 1234 | 无关联 → 强制问用户 |
| 3 | 1237 | 3.3 写入前最后检查 |
| 2 | 1248 | 第三步 bis：生成 RAGeneratePlan |
| 2 | 1251 | RAGeneratePlan - {RA-ID} - {YYYY-MM-DD HH:mm} |
| 3 | 1253 | 1. 输入识别 |
| 3 | 1258 | 2. 8 维度挖掘结论 |
| 3 | 1270 | 3. 5 问自检通过率 |
| 3 | 1275 | 4. 缺口管理结果 |
| 3 | 1281 | 5. 规模裁定结果 |
| 3 | 1286 | 6. 与下游 SKILL 衔接 |
| 3 | 1291 | 7. 未解决问题清单 |
| 2 | 1297 | 第四步：🔴 人工审核点（双支柱呈现） |
| 3 | 1301 | 4.1 支柱 1：叙述性讲解 |
| 3 | 1337 | 4.2 支柱 2：对话内直接呈现 |
| 4 | 1341 | 表 1：角色 × 需求 × 效果矩阵 |
| 4 | 1348 | 表 2：场景清单 |
| 4 | 1356 | 图 3：状态机图（Mermaid） |
| 4 | 1364 | 表 4：5 维规模评分 |
| 4 | 1374 | 表 5：路由决策 |
| 4 | 1380 | 表 6：缺口清单 |
| 3 | 1387 | 4.3 用户确认动作 |

## Inline References

| ref |
| --- |
| .ae-plan/ |
| .ae-sdd/tmp/RA-001-draft.md |
| .ae-sdd/tmp/{doc-id}-draft.md |
| .ae-task/ |
| .spec/iterations/ |
| D:\al-agent-workspace\ae-sdd-update-doc\history\2026-06-26-ae-sdd自动化程度分析与修订建议书.md |
| SKILL.md |
| ae-sdd assets outline |
| ae-sdd assets outline --project <projectKey> |
| ae-sdd assets query |
| ae-sdd assets query "<componentName>" |
| ae-sdd assets query "<componentName>" --project <projectKey> |
| ae-sdd assets query "<method>" |
| ae-sdd assets query "<moduleName>" |
| ae-sdd assets query "<name>" --project <projectKey> |
| ae-sdd assets query "<tableName>" |
| ae-sdd assets query "<tableName>" --project <projectKey> |
| ae-sdd assets query "<{X}>" |
| ae-sdd assets query "<关键词>" |
| ae-sdd assets query "<字段名>" |
| ae-sdd assets query "X" |
| ae-sdd assets query "aggregate" |
| ae-sdd assets query "cache" |
| ae-sdd assets query "event" |
| ae-sdd assets query "mq" |
| ae-sdd assets query "角色" |
| ae-sdd assets read |
| ae-sdd assets read requirement-analysis --project <projectKey> |
| ae-sdd assets section <§X.Y> --project <projectKey> |
| ae-sdd assets section §6.X |
| ae-sdd doc resolve --intent CODING_PLAN |
| ae-sdd doc resolve --intent DR |
| ae-sdd doc resolve --intent ISSUE --doc-id {issueId} |
| ae-sdd doc resolve --intent PRD --doc-id {prdId} |
| ae-sdd doc resolve --intent PRD/ISSUE --doc-id {id} |
| ae-sdd doc resolve --intent RA --doc-id {raId} |
| ae-sdd doc resolve --intent STORY |
| ae-sdd doc resolve --intent TASK |
| ae-sdd doc resolve --intent {对应intent} |
| ae-sdd doc save |
| ae-sdd doc save --intent ISSUE --doc-id {issueId} ... |
| ae-sdd doc save --intent PRD --doc-id {prdId} ... |
| ae-sdd doc save --intent PRD/ISSUE --doc-id {id} |
| ae-sdd doc save --intent RA --doc-id {raId} |
| ae-sdd doc save --intent RA --doc-id {raId} ... |
| ae-sdd doc save --intent RA ... |
| ae-sdd doc save --intent RA_GENERATE_PLAN --doc-id {raId} --version "v1-r1" ... |
| ae-sdd doc save --intent RA_IMPACT --doc-id {raId} |
| ae-sdd doc save --intent RA_IMPACT --doc-id {raId} --version "v1-r1" ... |
| ae-sdd doc save --intent RA_REVERSE_ISSUES --doc-id {raId} |
| ae-sdd doc save --intent RA_REVERSE_ISSUES --doc-id {raId} ... |
| ae-sdd run dr-review-skill |
| ae-sdd update-check |
| ae-sdd-doc/RA/RA-001.md |
| agent-orchestration-skill.md |
| coding-skill.md |
| design/ |
| document-storage-skill.md §9.1 |
| dr-generate-skill.md |
| dr-review-skill.md |
| project-assets-update-skill.md |
| project-assets-update-skill.md §G |
| requirement-analysis-skill.md |
| review-loop-skill.md |
| scripts/ra_depth_scan.py |
| scripts/ra_implementation_scan.py |
| source → ingress(API/event/job) → domain/service → persistence/cache/MQ → output/observability |
| src/.../Class.method |
| standards/constraints/ |
| story-generate-skill.md |
| story-update-skill.md |
| task-generate-skill.md |
| templates/design/issue-template.md |
| templates/design/prd-template.md |
| templates/design/ra-template.md |
| tools/lib/document_storage.py:check_ra_prerequisites() |
| tools/lib/gates.py |
