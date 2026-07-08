---
name: dr-generate
description: DR 生成 SKILL — ae-sdd Phase 1 ② 节点（规模=大 时触发）。从 RA 需求分析文档 + PRD + 项目资产 + 产品原型（可选）生成 DR 总体设计文档，对齐 dr-template 18 章节。当用户说"生成 DR"/"写 DR"/"从 RA 生成 DR"/"DR 起草"时触发。🆕 2026-06-17 新建，与 requirement-analysis-skill / dr-review-skill / dr-update-skill 形成完整 DR 链路。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase1-design/dr-generate-skill.full.md
source_fallback_sha256: 365aadbb7005bcad3023f8731c381470dd328640276c57ffac1e5882dd7cb43a
source_original_bytes: 46206
source_original_lines: 1064
source_semantic_inventory_sha256: bce5e84fb6334335f8f9d78c485271cd715c9109c098e15793a537fdb84147e8
source_slimmer: slim_source_skills.py@2
---

# DR Generate — 从 RA 生成 DR Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase1-design/dr-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase1-design/dr-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase1-design/dr-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase1-design/dr-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase1-design/dr-generate-skill.full.md`
- fallback_sha256: `365aadbb7005bcad3023f8731c381470dd328640276c57ffac1e5882dd7cb43a`
- original_lines: 1064
- original_bytes: 46206
- semantic_inventory_sha256: `bce5e84fb6334335f8f9d78c485271cd715c9109c098e15793a537fdb84147e8`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: DR 生成 SKILL — ae-sdd Phase 1 ② 节点（规模=大 时触发）。从 RA 需求分析文档 + PRD + 项目资产 + 产品原型（可选）生成 DR 总体设计文档，对齐 dr-template 18 章节。当用户说"生成 DR"/"写 DR"/"从 RA 生成 DR"/"DR 起草"时触发。🆕 2026-06-17 新建，与 requirement-analysis-skill / dr-review-skill / dr-update-skill 形成完整 DR 链路。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:23 📦 文档存放前置调用（🔴 横切依赖）; L2:166 触发条件; L3:617 4.1 调用 save_doc 的参数; +2 more; keyword_hits: 47 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:104 整体流程; L3:474 §9 状态与业务规则（按需：跨 Story 状态机时填写）; keyword_hits: 48 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:66 标尺 1：与 RA 100% 一致（🔴 DR 不得偏离 RA）; L3:74 标尺 2：与项目资产对齐（🔴 实现必须基于现有工程能力）; L2:191 第零步：DR 准入检查（🔴 硬门禁，未通过禁止进入生成）; +2 more; keyword_hits: 71 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:464 §8 接口契约（按需：有对外暴露的接口时填写）; L4:510 §12.2 前端 Story 矩阵（待后端接口稳定后补充）; L1:620 旧式伪代码已废弃，改用 CLI：; +3 more; keyword_hits: 85 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:474 §9 状态与业务规则（按需：跨 Story 状态机时填写）; keyword_hits: 53 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:23 📦 文档存放前置调用（🔴 横切依赖）; L3:274 1.4 产出：输入清单; L3:499 §12 Story 拆分（必填 — DR 核心产出）; +4 more; keyword_hits: 92 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 34; refs: .ae-plan/; .ae-task/; .spec/iterations/; +31 more; headings: L1:621 ae-sdd doc save --intent DR --doc-id DR-001 --content-file 草稿.md --changelog-note "首次创建"; L1:644 → 实际由 ae-sdd doc save 完成，自动写入 ae-sdd-doc/DR/DR-001.md; keyword_hits: 61 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:74 标尺 2：与项目资产对齐（🔴 实现必须基于现有工程能力）; L3:82 标尺 3：实现方案决策基线（🔴 复用优先 + 成熟方案参考 + 五维质量）; L3:94 标尺 4：Story 拆分可执行（🔴 DR §12 是下游 Story 的"母版"）; +27 more; keyword_hits: 334 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L3:561 §15 发布、迁移与回滚（按需：数据迁移 / 不兼容变更时填写）; L1:621 ae-sdd doc save --intent DR --doc-id DR-001 --content-file 草稿.md --changelog-note "首次创建"; L1:645 → 自动追加 ChangeLog 行; +1 more; keyword_hits: 45 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | DR Generate — 从 RA 生成 DR Skill |
| 2 | 23 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 52 | 目标 |
| 2 | 64 | DR Generate 总则（🔴 4 条标尺，贯穿全 SKILL，违反 = 结论无效） |
| 3 | 66 | 标尺 1：与 RA 100% 一致（🔴 DR 不得偏离 RA） |
| 3 | 74 | 标尺 2：与项目资产对齐（🔴 实现必须基于现有工程能力） |
| 3 | 82 | 标尺 3：实现方案决策基线（🔴 复用优先 + 成熟方案参考 + 五维质量） |
| 3 | 94 | 标尺 4：Story 拆分可执行（🔴 DR §12 是下游 Story 的"母版"） |
| 2 | 104 | 整体流程 |
| 2 | 166 | 触发条件 |
| 2 | 177 | Plan-first 生成原则（🔴 写 DR 前硬前置） |
| 2 | 191 | 第零步：DR 准入检查（🔴 硬门禁，未通过禁止进入生成） |
| 2 | 221 | 准入检查记录 - DR-{ID} - {YYYY-MM-DD HH:mm} |
| 2 | 238 | 第一步：读取输入（继承 RA 8 维度结论） |
| 3 | 242 | 1.1 RA → DR 章节映射 |
| 3 | 255 | 1.2 提取 PRD / 产品原型信息 |
| 3 | 264 | 1.3 提取项目资产信息 |
| 3 | 274 | 1.4 产出：输入清单 |
| 2 | 298 | 第一步 bis：实现方案决策基线（🔴 4 步强制） |
| 3 | 302 | 1bis.1 实现点清单（先穷举） |
| 3 | 316 | 1bis.2 现有能力复用扫描（所有实现点必填） |
| 3 | 327 | 1bis.3 业内成熟方案参考（所有非平凡实现点必填） |
| 3 | 337 | 1bis.4 五维代码质量评估（候选方案必填） |
| 3 | 345 | 1bis.5 核心能力归属（防重复实现） |
| 3 | 355 | 1bis 出闸条件 |
| 2 | 368 | 第二步：DR 章节挖掘（按 dr-template 18 章节分维度） |
| 3 | 372 | §1 元信息（必填） |
| 3 | 382 | §2 设计目标 + 非目标（必填） |
| 3 | 391 | §3 约束承接（必填） |
| 3 | 407 | §4 关键决策（必填） |
| 4 | 411 | 决策 1：<决策点名称> |
| 4 | 430 | 决策 2、3、... |
| 3 | 434 | §5 架构概览（必填） |
| 3 | 443 | §6 关键时序（按需：跨服务交互时填写） |
| 3 | 455 | §7 数据模型（必填） |
| 3 | 464 | §8 接口契约（按需：有对外暴露的接口时填写） |
| 3 | 474 | §9 状态与业务规则（按需：跨 Story 状态机时填写） |
| 3 | 482 | §10 故障模式（按需：核心链路 / 强一致性要求时填写） |
| 3 | 490 | §11 权限、安全与审计（按需：新权限模型 / 敏感数据时填写） |
| 3 | 499 | §12 Story 拆分（必填 — DR 核心产出） |
| 4 | 503 | §12.1 后端 Story 矩阵 |
| 4 | 510 | §12.2 前端 Story 矩阵（待后端接口稳定后补充） |
| 4 | 514 | §12.3 Story 详情 |
| 4 | 529 | §12.4 依赖关系图 |
| 3 | 545 | §13 测试策略（按需：特殊测试要求时填写） |
| 3 | 553 | §14 可观测性（按需：新增核心链路时填写） |
| 3 | 561 | §15 发布、迁移与回滚（按需：数据迁移 / 不兼容变更时填写） |
| 3 | 568 | §16 风险评估（按需：明确高风险点时填写） |
| 3 | 574 | §17 未决问题（按需：待确认的决策点时填写） |
| 3 | 580 | §18 追踪 |
| 2 | 589 | 第三步：合理性自检 |
| 3 | 591 | 3.1 自检 4 维度 |
| 3 | 600 | 3.2 自检结论 |
| 2 | 612 | 第四步：写入 DR 文档 |
| 3 | 617 | 4.1 调用 save_doc 的参数 |
| 1 | 620 | 旧式伪代码已废弃，改用 CLI： |
| 1 | 621 | ae-sdd doc save --intent DR --doc-id DR-001 --content-file 草稿.md --changelog-note "首次创建" |
| 1 | 644 | → 实际由 ae-sdd doc save 完成，自动写入 ae-sdd-doc/DR/DR-001.md |
| 1 | 645 | → 自动追加 ChangeLog 行 |
| 3 | 648 | 4.2 关联性分析调用 |
| 3 | 677 | 4.3 写入前最后检查 |
| 2 | 691 | 第四步 bis：生成 DRGeneratePlan |
| 2 | 694 | DRGeneratePlan - {DR-ID} - {YYYY-MM-DD HH:mm} |
| 3 | 696 | 1. 章节填写顺序 |
| 3 | 730 | 2. 与下游 SKILL 衔接 |
| 3 | 733 | 3. 用户确认 |
| 2 | 740 | 第五步：🔴 人工审核点（双支柱呈现） |
| 3 | 744 | 5.1 支柱 1：叙述性讲解 |
| 3 | 785 | 5.2 支柱 2：对话内直接呈现 |
| 4 | 789 | 表 1：架构概览（精简版） |
| 4 | 796 | 表 2：关键决策表 |
| 4 | 803 | 图 3：架构图（Mermaid） |
| 4 | 812 | 表 4：数据模型变更清单 |
| 4 | 818 | 表 5：接口契约表 |
| 4 | 824 | 表 6：Story 矩阵 |
| 4 | 831 | 图 7：Story 依赖关系图（Mermaid） |
| 4 | 840 | 表 8：风险评估表 |
| 3 | 846 | 5.3 用户确认动作 |
| 2 | 855 | 第六步：触发下游 SKILL |
| 2 | 865 | 第七步：DR Generate 闸门全集（8 道闸） |
| 2 | 892 | 执行清单（🔴 逐项执行，24 步，对应 TodoWrite） |
| 2 | 923 | 禁止事项 |
| 2 | 942 | 与其他 SKILL 的关系 |
| 2 | 965 | 📖 人工审核主动讲解规范 — DR 节点 |
| 2 | 983 | 📋 多 Agent 编排（角色 dr-writer） |
| 3 | 987 | 角色：DR 生成 Agent（`dr-writer`） |
| 3 | 998 | 角色 Prompt 模板 |
| 1 | 1001 | 任务分配卡 |
| 2 | 1045 | 维护 |

## Inline References

| ref |
| --- |
| .ae-plan/ |
| .ae-task/ |
| .spec/iterations/ |
| SKILL.md |
| SKILL.md §实现方案决策基线 |
| SKILL.md §📖 人工审核主动讲解规范 |
| _iter_ra_files/_find_prd_files |
| ae-sdd assets outline |
| ae-sdd assets outline --project <projectKey> |
| ae-sdd assets query "<name>" --project <projectKey> |
| ae-sdd assets query "<{X}>" |
| ae-sdd assets read |
| ae-sdd assets read dr-generate --project <projectKey> |
| ae-sdd assets section <§X.Y> --project <projectKey> |
| ae-sdd doc resolve --intent DR --doc-id {drId} |
| ae-sdd doc save |
| ae-sdd doc save --intent DR |
| ae-sdd doc save --intent DR --doc-id {drId} --content-file 草稿.md |
| ae-sdd doc save --intent DR ... |
| ae-sdd doc save --intent RA_GENERATE_PLAN --doc-id {raId} --content-file 草稿.md |
| constraints/assets/RA/PRD |
| design/ |
| document-storage-skill.md |
| dr-generate-skill.md |
| dr-review-skill.md |
| dr-update-skill.md |
| get_constraints/get_assets |
| project-assets-update-skill.md |
| requirement-analysis-skill.md |
| skills/orchestration/ae-sdd-skill.md |
| standards/constraints/ |
| story-generate-skill.md |
| templates/design/dr-template.md |
| {DR-ID}-DR-WriterReport.md |
