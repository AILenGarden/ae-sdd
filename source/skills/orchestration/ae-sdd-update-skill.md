---
name: ae-sdd-update
description: 规范各 SKILL 的内容边界与维护规则。ae-sdd-skill 退守"流程编排"（流程怎么走、节点间如何流转），各子 SKILL 负责"环节内具体规则"（每一步具体怎么做、出错怎么处理）。当用户新增/修改任何 AE 相关 SKILL 时，先查阅本 SKILL 确认内容应放在哪个文件，避免在错误位置撰写或重复堆积。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md
source_fallback_sha256: 82cb9f3448e0053a770e37953dfe6d9761234613f62b7cc417bf0c57ce606745
source_original_bytes: 80310
source_original_lines: 942
source_semantic_inventory_sha256: b57e3cf5ba759e6d3d61ab1286b149f348fee727a0d7e484c2f5c1d397866bc1
source_slimmer: slim_source_skills.py@2
---

# Auto Engineering Update — SKILL 边界维护规范 Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md`, not this slim entry.

## Summary

- source: `skills/orchestration/ae-sdd-update-skill.md`
- fallback: `skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md`
- fallback_sha256: `82cb9f3448e0053a770e37953dfe6d9761234613f62b7cc417bf0c57ce606745`
- original_lines: 942
- original_bytes: 80310
- semantic_inventory_sha256: `b57e3cf5ba759e6d3d61ab1286b149f348fee727a0d7e484c2f5c1d397866bc1`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 规范各 SKILL 的内容边界与维护规则。ae-sdd-skill 退守"流程编排"（流程怎么走、节点间如何流转），各子 SKILL 负责"环节内具体规则"（每一步具体怎么做、出错怎么处理）。当用户新增/修改任何 AE 相关 SKILL 时，先查阅本 SKILL 确认内容应放在哪个文件，避免在错误位置撰写或重复堆积。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L3:407 适用范围; keyword_hits: 40 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L3:14 auto-engineering-skill = 流程编排（不退守就会腐化）; L2:255 内容回写到正确位置的 5 步流程; L3:343 步骤 4.1：PRD 级状态机同步清单扩展（🆕 v3.3.0）; +1 more; keyword_hits: 161 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:14 auto-engineering-skill = 流程编排（不退守就会腐化）; L3:529 禁止; L3:549 📍 权威源（机器可读，Agent 必须从这里消费）; +3 more; keyword_hits: 279 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:646 检查器（`tools/lib/update_graph.py` + AA 注入）; L4:702 步骤 2 + 3 + 4：跑 `ae-sdd iteration-check`（🆕 v3.5.4 接管步骤 2/3/4 机器粗筛）; keyword_hits: 247 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:343 步骤 4.1：PRD 级状态机同步清单扩展（🆕 v3.3.0）; keyword_hits: 90 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L3:367 步骤 4.5：写入 CHANGELOG（🆕 2026-06-10 强制）; L3:724 输出物：《设计-实现一致性迭代检查报告》; L3:735 🔴 阻断级（文档撒谎：声明存在但实际无）; keyword_hits: 150 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 120; refs: *-skill.md; *.md; ../../assets/{projectKey}/*.assets.md; +117 more; headings: L3:195 实例化 4 层架构速查（与 SKILL.md §6 互补）; keyword_hits: 559 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:12 核心设计哲学; L2:73 项目结构与设计说明（🆕 v3.2.4 — 维护者的项目地图）; L3:195 实例化 4 层架构速查（与 SKILL.md §6 互补）; +5 more; keyword_hits: 299 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L2:73 项目结构与设计说明（🆕 v3.2.4 — 维护者的项目地图）; L3:367 步骤 4.5：写入 CHANGELOG（🆕 2026-06-10 强制）; L3:461 同步脚本说明（🆕 v3.0 三脚本分工）; +2 more; keyword_hits: 96 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Auto Engineering Update — SKILL 边界维护规范 |
| 2 | 12 | 核心设计哲学 |
| 3 | 14 | auto-engineering-skill = 流程编排（不退守就会腐化） |
| 2 | 31 | 🔴 极简描述原则（🆕 2026-07-01 — 瘦身是持续动作，不是一次性重构） |
| 3 | 53 | 各子 SKILL = 环节内具体规则 |
| 2 | 73 | 项目结构与设计说明（🆕 v3.2.4 — 维护者的项目地图） |
| 3 | 79 | 6 大子系统总览 |
| 3 | 92 | 子系统协同关系图 |
| 3 | 136 | 各子系统维护边界判定（扩展原判定表） |
| 3 | 151 | 维护者 SOP（按子系统） |
| 3 | 195 | 实例化 4 层架构速查（与 SKILL.md §6 互补） |
| 2 | 210 | SKILL 边界判定表（新增/修改内容时使用） |
| 2 | 255 | 内容回写到正确位置的 5 步流程 |
| 3 | 261 | 步骤 1：识别内容类型 |
| 3 | 284 | 步骤 2：定位目标 SKILL |
| 3 | 312 | 步骤 3：执行回写 |
| 3 | 322 | 步骤 4：更新交叉引用 |
| 3 | 343 | 步骤 4.1：PRD 级状态机同步清单扩展（🆕 v3.3.0） |
| 3 | 367 | 步骤 4.5：写入 CHANGELOG（🆕 2026-06-10 强制） |
| 3 | 389 | 步骤 5：验证无重复 |
| 1 | 394 | 在 AE-skill 中 grep "已下沉到" — 应能列出所有外链指针 |
| 1 | 397 | 在子 SKILL 中 grep 关键章节标题 — 应能在目标位置找到 |
| 2 | 403 | 母版修改后的同步规则（强制） |
| 3 | 407 | 适用范围 |
| 3 | 418 | 默认规则（🆕 v3.0 双目录分层） |
| 3 | 436 | 修改后动作（🆕 v3.0 工作流） |
| 3 | 461 | 同步脚本说明（🆕 v3.0 三脚本分工） |
| 3 | 469 | 🆕 v3.4.0 自动分发闭环（post-commit hook） |
| 3 | 529 | 禁止 |
| 2 | 543 | 更新依赖图谱（🆕 v3.2 — 改了 A 要同步 BCDEFG，杜绝漏更新） |
| 3 | 549 | 📍 权威源（机器可读，Agent 必须从这里消费） |
| 3 | 574 | 机器同步锚点（UC-14 自动读取） |
| 3 | 581 | 🤖 Agent 程序化消费协议（强制 — Agent 改完文件后必做） |
| 1 | 593 | qr.affected_items → 连带项清单（path/action/auto_checkable） |
| 1 | 594 | qr.checks_to_run → 该跑的 UC-XX 检查 ID |
| 1 | 603 | 或只跑第 1 步返回的 checks_to_run |
| 3 | 611 | 人读视图（仅供参考，非权威） |
| 3 | 629 | 图谱使用 SOP（Agent 流程） |
| 3 | 646 | 检查器（`tools/lib/update_graph.py` + AA 注入） |
| 3 | 661 | 图谱维护规则 |
| 2 | 671 | 设计-实现一致性迭代检查（🆕 v3.5.3 — 每月/重大变更后跑，补 UC 自动检查的盲区） |
| 3 | 675 | 为什么需要本节（UC 自动检查够不到的 4 类盲区） |
| 3 | 684 | 检查时机 |
| 3 | 690 | 检查 SOP（4 步，强制顺序） |
| 4 | 692 | 步骤 1：跑自动化基线（UC + health + gates） |
| 4 | 702 | 步骤 2 + 3 + 4：跑 `ae-sdd iteration-check`（🆕 v3.5.4 接管步骤 2/3/4 机器粗筛） |
| 3 | 724 | 输出物：《设计-实现一致性迭代检查报告》 |
| 1 | 727 | ae-sdd 设计-实现一致性迭代检查报告（{日期}） |
| 2 | 729 | 自动化基线 |
| 2 | 734 | 不一致清单（按严重度） |
| 3 | 735 | 🔴 阻断级（文档撒谎：声明存在但实际无） |
| 3 | 738 | 🟡 一般级（部分一致：实现存在但缩水） |
| 3 | 740 | ✅ 已诚实自认降级（不算撒谎） |
| 2 | 743 | 根因分析 |
| 2 | 746 | 修复建议（按优先级 P0/P1/P2） |
| 3 | 750 | 与 UC 自动检查的关系（不替代，是补充） |
| 3 | 762 | 门禁 |
| 2 | 770 | SKILL 健康度自检清单（每月或重大变更后跑一次） |
| 3 | 772 | AE-skill 健康度 |
| 3 | 784 | 子 SKILL 健康度 |
| 3 | 848 | 跨 SKILL 一致性 |
| 2 | 859 | 禁止的 6 种反模式 |
| 2 | 873 | 与其他 SKILL 的关系 |
| 2 | 882 | 本次重构摘要（2026-06-04） |
| 2 | 899 | 本次重构摘要（2026-06-10 任务规模分级） |
| 2 | 919 | 本次重构摘要（2026-06-10 SKILL 母版目录全面重组 + 3 项配套整改） |

## Inline References

| ref |
| --- |
| *-skill.md |
| *.md |
| ../../assets/{projectKey}/*.assets.md |
| ../../standards/constraints/*.md |
| ../../standards/project-assets/project-assets-schema.md |
| ../../standards/testing/be-testcase-strategy.md |
| ../../standards/thinking/be-coding-thinking-engine.md |
| ../../templates/coding/be-codereview-template.md |
| ../../templates/coding/be-coding-report-template.md |
| ../../templates/design/*.md |
| ../../templates/testcase/*.md |
| ../cross-cutting/ae-sdd-plugin-loader-skill.md |
| ../cross-cutting/agent-orchestration-skill.md |
| ../cross-cutting/document-storage-skill.md |
| ../cross-cutting/project-assets-update-skill.md |
| ../cross-cutting/proposal-skill.md |
| ../phase1-design/dr-update-skill.md |
| ../phase1-design/story-review-skill.md |
| ../phase1-design/story-update-skill.md |
| ../phase1-design/testcase-generate-skill.md |
| ../phase1-design/testcase-review-skill.md |
| ../phase2-coding/coding-skill.md |
| ../phase2-task/task-generate-skill.md |
| ../phase3-review/test-generate-skill.md |
| ../phase3-review/test-review-skill.md |
| ./xxx-skill.md |
| ./xxx-skill.md#锚 |
| .ae-plan/ |
| .ae-sdd/runtime-stats/ |
| .ae-task/ |
| .claude-plugin/marketplace.json |
| .githooks/post-commit |
| .harness/.adapter.lock |
| .harness/agent.md |
| .idea/ |
| 2026-06-10-AE-4类需求路由.md |
| 2026-06-27-v3.5.9-ra-derivation-depth-gate.md |
| 2026-06-30-v3.6.0-ra-implementation-view-gate.md |
| 2026-07-02-automation-switch.md |
| 2026-07-03-runtime-stats-p0.md |
| <project>/.ae-sdd/ |
| CHANGELOG/ |
| CHANGELOG/_template.md |
| HARNESS.md |
| N/A |
| Plan/ |
| README.md |
| README.md:5 |
| SKILL.md |
| SKILL.md §1.1~1.6 |
| SKILL.md §1.3 |
| SKILL.md §4 类需求智能路由 |
| SKILL.md §智能路由表 + §路由决策算法 2.2 |
| SKILL.md §角色库 |
| SKILL.md §路由决策算法 2.2 套模板判定步骤 |
| Task/ |
| YYYY-MM-DD-{主题}.md |
| _G\d+_CLEAR_RE |
| ae-sdd --help \\| grep prd- |
| ae-sdd <cmd> |
| ae-sdd context-pressure [--story <ID>] |
| ae-sdd fork |
| ae-sdd health |
| ae-sdd health master-freshness |
| ae-sdd init |
| ae-sdd init <project-dir> <project-key> |
| ae-sdd iteration-check |
| ae-sdd state write --phase <next> |
| ae-sdd update-check |
| ae-sdd update-check --affected |
| ae-sdd update-check --affected <改动的文件> |
| ae-sdd update-check --only UC-14 |
| ae-sdd-conventions.md §2.3 |
| ae-sdd-design.md |
| ae-sdd-implementation-architecture.md |
| ae-sdd-monitor-design.md |
| ae-sdd-plugin-loader-skill.md |
| ae-sdd-skill.md |
| agent-orchestration-skill.md |
| apps/ae-sdd-monitor/ |
| apps/ae-sdd-monitor/README.md |
| apps/ae-sdd-monitor/src/workspace.js |
| apps/ae-sdd-monitor/test/workspace.test.js |
| assets check/generate/audit/update |
| assets/ |
| assets/{projectKey}/ |
| automation status/enable/disable |
| bash scripts/build-dist.sh |
| bash scripts/dev-sync.sh |
| bash scripts/install-hooks.sh |
| bash scripts/install.sh |
| code-review-skill.md |
| code-review-skill.md §第四步 bis |
| coding-report-skill.md |
| coding-skill.md |
| coding-skill.md §4.2 |
| coding-skill.md §4.2 / §6.0 |
| coding-skill.md §6.0 |
| coding-skill.md §CodingSkill 对外调用契约 |
| constraints/ |
| cp -r dist/ae-sdd/. ~/.claude/skills/ae-sdd/ |
| database-tool-skill.md |
| db profiles/query/explain/audit |
| db-connection-profile.schema.md |
| design/ |
| dist/ |
| dist/ae-sdd/ |
| dist/ae-sdd/.claude-plugin/plugin.json |
| dist/ae-sdd/SKILL.md |
| dist/ae-sdd/VERSION |
| docs/ |
| document-storage-skill.md |
| document-storage-skill.md §1.3 路径模板 |
| document-storage-skill.md §1.6 旧路径兼容层 |
| document-storage-skill.md 附录 A |
| dr-update-skill.md |
| durationMs/slowest |
| git config --global core.hooksPath /dev/null |
| git status/diff/log/blame/impact |
| git-insight-skill.md |
