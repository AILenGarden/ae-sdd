---
name: ae-sdd
version: 3.10.3
description: |
  端到端自动化工程主入口（v3.10.2）。从 DR 出发，经 Story->TestCase->CodingPlan->Coding->Test->Review，直到全部通过。
  支持大/中/小/微四条子链（按已有产物就近入链）、流程状态跟踪、中断恢复、主流程监管器（产物核查+偏移检测+暂离回归协议）。
  🆕 v3.10.2：micro 意图分流——`/ae-sdd 优化这部分实现` / `/ae-sdd CodeReview 这段` 不再误进自更新、也不走完整 Coding 全链。classify 新增 entryNode=OPTIMIZE/CODE_REVIEW + 代码上下文消歧（self-update 上下文优先）；gate 跨步跳跃对微链意图 entry_node 放行（复用 BUG 豁免范式）；code-review 新增无文档轻量准入分支；coding-process §A1.4 加意图分流前置门。详见 CHANGELOG/2026-07-11-v3.10.2-micro-intent-routing.md。
  🆕 v3.10.0：砍 Task phase + Route 下移重分级--Task 骨架分解合并进 CodingProcess §A1.5；大=DR、中=Story、小=CodingPlan、微=无文档。精简流程为 Story->TestCase->CodingPlan->Coding->Test->Review（含实现报告）。
  🆕 v3.10.1：state 创建时带随机 UUID 前缀保证目录名/stateMachineId 全局唯一--目录名从 `PRD-IM-CS` 变为 `{uuid}-PRD-IM-CS`，新增 `stateMachineName`（纯业务名）+ `stateUuid` 字段；`find_work_item_state_path` 增后缀匹配（按业务名可命中 UUID 前缀目录）；防同业务名撞目录互相覆盖。向后兼容旧 state。
  🆕 v3.9.22：测试 fixture 全量迁移到 task-scoped work-item state（跟随 v3.9.13 架构决策）+ 修复 6 处确定性 bug（入口脚本 py -3 引号 / assets_index 多文件 stats 崩溃 / gates.py 三元运算符丢行号 / update_graph kind 误标 / post-commit 无 pipefail 掩盖分发失败 / 版本号三处对齐）。
  🆕 v3.9.21：门禁按会话 engage 按需启用——修复"没调 /ae-sdd 的会话/子 Agent 也被全局 hook 锁死"。gate-intercept 增加 engage 短路：未 engage 直接放行；prompt-inject 检测 /ae-sdd 触发词写会话级 engage 标记（.ae-sdd/.session-engaged/），说"退出 ae-sdd"清除。语义从"有 .ae-sdd/ 就锁"改为"调了 ae-sdd 才锁"。
  🆕 v3.9.20：三症同治——(1) manifest 拆双文件（manifest-index.json LLM 用，省 75% tokens）；(2) G-STORY-CTX 升级真"已引用"门禁（查 Story 正文引用约束条目 + 取消小/微豁免）；(3) 新增 G-REVIEW-DEPTH（禁裸✅ + 零发现举证）。统一哲学：查产物证据不查行为。
  🆕 v3.9.19：顶层结构整理——清 scratch + README 仓库结构树补齐 + RELEASING.md 发版包指南 + UC-17 仓库顶层结构契约守门。
  🆕 v3.9.12：Story 模板新增「## 人工任务」章节——修复"人工任务"语义分裂（声明源在 StoryGeneratePlan §1.6 临时计划产物里、登记处在 Story 验收记录尾巴、Story 正文无声明源）的设计断裂。新增 `## 人工任务 \`选填\`` 章节（位于实现任务映射之后、偏离声明之前）作为非编码人工处理项的长期声明源（含类型枚举 8 类）；StoryGeneratePlan §1.6 加落位指引；story-template 验收记录下「人工任务完成」改为引用本章节（DRY）；story-generation-standard §2.5 F 阶段映射新增「§人工任务」。
  🆕 v3.9.11：镜像反模式根除 + 5 层防复发护城河——life 项目 STORY-003 卡死事故复盘，5 个独立缺口叠加（镜像冻结/phase 缺失/G-00 未同步/cmd_state_write 无冻结检测/缺维护脚本）。5 层防御：G-00 二段校验（镜像可缺 + 镜像-源一致性）+ 5 单测 + cmd_state_write 镜像冻结自动恢复 + prompt-inject step-X- 反模式检测 + check_mirror_health.py 维护脚本。
  🆕 v3.9.10：门禁路径 bug 修复--`paths.find_doc` / `paths.list_docs` 原只搜 `design/` + 项目根（deprecated 旧路径），未覆盖 document-storage 新布局 `ae-sdd-doc/{Category}/`；G-02/G-04/G-05/G-07 + 上下文准入门禁（G-STORY-CTX 等）在项目用新布局存文档时误判 block 失败。新增 `paths.doc_search_roots`（多根：项目根 + docWorkspace），find_doc/list_docs 内部同时搜旧路径 + `ae-sdd-doc/`（rglob 兜底），签名向后兼容；`gates._doc_search_roots` 委托 paths 统一入口（DRY）。
  🆕 v3.9.9：harness 回滚补全 README.md + identity sanity check 单测覆盖——mount 失败回滚三件套（agent.md/README.md/.adapter.lock）；`_IDENTITY_ATTRIBUTION_PATTERNS` Pattern 1 正则收窄（加归属动词限定，消除合法提及误报）；新增 `TestIdentitySanityCheck` 14 用例（11 命中 + 3 误报防护）。
  🆕 v3.9.8：mirror-fallback trap fix——`_active_state_from_mirror` + `_main_state_path_for_args` 第 213-235 行在 `.ae-sdd/state.json` 镜像缺失时主动扫描 `.auto-engineering/*/state.json` 按 mtime 选最近活跃为 source；`health` 检查项 `state.json 可读` → `state.json 可定位`（镜像 + 源任一可定位即 pass）。允许 life 等项目把镜像当反模式删除，仅留 work-item 源为唯一真值。
  支持大/中/小/微四条子链（按已有产物就近入链）、流程状态跟踪、中断恢复、主流程监管器（产物核查+偏移检测+暂离回归协议）。
  🆕 v3.9.7：gate_intercept `_check_memory_entered` 入口惰性创建 `.ae-sdd/memory/` 根目录（best-effort），修复"全新项目从未跑 memory enter 时，目录缺失 = stage 永假"导致的设计阶段死环（life 项目实测触发）；不改变活跃态判定语义。
  🆕 v3.9.6：模板排版规范化——22 个模板统一 10 类排版规范（必填/选填标记、表格分隔符、章节编号、示例引导、强制规则锚点、emoji 语义、文档头部声明、末尾收尾）；新建 `template-layout-standard.md` SSOT。
  🆕 v3.9.5：Story 模板接口契约章节合并——原「接口契约-SPI/API」+「🔴 前端接口契约」两段合并为单一 `## 接口契约` 章节；每个接口用 `### 接口 N：{签名}（REST|SPI）` 统一编号锚点 + `---` 强制分隔，解决多接口渲染黏连；接口块内融合后端契约（Request/VO 四维）与前端视角（JSON 示例/调用流程/状态展示/边界处理）；6 个引用文件同步锚点名；`gates.py:_check_source_trace` 兼容性验证通过。
  🆕 v3.9.4：Story 流程根治——新增 `story-input-checklist.md` SSOT 输入清单（13 项 4 类）；`G-STORY-CTX` 扩展为 6 类（新增 dependsStory + sourceTrace）；`story-generation-standard.md` §2.5 新增 7 阶段→模板章节映射表，§4 自检闸门 8→10（新增来源追溯闸 + 章节映射闸）；Story generate/review/update 三件套 SSOT 化 + 来源追溯步骤。
  🆕 v3.9.3：新增「输出核心原则」第 4 条——禁止文档承载 changelog（设计/架构/模板/标准类文档只写当前生效内容，历史变更走 `source/CHANGELOG/{YYYY-MM-DD}-{主题}.md`）。
  🆕 v3.9.1：修复 gate_intercept 对嵌套 state 不感知——4 处顶层 phase/currentStory 读取改用 get_active_phase/get_active_story 统一接口，消除嵌套 state 项目 src/ 写入被误拦为"设计阶段禁止写入源码目录"的回归。
  🆕 v3.9.0：嵌套状态模型——单文件嵌套 state（prdState/drState/storyStates{N}），任意节点出发+向上归入，/ae-sdd 路由自动匹配/新建 state，改已管理 Story 自动重定位+重置子状态；命名只以顶层主体特征命名。
  🆕 v3.8.2：修复五层记忆存取断裂；强化独立需求状态机入口，`state new --id --name` 创建 `{ID}--{name}` 状态机目录。
  🆕 v3.8.0：自动化开关配置（`.ae-sdd/config.yaml` 的 `automation` 段，默认关闭）。开启后 6 个人工审核点改走 Tier 3 多 reviewer 联审共识，实现输入→结果全自动化；开工前预收集所有必需信息。
  历史变更见 source/CHANGELOG/。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/SKILL.full.md
source_fallback_sha256: df06c48347157939c7cc2d3baf9064e77a60f242c538d1da20cf777334ee709b
source_original_bytes: 45937
source_original_lines: 749
source_semantic_inventory_sha256: 32041130d5eddc003bab8d0bec043b720c216394fa1e2f58dd539c70f0197b5b
source_slimmer: slim_source_skills.py@2
---

# ae-sdd Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/SKILL.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/SKILL.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/SKILL.full.md`, not this slim entry.

## Summary

- source: `SKILL.md`
- fallback: `skill-fallbacks/SKILL.full.md`
- fallback_sha256: `df06c48347157939c7cc2d3baf9064e77a60f242c538d1da20cf777334ee709b`
- original_lines: 749
- original_bytes: 45937
- semantic_inventory_sha256: `32041130d5eddc003bab8d0bec043b720c216394fa1e2f58dd539c70f0197b5b`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 端到端自动化工程主入口（v3.10.2）。从 DR 出发，经 Story->TestCase->CodingPlan->Coding->Test->Review，直到全部通过。
支持大/中/小/微四条子链（按已有产物就近入链）、流程状态跟踪、中断恢复、主流程监管器（产物核查+偏移检测+暂离回归协议）。
🆕 v3.10.2：micro 意图分流——`/ae-sdd 优化这部分实现` / `/ae-sdd CodeReview 这段` 不再误进自更新、也不走完整 Coding 全链。classify 新增 entryNode=OPTIMIZE/CODE_REVIEW + 代码上下文消歧（self-update 上下文优先）；gate 跨步跳跃对微链意图 entry_node 放行（复用 BUG 豁免范式）；code-review 新增无文档轻量准入分支；coding-process §A1.4 加意图分流前置门。详见 CHANGELOG/2026-07-11-v3.10.2-micro-intent-routing.md。
🆕 v3.10.0：砍 Task phase + Route 下移重分级--Task 骨架分解合并进 CodingProcess §A1.5；大=DR、中=Story、小=CodingPlan、微=无文档。精简流程为 Story->TestCase->CodingPlan->Coding->Test->Review（含实现报告）。
🆕 v3.10.1：state 创建时带随机 UUID 前缀保证目录名/stateMachineId 全局唯一--目录名从 `PRD-IM-CS` 变为 `{uuid}-PRD-IM-CS`，新增 `stateMachineName`（纯业务名）+ `stateUuid` 字段；`find_work_item_state_path` 增后缀匹配（按业务名可命中 UUID 前缀目录）；防同业务名撞目录互相覆盖。向后兼容旧 state。
🆕 v3.9.22：测试 fixture 全量迁移到 task-scoped work-item state（跟随 v3.9.13 架构决策）+ 修复 6 处确定性 bug（入口脚本 py -3 引号 / assets_index 多文件 stats 崩溃 / gates.py 三元运算符丢行号 / update_graph kind 误标 / post-commit 无 pipefail 掩盖分发失败 / 版本号三处对齐）。
🆕 v3.9.21：门禁按会话 engage 按需启用——修复"没调 /ae-sdd 的会话/子 Agent 也被全局 hook 锁死"。gate-intercept 增加 engage 短路：未 engage 直接放行；prompt-inject 检测 /ae-sdd 触发词写会话级 engage 标记（.ae-sdd/.session-engaged/），说"退出 ae-sdd"清除。语义从"有 .ae-sdd/ 就锁"改为"调了 ae-sdd 才锁"。
🆕 v3.9.20：三症同治——(1) manifest 拆双文件（manifest-index.json LLM 用，省 75% tokens）；(2) G-STORY-CTX 升级真"已引用"门禁（查 Story 正文引用约束条目 + 取消小/微豁免）；(3) 新增 G-REVIEW-DEPTH（禁裸✅ + 零发现举证）。统一哲学：查产物证据不查行为。
🆕 v3.9.19：顶层结构整理——清 scratch + README 仓库结构树补齐 + RELEASING.md 发版包指南 + UC-17 仓库顶层结构契约守门。
🆕 v3.9.12：Story 模板新增「## 人工任务」章节——修复"人工任务"语义分裂（声明源在 StoryGeneratePlan §1.6 临时计划产物里、登记处在 Story 验收记录尾巴、Story 正文无声明源）的设计断裂。新增 `## 人工任务 \`选填\`` 章节（位于实现任务映射之后、偏离声明之前）作为非编码人工处理项的长期声明源（含类型枚举 8 类）；StoryGeneratePlan §1.6 加落位指引；story-template 验收记录下「人工任务完成」改为引用本章节（DRY）；story-generation-standard §2.5 F 阶段映射新增「§人工任务」。
🆕 v3.9.11：镜像反模式根除 + 5 层防复发护城河——life 项目 STORY-003 卡死事故复盘，5 个独立缺口叠加（镜像冻结/phase 缺失/G-00 未同步/cmd_state_write 无冻结检测/缺维护脚本）。5 层防御：G-00 二段校验（镜像可缺 + 镜像-源一致性）+ 5 单测 + cmd_state_write 镜像冻结自动恢复 + prompt-inject step-X- 反模式检测 + check_mirror_health.py 维护脚本。
🆕 v3.9.10：门禁路径 bug 修复--`paths.find_doc` / `paths.list_docs` 原只搜 `design/` + 项目根（deprecated 旧路径），未覆盖 document-storage 新布局 `ae-sdd-doc/{Category}/`；G-02/G-04/G-05/G-07 + 上下文准入门禁（G-STORY-CTX 等）在项目用新布局存文档时误判 block 失败。新增 `paths.doc_search_roots`（多根：项目根 + docWorkspace），find_doc/list_docs 内部同时搜旧路径 + `ae-sdd-doc/`（rglob 兜底），签名向后兼容；`gates._doc_search_roots` 委托 paths 统一入口（DRY）。
🆕 v3.9.9：harness 回滚补全 README.md + identity sanity check 单测覆盖——mount 失败回滚三件套（agent.md/README.md/.adapter.lock）；`_IDENTITY_ATTRIBUTION_PATTERNS` Pattern 1 正则收窄（加归属动词限定，消除合法提及误报）；新增 `TestIdentitySanityCheck` 14 用例（11 命中 + 3 误报防护）。
🆕 v3.9.8：mirror-fallback trap fix——`_active_state_from_mirror` + `_main_state_path_for_args` 第 213-235 行在 `.ae-sdd/state.json` 镜像缺失时主动扫描 `.auto-engineering/*/state.json` 按 mtime 选最近活跃为 source；`health` 检查项 `state.json 可读` → `state.json 可定位`（镜像 + 源任一可定位即 pass）。允许 life 等项目把镜像当反模式删除，仅留 work-item 源为唯一真值。
支持大/中/小/微四条子链（按已有产物就近入链）、流程状态跟踪、中断恢复、主流程监管器（产物核查+偏移检测+暂离回归协议）。
🆕 v3.9.7：gate_intercept `_check_memory_entered` 入口惰性创建 `.ae-sdd/memory/` 根目录（best-effort），修复"全新项目从未跑 memory enter 时，目录缺失 = stage 永假"导致的设计阶段死环（life 项目实测触发）；不改变活跃态判定语义。
🆕 v3.9.6：模板排版规范化——22 个模板统一 10 类排版规范（必填/选填标记、表格分隔符、章节编号、示例引导、强制规则锚点、emoji 语义、文档头部声明、末尾收尾）；新建 `template-layout-standard.md` SSOT。
🆕 v3.9.5：Story 模板接口契约章节合并——原「接口契约-SPI/API」+「🔴 前端接口契约」两段合并为单一 `## 接口契约` 章节；每个接口用 `### 接口 N：{签名}（REST|SPI）` 统一编号锚点 + `---` 强制分隔，解决多接口渲染黏连；接口块内融合后端契约（Request/VO 四维）与前端视角（JSON 示例/调用流程/状态展示/边界处理）；6 个引用文件同步锚点名；`gates.py:_check_source_trace` 兼容性验证通过。
🆕 v3.9.4：Story 流程根治——新增 `story-input-checklist.md` SSOT 输入清单（13 项 4 类）；`G-STORY-CTX` 扩展为 6 类（新增 dependsStory + sourceTrace）；`story-generation-standard.md` §2.5 新增 7 阶段→模板章节映射表，§4 自检闸门 8→10（新增来源追溯闸 + 章节映射闸）；Story generate/review/update 三件套 SSOT 化 + 来源追溯步骤。
🆕 v3.9.3：新增「输出核心原则」第 4 条——禁止文档承载 changelog（设计/架构/模板/标准类文档只写当前生效内容，历史变更走 `source/CHANGELOG/{YYYY-MM-DD}-{主题}.md`）。
🆕 v3.9.1：修复 gate_intercept 对嵌套 state 不感知——4 处顶层 phase/currentStory 读取改用 get_active_phase/get_active_story 统一接口，消除嵌套 state 项目 src/ 写入被误拦为"设计阶段禁止写入源码目录"的回归。
🆕 v3.9.0：嵌套状态模型——单文件嵌套 state（prdState/drState/storyStates{N}），任意节点出发+向上归入，/ae-sdd 路由自动匹配/新建 state，改已管理 Story 自动重定位+重置子状态；命名只以顶层主体特征命名。
🆕 v3.8.2：修复五层记忆存取断裂；强化独立需求状态机入口，`state new --id --name` 创建 `{ID}--{name}` 状态机目录。
🆕 v3.8.0：自动化开关配置（`.ae-sdd/config.yaml` 的 `automation` 段，默认关闭）。开启后 6 个人工审核点改走 Tier 3 多 reviewer 联审共识，实现输入→结果全自动化；开工前预收集所有必需信息。
历史变更见 source/CHANGELOG/。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description, version; headings: L3:133 编码意图检测（暂离期间触发）; L3:164 G-00 项目资产（每次调用必过）; keyword_hits: 34 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:89 🎛️ 主流程监管器执行协议; L2:114 🔀 暂离与回归协议（流程偏离防护）; L3:283 开工前信息预收集（Step 1.5，仅自动化模式）; +9 more; keyword_hits: 142 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:162 🛡️ 门禁速查; L3:164 G-00 项目资产（每次调用必过）; L3:175 G-RA 需求分析准入（dr-generate / story-generate / task-generate 前必过）; +11 more; keyword_hits: 188 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:504 ①bis 前端视角接口审视; L2:650 🛠️ 工具 API 速查; keyword_hits: 95 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L3:257 配置（`.ae-sdd/config.yaml` 的 `automation` 段，SSOT）; L3:379 状态机子链（实际 state.json phase 值）; L2:480 流程状态与再启动; +3 more; keyword_hits: 90 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L3:118 暂离声明（AI 主动输出）; L3:205 G-DOC-STORAGE（任何产出文档落地前）; L3:217 G-DOC-CONSISTENCY（文档落地前 + G-00后）; +4 more; keyword_hits: 105 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 95; refs: ## 人工任务 \; .ae-sdd/; .ae-sdd/compact-trigger; +92 more; keyword_hits: 80 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:317 🔴 实现方案决策基线（Story→Task→Coding 全链路）; L3:517 🔍 审核点1.5 实现方案预确认（AI先答，用户确认）; keyword_hits: 78 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 22 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 2 | 89 | 🎛️ 主流程监管器执行协议 |
| 2 | 114 | 🔀 暂离与回归协议（流程偏离防护） |
| 3 | 118 | 暂离声明（AI 主动输出） |
| 3 | 133 | 编码意图检测（暂离期间触发） |
| 3 | 145 | 回归门（回归时强制执行） |
| 2 | 162 | 🛡️ 门禁速查 |
| 3 | 164 | G-00 项目资产（每次调用必过） |
| 3 | 175 | G-RA 需求分析准入（dr-generate / story-generate / task-generate 前必过） |
| 3 | 192 | G-CODEPLAN-SRC（CodingPlan → ⑤Coding 前） |
| 3 | 205 | G-DOC-STORAGE（任何产出文档落地前） |
| 3 | 217 | G-DOC-CONSISTENCY（文档落地前 + G-00后） |
| 3 | 225 | G-14 CodingPlan-Story 一致性（CodingPlan → ⑤Coding 前，与 G-CODEPLAN-SRC 正交） |
| 3 | 237 | G-AUTO-CONSENSUS 自动化联审共识（🆕 v3.8.0，仅自动化模式启用） |
| 2 | 253 | 🚀 自动化模式（🆕 v3.8.0 — 输入→结果全自动化） |
| 3 | 257 | 配置（`.ae-sdd/config.yaml` 的 `automation` 段，SSOT） |
| 3 | 269 | 行为分叉（每个审核点） |
| 3 | 277 | 阻断出口 |
| 3 | 283 | 开工前信息预收集（Step 1.5，仅自动化模式） |
| 3 | 295 | 禁止事项（自动化模式专属） |
| 2 | 305 | 🔴 输出核心原则（最高优先级） |
| 2 | 317 | 🔴 实现方案决策基线（Story→Task→Coding 全链路） |
| 2 | 328 | 🎯 智能路由 |
| 3 | 341 | 路由表（编码类） |
| 3 | 368 | 4类规格判定 |
| 3 | 379 | 状态机子链（实际 state.json phase 值） |
| 2 | 392 | 📖 人工审核主动讲解规范 |
| 2 | 406 | 🤖 多 Agent 机制摘要 |
| 2 | 426 | ⏱️ 节点级上下文压力（6个审核点边界必调） |
| 2 | 443 | 整体流程骨架 |
| 2 | 480 | 流程状态与再启动 |
| 2 | 502 | Phase 1 关键节点 |
| 3 | 504 | ①bis 前端视角接口审视 |
| 3 | 509 | ② Story Review |
| 3 | 514 | 🔍 审核点1 对话内呈现（必须直接输出） |
| 3 | 517 | 🔍 审核点1.5 实现方案预确认（AI先答，用户确认） |
| 2 | 522 | Phase 2 关键节点 |
| 3 | 524 | ④bis CodingProcess（🆕 v3.5.17） |
| 3 | 542 | 🔍 审核点2 逐文件核对（强制，禁止一锅端） |
| 3 | 545 | 🔍 审核点2.5 CodingPlan评审（必须直接输出） |
| 2 | 550 | Phase 3 关键节点 |
| 3 | 552 | ⑥ 完成判定（6.1~6.10） |
| 3 | 567 | ⑦ter 流程收尾合规自检（5维度，禁止裸✅收尾） |
| 3 | 579 | ⑧ 完成产出物 |
| 2 | 595 | PRD级完成判定（v3.3.0） |
| 2 | 620 | 子 SKILL 索引 |
| 2 | 650 | 🛠️ 工具 API 速查 |
| 2 | 687 | 🔧 维护工作流 |
| 2 | 701 | 禁止事项 |
| 2 | 716 | 执行清单（TodoWrite 1:1 映射） |

## Inline References

| ref |
| --- |
| ## 人工任务 \ |
| .ae-sdd/ |
| .ae-sdd/compact-trigger |
| .ae-sdd/config.yaml |
| .ae-sdd/memory/ |
| .ae-sdd/preflight-info.yaml |
| .ae-sdd/state.json |
| .auto-engineering/*/state.json |
| .auto-engineering/{WORKITEM-ID}/state.json |
| /ae-sdd CodeReview 这段 |
| /ae-sdd 优化这部分实现 |
| /compact |
| ae-sdd assets read/outline/section/query/stats |
| ae-sdd automation status |
| ae-sdd automation status/enable/disable |
| ae-sdd baseline inspect/create/diff |
| ae-sdd classify |
| ae-sdd context-pressure [--story <ID>] |
| ae-sdd db profiles/query/explain/audit |
| ae-sdd enter |
| ae-sdd evidence record/lookup |
| ae-sdd flow-violation-scan |
| ae-sdd gate ra-required/coding-required/doc-storage |
| ae-sdd gates check |
| ae-sdd gates check --only G-00 |
| ae-sdd gates check [--only <G-XX>] |
| ae-sdd git status/diff/log/blame/impact |
| ae-sdd health |
| ae-sdd init <dir> <projectKey> |
| ae-sdd iteration-check |
| ae-sdd memory enter/write/exit/read/search |
| ae-sdd perf report/doctor/clear |
| ae-sdd plugin list/validate/trace/init |
| ae-sdd preflight collect |
| ae-sdd ra-depth-scan |
| ae-sdd ra-implementation-scan |
| ae-sdd review start/collect/status/verify-exit/abort/retry-role |
| ae-sdd runtime compact |
| ae-sdd state lock --path <相对路径> --agent <agentId> |
| ae-sdd state lock/unlock |
| ae-sdd state prd-check-complete/prd-complete/prd-archive |
| ae-sdd state read |
| ae-sdd state read --work-item <WORKITEM-ID> --json |
| ae-sdd state read/write/next-step/confirm |
| ae-sdd state register-review-consensus |
| ae-sdd state relocate --story <ID> |
| ae-sdd state write --event skill-launched |
| ae-sdd state write --phase completed |
| ae-sdd state write --resume |
| ae-sdd update-check |
| ae-sdd verify plan |
| ae-sdd version/bump/init |
| ae-sdd-doc/ |
| ae-sdd-doc/{Category}/ |
| ae-sdd-install-skill.md |
| ae-sdd-update-skill.md |
| agent-orchestration-skill.md |
| code-review-skill.md |
| code-review-skill.md §📖 |
| coding-process-skill.md |
| coding-process-skill.md §A1.5 |
| coding-report-skill.md |
| coding-skill.md |
| design/ |
| document-storage-skill.md |
| dr-generate-skill.md |
| dr-review-skill.md |
| dr-update-skill.md |
| mavis session rotate --handoff-file summary.md |
| project-assets-update-skill.md |
| project-assets-update-skill.md §3 |
| proposal-skill.md |
| requirement-analysis-skill.md |
| review-loop-skill.md |
| source/CHANGELOG/{YYYY-MM-DD}-{主题}.md |
| source/SKILL.md |
| source/standards/ |
| story-generate-skill.md |
| story-generation-standard.md |
| story-input-checklist.md |
| story-review-skill.md |
| story-review-skill.md §📋 ①bis |
| story-review-skill.md §📖 |
| story-update-skill.md |
| template-layout-standard.md |
| test-generate-skill.md |
| test-review-skill.md |
| testcase-generate-skill.md |
| testcase-review-skill.md |
| tools/bin/ae-sdd |
| tools/lib/*.py |
| {series}-generate-skill.md |
| {series}-review-skill.md |
| {story}-Report-v{N}-r{M}.md |
| 优化/重构/改进 |
