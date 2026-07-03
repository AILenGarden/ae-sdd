---
name: project-assets-update
description: 项目资产更新 SKILL — 维护 {projectKey}.assets.md，生成 7 层索引（大纲/模块/字段/组件/API/反向/读取 API），支持按需加载和增量更新。当需要"查看/更新项目资产"或新工程初始化时触发。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/project-assets-update-skill.full.md
source_fallback_sha256: c080f795e036de22c41991e2428b40bd0fac5e9c086a520dd59ed5ec40e424fc
source_original_bytes: 78891
source_original_lines: 1691
source_semantic_inventory_sha256: 720b05b3b44fbc37771a508f2af731e002d20d21ace14ba6c26c5615551703bc
source_slimmer: slim_source_skills.py@2
---

# Project Assets Update — 项目资产目录生成/更新/审计 SKILL Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/project-assets-update-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/project-assets-update-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/project-assets-update-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/project-assets-update-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/project-assets-update-skill.full.md`
- fallback_sha256: `c080f795e036de22c41991e2428b40bd0fac5e9c086a520dd59ed5ec40e424fc`
- original_lines: 1691
- original_bytes: 78891
- semantic_inventory_sha256: `720b05b3b44fbc37771a508f2af731e002d20d21ace14ba6c26c5615551703bc`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 项目资产更新 SKILL — 维护 {projectKey}.assets.md，生成 7 层索引（大纲/模块/字段/组件/API/反向/读取 API），支持按需加载和增量更新。当需要"查看/更新项目资产"或新工程初始化时触发。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:29 📦 文档存放前置调用（🔴 横切依赖）; L2:62 1. 触发条件; L3:126 3.1 触发场景; +8 more; keyword_hits: 123 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:74 2. 整体流程; L3:690 6.2 读取流程（🆕 2026-06-24 脚本化：初始化检查 + 映射表 + BM25 查询）; L4:710 步骤 2：按阶段调用 `ae-sdd assets read`（核心入口）; +2 more; keyword_hits: 60 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L4:167 3.3.1 🆕 pending-questions.md 格式; L3:197 3.4 门禁; L4:490 步骤 6：🆕 消掉 pending-questions.md 中已解决的问题; +6 more; keyword_hits: 89 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:233 §3.5.2 抽取命令清单; L1:295 2. 硬编码 API Key / Secret（🔴 P0）; L4:432 步骤 2：跑对应探查命令; +3 more; keyword_hits: 183 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:270 §3.6.1 部署信息抽取（schema §1.X 10 个字段）; L3:354 §3.7.3 步骤 16.c — config/ 环境配置专题; L1:447 🆕 新增表/字段; +3 more; keyword_hits: 118 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:29 📦 文档存放前置调用（🔴 横切依赖）; L3:158 3.3 输出物; L4:598 步骤 6：输出审计报告; +3 more; keyword_hits: 65 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 97; refs: <projectKey>.assets.md; <version>X.X.X</version>; [已确认]/[据推断]/[待确认]; +94 more; headings: L3:132 3.2 17 步 SOP（详见 `project-assets-schema.md §9 + §12-15`，🆕 2026-06-26 增步骤 10-17）; L4:167 3.3.1 🆕 pending-questions.md 格式; L4:490 步骤 6：🆕 消掉 pending-questions.md 中已解决的问题; +4 more; keyword_hits: 112 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:132 3.2 17 步 SOP（详见 `project-assets-schema.md §9 + §12-15`，🆕 2026-06-26 增步骤 10-17）; L2:217 §3.5 步骤 14 SOP：抽完整技术栈版本号表（🆕 2026-06-26）; L3:221 §3.5.1 七张表必填; +24 more; keyword_hits: 688 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L4:418 步骤 1：识别变更类型; keyword_hits: 38 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Project Assets Update — 项目资产目录生成/更新/审计 SKILL |
| 2 | 29 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 52 | 0. 目标 |
| 2 | 62 | 1. 触发条件 |
| 2 | 74 | 2. 整体流程 |
| 2 | 124 | 3. 动作 1：生成（首次或新项目） |
| 3 | 126 | 3.1 触发场景 |
| 3 | 132 | 3.2 17 步 SOP（详见 `project-assets-schema.md §9 + §12-15`，🆕 2026-06-26 增步骤 10-17） |
| 3 | 158 | 3.3 输出物 |
| 4 | 167 | 3.3.1 🆕 pending-questions.md 格式 |
| 1 | 170 | {projectKey} 项目资产 — 待确认问题清单 |
| 2 | 179 | 待确认问题 |
| 3 | 197 | 3.4 门禁 |
| 2 | 217 | §3.5 步骤 14 SOP：抽完整技术栈版本号表（🆕 2026-06-26） |
| 3 | 221 | §3.5.1 七张表必填 |
| 3 | 233 | §3.5.2 抽取命令清单 |
| 1 | 236 | 1. 主框架版本（来自 <parent>） |
| 1 | 240 | 2. 所有显式声明的依赖版本 |
| 1 | 243 | 3. 内部 starter（公司 starter 必有 cass/panda/courier/job 等关键字） |
| 1 | 247 | 4. 测试框架 |
| 1 | 250 | 5. 构建与镜像 |
| 1 | 254 | 6. 静态分析 |
| 3 | 258 | §3.5.3 准入规则 |
| 2 | 266 | §3.6 步骤 15 SOP：抽部署信息 + 跑安全隐患扫描（🆕 2026-06-26） |
| 3 | 270 | §3.6.1 部署信息抽取（schema §1.X 10 个字段） |
| 3 | 285 | §3.6.2 安全隐患强制扫描（schema §14.1） |
| 1 | 290 | 1. 配置明文密码（🔴 P0） |
| 1 | 295 | 2. 硬编码 API Key / Secret（🔴 P0） |
| 1 | 299 | 3. Actuator 端点外露（🔴 P0） |
| 1 | 303 | 4. 数据库连接串明文账号（🔴 P0） |
| 3 | 313 | §3.6.3 与工程级子文件的衔接 |
| 2 | 319 | §3.7 步骤 16-17 SOP：工程级拆文件 + 横切专题 + 可信度三态（🆕 2026-06-26） |
| 3 | 323 | §3.7.1 步骤 16.a — 工程级子文件生成 |
| 3 | 339 | §3.7.2 步骤 16.b — function/ 业务场景专题 |
| 3 | 354 | §3.7.3 步骤 16.c — config/ 环境配置专题 |
| 3 | 370 | §3.7.4 步骤 16.d — domain/ 业务域概览专题 |
| 3 | 384 | §3.7.5 步骤 17 — 可信度三态标注 |
| 2 | 403 | 4. 动作 2：更新（增量修改） |
| 3 | 405 | 4.1 触发场景 |
| 3 | 416 | 4.2 5 步 SOP |
| 4 | 418 | 步骤 1：识别变更类型 |
| 4 | 432 | 步骤 2：跑对应探查命令 |
| 1 | 437 | 新增微服务 |
| 1 | 441 | 修改分层 |
| 1 | 444 | 补缺口（端口/服务名） |
| 1 | 447 | 🆕 新增表/字段 |
| 1 | 449 | 或 SHOW CREATE TABLE {table_name} |
| 1 | 451 | 🆕 新增公共组件 |
| 1 | 455 | 🆕 新增跨服务 API |
| 4 | 459 | 步骤 3：🔴 触发 Proposal（不直接改项目资产） |
| 4 | 472 | 步骤 4：跑双源一致性检查（如果涉及 §6 工程约束） |
| 1 | 475 | 自动化脚本建议（每月审计时跑） |
| 4 | 483 | 步骤 5：写更新日志条目 |
| 4 | 490 | 步骤 6：🆕 消掉 pending-questions.md 中已解决的问题 |
| 3 | 503 | 4.3 门禁 |
| 3 | 512 | 4.4 🆕 工程级子文件更新 SOP（2026-06-26） |
| 2 | 543 | 5. 动作 3：审计（每月例行） |
| 3 | 545 | 5.1 触发场景 |
| 3 | 551 | 5.2 7 步 SOP（新增第 4 步索引有效性） |
| 4 | 553 | 步骤 1：读更新日志 |
| 4 | 557 | 步骤 2：跑双源一致性脚本 |
| 1 | 560 | 1) §6 引用完整性 |
| 1 | 562 | 对比 constraints/ 目录文件名 |
| 1 | 565 | 2) §6 描述与 constraints/ 实际内容一致性 |
| 1 | 566 | 人工 spot check 3-5 条 |
| 4 | 569 | 步骤 3：跑缺口进度 |
| 4 | 578 | 步骤 4：🆕 跑索引有效性检查 |
| 4 | 589 | 步骤 5：跑"知识衰减"检查 |
| 4 | 598 | 步骤 6：输出审计报告 |
| 2 | 603 | 审计报告 - {YYYY-MM-DD} |
| 4 | 621 | 步骤 7：更新 §1 lastAuditedAt |
| 3 | 626 | 5.3 门禁 |
| 3 | 634 | 5.4 🆕 横切专题审计 SOP（2026-06-26） |
| 2 | 662 | 横切专题审计（YYYY-MM-DD） |
| 2 | 678 | 6. 动作 4：读取（被其他 SKILL 调用）— 原 ④bis SOP 步骤 1 |
| 3 | 682 | 6.1 触发场景 |
| 3 | 690 | 6.2 读取流程（🆕 2026-06-24 脚本化：初始化检查 + 映射表 + BM25 查询） |
| 4 | 697 | 步骤 1：检查项目资产 + 索引是否就绪 |
| 1 | 700 | 检查资产文件存在 |
| 1 | 702 | → 返回索引统计 + cache_status（hit=复用缓存 / miss=本次新建） |
| 1 | 703 | → 资产不存在 → 禁止继续，先执行本 SKILL §3 生成动作 |
| 4 | 710 | 步骤 2：按阶段调用 `ae-sdd assets read`（核心入口） |
| 1 | 713 | LLM 只说阶段 + 可选追加精准 KEY，脚本返回简约定位 v |
| 4 | 735 | 步骤 2bis：精准查（LLM 追加自定义 KEY） |
| 1 | 738 | 例：coding 阶段，额外精准查某个具体类名/字段 |
| 1 | 740 | → extra_hits 字段返回这两个 KEY 的 BM25 命中 |
| 4 | 743 | 步骤 3：确认 §1 lastAuditedAt 在合理范围 |
| 1 | 747 | → stats 含 lastAuditedAt；> 90 天 → 禁止使用，先跑审计（§5） |
| 4 | 750 | 步骤 4：在 CodePlan 头部写"项目资产已就绪"声明 |
| 3 | 761 | 6.3 门禁 |
| 3 | 768 | 6.4 🆕 按功能专题加载 SOP（2026-06-26） |
| 1 | 784 | 按 Story 编号加载（最常用） |
| 1 | 786 | 返回：function/STORY-003-BE-登录鉴权.md 全文 |
| 1 | 788 | 按场景关键词加载（不记得 Story 编号时） |
| 1 | 790 | 返回：所有含"loginScene=cs"的 function/ 文档 |
| 1 | 792 | 按工程加载（"我要看 boss-user 工程所有上下游契约"） |
| 1 | 794 | 返回：主体 §B + 工程级子文件 icec-cloud-boss-user.assets.md 全文 |
| 1 | 796 | 按环境配置加载（"本地起 boss-user 前要做什么"） |
| 1 | 798 | 返回：config/test/api-test-env.md 中 boss-user 章节 |
| 1 | 800 | 按业务域加载（"了解 IM 域全景"） |
| 1 | 802 | 返回：domain/im.md 全文 + 主体 §2 IM 相关行 |
| 2 | 819 | 7. 异常路径 |
| 3 | 821 | A1：项目资产生成中途发现项目结构本身有缺陷 |
| 3 | 827 | A2：双源一致性审计发现严重漂移 |
| 3 | 833 | A3：缺口长期未补（连续 3 个月） |
| 3 | 839 | A4：用户手动撤销项目资产 |
| 3 | 845 | A5：🆕 索引项与实际代码严重不一致 |
| 2 | 853 | 8. 与其他 SKILL 的衔接 |
| 2 | 867 | §A 资产大纲生成 SOP（🆕 索引层 1/7） |
| 3 | 869 | A.1 目标 |
| 3 | 873 | A.2 触发场景 |
| 3 | 878 | A.3 输出模板 |
| 2 | 881 | §A 资产大纲（Outline） |
| 3 | 886 | A.1 项目速览 |
| 3 | 898 | A.2 一级目录速查 |
| 3 | 923 | A.4 门禁 |
| 2 | 930 | §B 模块索引 SOP（🆕 索引层 2/7） |
| 3 | 932 | B.1 目标 |
| 3 | 936 | B.2 触发场景 |
| 3 | 942 | B.3 输出模板 |

## Inline References

| ref |
| --- |
| <projectKey>.assets.md |
| <version>X.X.X</version> |
| [已确认]/[据推断]/[待确认] |
| \\| |
| \\| \ |
| ae-sdd assets |
| ae-sdd assets outline --project <key> |
| ae-sdd assets query "<keyword>" --project <key> |
| ae-sdd assets query "<name>" --project <key> |
| ae-sdd assets query "<table>" --project <key> |
| ae-sdd assets read |
| ae-sdd assets read <stage> |
| ae-sdd assets read <stage> [--keys "..."] --project <key> |
| ae-sdd assets section 4 |
| ae-sdd assets section <name> --project <key> |
| ae-sdd assets stats --project <key> |
| ae-sdd doc save --intent PROPOSAL --story-id {projectKey} --content-file 草稿.md |
| ae-sdd-skill.md |
| api-test-env.md |
| assets/{projectKey}/.archive/{date}/ |
| boss-common/.../context/AccessUserInfoContext.java |
| boss-common/.../request/PageRequest.java |
| boss-common/.../result/ApiResult.java |
| boss-common/.../result/PagedModels.java |
| boss_user / boss_role / boss_menu |
| code-review-skill.md |
| coding-skill.md |
| config-topic-template.md |
| config/ |
| config/test/ |
| config/test/api-test-env.md |
| config/{env}/部署清单.md |
| constraints/ |
| constraints/project-structure.md |
| cp {schema 附录 B starter} → {projectKey}.{module-name}.assets.md |
| cs_ticket / cs_session |
| document-storage-skill.md |
| document-storage-skill.md §1.3 路径模板 |
| document-storage-skill.md §1.4 资产类路径模板 |
| domain-topic-template.md |
| domain/ |
| domain/{域}.md |
| dr-update-skill.md |
| find . -name "pom.xml" -path "*/icec-cloud-*" |
| function-topic-template.md |
| function/ |
| function/STORY-003-BE-登录鉴权.md |
| function/{Story}.md |
| function/{Story编号}.md |
| grep "cass-public" {gitPath}/pom.xml |
| grep "jacoco" {gitPath}/*/pom.xml |
| grep "工程信息" {资产根}/config/test/*.md |
| grep -A 5 "hikari" {gitPath}/*/src/main/resources/ |
| grep -B 1 -A 3 "docker.image" {gitPath}/*/pom.xml |
| grep -E "redis.*password.*:" {gitPath}/*/src/main/resources/ |
| grep -r "icec.api" {gitPath}/*/src/main/resources/ |
| grep -r "jdbc:mysql" {gitPath}/*/src/main/resources/ |
| grep -r "management.port" {gitPath}/*/src/main/resources/ |
| grep -r "spring.profiles.active" {gitPath}/*/src/main/resources/ |
| grep -r "spring.redis.host\\|redis.*dcs" {gitPath}/*/src/main/resources/ |
| icec-cloud-boss-security/.../annotation/RequiresPermissions.java |
| icec-cloud-boss-security/.../annotation/SkipAuth.java |
| icec-cloud-boss-security/.../service/TokenService.java |
| icec-cloud-boss-user-bff/.../operationlog/capability/RoleOperationLoggable.java |
| icec-cloud-boss-user-infrastructure/.../config/MybatisPlusConfig.java |
| icec-cloud-boss-user-infrastructure/.../messaing/publisher/KafkaDomainEventPublisher.java |
| im_message / im_session |
| ls {资产根}/*/{工程}.assets.md |
| ls {资产根}/domain/ |
| ls {资产根}/function/ \\| wc -l |
| ls {资产根}/{line}/*/{工程}.assets.md |
| project-assets-schema.md |
| project-assets-schema.md §7.2 |
| project-assets-schema.md §9 + §12-15 |
| project-assets-template.md |
| proposal-skill.md |
| skills/ae-sdd/assets/ |
| skills/ae-sdd/assets/{projectKey}/ |
| src/test |
| story-review-skill.md |
| story-update-skill.md |
| task-generate-skill.md |
| templates/project-assets/project-assets-update-log-template.md |
| testcase-generate-skill.md |
| tools/lib/assets_index.py |
| {Story编号}.md |
| {docWorkspacePath}/.ae-sdd/assets/{workspaceKey}/ |
| {line}/{工程名}/ |
| {module}.assets.md |
| {projectKey}.pending-questions.md |
| {projectKey}.update-log.md |
| {projectKey}.{module}.assets.md |
| {域Key}.md |
| {资产根}/[{line}/]{工程名}/{工程名}.assets.md |
| {资产根}/{workspaceKey}.assets.md |
| {资产根}/{workspaceKey}.pending-questions.md |
| {资产根}/{workspaceKey}.update-log.md |
