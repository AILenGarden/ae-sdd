---
name: ae-sdd
version: 3.10.2
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
---

main_entry: true
triggers:
  - "/ae-sdd"
  - "启动自动化工程"
  - "从 DR 开始实现"
  - "端到端实现"
  - "继续流程"
  - "继续上次"
  - "ae-sdd-quick"
---

> ## 🔴 第一动作 — 5 步启动序列（禁止跳过任意步）
>
> **Step 1  工作区确认**
> - 检查当前目录下 `.ae-sdd/` 是否存在
> - 不存在 → `ae-sdd init <dir> <projectKey>`（创建 state.json，启动主流程监管器）→ 完成后进 Step 2
> - 已存在 → `ae-sdd state read`：`paused` → 输出暂停播报，三选一；in-progress → 续接播报；initialized/completed → Step 2
> - **🆕 v3.8.0 自动化模式检测**：`ae-sdd automation status`；若 `enabled:true` → 输出 `【自动化模式已启用 — 审核点将走 Tier 3 联审共识，跳过人工✅】` 并进 Step 1.5；否则按现状走 Step 2
>
> **Step 1.5  开工前信息预收集**（🆕 v3.8.0，仅自动化模式 `automation.preflightInfoCollection=true` 时执行）
> - 调 `ae-sdd preflight collect` 扫描输入材料（PRD/DR/Story）+ 项目资产 7 层索引
> - 识别 6 类待补信息：①第三方平台凭证（Key/Secret）②复用项选择（AI 找不到时问用哪个）③环境配置（DB/Redis/MQ 地址）④命名约定 ⑤已有对接方信息 ⑥数据初始化要求
> - 输出 `【开工前信息清单】`表格，**用户一次性补齐**后才进 Step 2；补齐信息写入 `.ae-sdd/preflight-info.yaml`（流程内只读引用）
> - 无待补信息 → 直接进 Step 2
>
> **Step 2  项目资产检查**
> - `ae-sdd gates check --only G-00`；不通过 → 加载 `project-assets-update-skill.md §3` 生成资产；完成后进 Step 3
>
> **Step 3  智能路由（任务类型 → 规格 → 入口节点）**
> - **任务类型一：ae-sdd 自更新** → `ae-sdd-update-skill.md`（监管器全权交接，state 标记 `ae-sdd-update`；收尾由 update-skill 负责）
> - **任务类型二：编码** → 按现有产物裁定规格：
>   - 已有 PRD → **大任务** → 从 RA 系列入
>   - 已有 DR → **中任务** → 从 DR 系列入
>   - 已有 Story → **小任务** → 从 Story 系列入
>   - BUG / 改逻辑 / 调整代码 → **微任务** → 从 Task 系列入
>   - 新需求无任何产物且非BUG → 🔴 阻断：`【主流程监管器 ❌ 阻断】新功能开发必须以 PRD 为起点。`
>
> **Step 4  执行编码流程**（详见 §🎛️ 主流程监管器执行协议）
>
> **Step 5  收尾交付**
> - ⑦ter 合规自检 → ⑧完成输出 → `ae-sdd state write --phase completed`
>
> **续接播报格式**：
> ```
> 【流程已恢复 — 主流程监管器续接】
> 项目 Key：{projectKey}  |  WorkItem ID：{WORKITEM-ID}  |  Story ID：{STORY-ID?}  |  规模：{大/中/小/微}
> 当前阶段：{phase}  |  已完成：{列表}  |  下一步：{SKILL名}，原因：{reason}
> ```
>
> **SKILL 调用声明格式**（每次加载子 SKILL 前输出）：
> ```
> 【主流程监管器 → 调用 SKILL】
> 调用：{skill}  |  理由：{reason}  |  期望产出：{list}  |  完成后 phase 推进至 {X}
> ```

---

## 🎛️ 主流程监管器执行协议（🆕 v3.10.3 3层Agent模型）

每系列标准 4 步。**主流程会话只编排+监管，不执行具体业务--委托子流程Agent接管。**

> **🆕 v3.10.3 3层Agent模型**：主流程会话 -> 子流程Agent（物理独立 session）-> sub-subAgent。
> 5大子流程（RA/DR/Story/TestCase/Coding）各委托1个子流程Agent，串行推进。
> 详见 §🤖 3层Agent模型。

| 步骤 | 动作 |
|------|------|
| 1 | `/compact`（系列入口，防跨系列上下文污染）；主流程判定子流程范围 + **委托子流程Agent**（`ae-sdd subprocess spawn --series <type> --entity-id <ID>`）；子流程Agent 创建对应实体 memory（`ae-sdd memory create`） |
| 2 | 子流程Agent 接管 -> 加载 `{series}-generate-skill.md` -> **起 sub-subAgent** 执行 generate -> 产物核查 |
| 3 | 子流程Agent 加载 `{series}-review-skill.md` -> **起 sub-subAgent** 执行 review -> **Loop**：有错+矫正<2 -> 回步骤2；矫正=2 -> Level 3 暂停等用户；连续2轮无新错 -> 步骤4 |
| 4 | 子流程Agent 汇总交付物 -> 回传主流程（`ae-sdd subprocess collect --agent-id <ID>`）-> **删自己的 memory** -> 人工审核：✅推进phase->下一系列；⚠️->重回步骤2；❌->paused<br>**🆕 v3.8.0 自动化模式**：联审共识 - 强制 Tier 3 派 3 个独立 session reviewer -> G-09B + G-REVIEW-LOOP + G-AUTO-CONSENSUS 全过即自动推进 |

**流程偏移与矫正**（自动执行，AI 无法绕过）：

| 漂移类型 | 检测 | 矫正级别 |
|---------|------|---------|
| B1 跳步 | gates check 未过但 AI 宣布进入下一系列 | L2 |
| B2 停滞 | correctionCounts[phase] ≥ 5 | L2 告警 |
| B3 伪完成 | gates check 不通过（产物核查替代自报） | L1→L2 |
| B4 旁路 | ≥5 后无进展 | L2→L3 |

- **L1** 静默注入；**L2** 输出 `【主流程监管器 🔴 矫正】`；**L3** `state.phase=paused`
- 恢复：`ae-sdd state write --resume`

---

## 🤖 3层Agent模型（🆕 v3.10.3）

### 层级职责

| 层级 | 角色 | 读什么 | 写什么 | 不做什么 |
|------|------|--------|--------|----------|
| **主流程会话** | 编排+监管+用户对话 | SKILL.md 编排层 + 主 state + subprocessAgents 状态 | 主 state.json + 委托契约 | 不读子SKILL正文、不读源码、不写流程文档 |
| **子流程Agent** | 接管1个系列 | 子流程所需 SKILL + 对应实体 memory | memory compact + 派 sub-subAgent + 交付物汇总 | 不直接写 state.json（回传主流程写） |
| **sub-subAgent** | 执行单系列内子任务 | 单系列子 SKILL + 任务卡 | 产出文档/代码/报告 | 不直接对话用户 |

### 隔离方式

子流程Agent = **物理独立 session**（mavis spawn / 新 workflow）。
- 复用 agent-orchestration-skill §8.4.5 自动化模式的物理独立要求。
- 环境不支持物理 spawn 时降级为逻辑隔离，标注 `agentMode: "logical-isolated"`。
- 自动化模式（`automation.enabled=true`）禁止降级，必须物理独立。

### 5大子流程串行委托

主流程按 RA -> DR -> Story -> TestCase -> Coding 依次委托子流程Agent。每个子流程Agent 完成交回主流程后，主流程再委托下一个。

### 通信协议

| 方向 | 内容 | 命令 |
|------|------|------|
| 主->子 | 委托契约（系列类型+实体ID+输入文档+交付物要求+deadline） | `ae-sdd subprocess spawn --series <type> --entity-id <ID>` |
| 子->主 | 交付物回传（汇总报告+产物路径+memory清理确认） | `ae-sdd subprocess collect --agent-id <ID> --deliverables '[...]'` |
| 主->主 | 监管（查询子流程状态） | `ae-sdd subprocess list` / `status --agent-id <ID>` |
| 子->主 | 故障升级（超时/产物缺失/门禁未过） | 主流程重派或升级用户 |

### 子流程Agent memory 管理

子流程Agent 启动时创建对应实体 memory（`ae-sdd memory create`），结束时删除（`ae-sdd subprocess collect` 自动清理）。common memory 跨子流程保留。详见 memory-management-skill.full.md。

---

## 🔄 compact 后重载协议（🆕 v3.10.3）

### 系列入口 compact（4步协议 step 1）

compact 后 -> 从 memory 重载完整上下文（boot+context+pending）-> 恢复子流程范围+当前系列+待决项。

### 中途 compact（子流程执行中上下文压力触发）

| 时机 | 动作 |
|------|------|
| compact 前 | `pre_compact_snapshot` 把当前系列进度/待决项写入 memory |
| compact 后 | `post_compact_reload` 从 memory 重载 -> 续接当前步骤（不跳步） |

### compact-trigger 读端（补齐 v3.10.3）

`state.prd_complete()` 写 `.ae-sdd/compact-trigger` 文件（claude-code runtime）。
`prompt_inject._check_compact_trigger()` 读 trigger -> 从 memory 重载 -> 清除 trigger。
主流程/无 memory 时走原逻辑（state.json 重建），向后兼容。

### PRD 收尾 compact

`prd_complete` -> `ae-sdd runtime compact`（含子流程 snapshot）-> summary.md + compact-trigger + prdStatus=compacted。

---

## 🔀 暂离与回归协议（流程偏离防护）

**核心原则：流程可以暂离讨论，但不能偏离——任何编码动作必须先回到流程。**

### 暂离声明（AI 主动输出）

当 AI 步出流程参与讨论/分析时，**必须**先输出：

```
【流程暂离 — 仅讨论模式】
当前 phase: {X}  |  暂离原因: {reason}
⚠️ 本模式下不执行任何代码改动，讨论结束后说「回归流程」继续。
```

暂离期间约束：
- 禁止写入 src/ 源码（Write/Edit/MultiEdit 到源码路径）
- 禁止运行编译/测试命令（Bash 非只读操作）
- 可以读代码、解释逻辑、回答问题

### 编码意图检测（暂离期间触发）

以下意图词出现时，暂离期间**必须先回归流程**，不得直接执行：

`改一下` / `加个` / `写代码` / `编码` / `修复` / `实现` / `提交` / `跑一下` / `试试` / `调整一下`

触发输出：
```
【主流程监管器 ❌ 阻断】当前处于讨论模式，编码前须先回归流程。
说「回归流程」执行回归检查，或 ae-sdd state read 确认当前节点。
```

### 回归门（回归时强制执行）

触发词：`回归流程` / `继续` / `开始做` / `接着做` / `继续上次`

回归动作（强制，不可跳过）：
1. `ae-sdd state read` — 确认当前 phase
2. 输出回归播报：

```
【流程回归 — 主流程监管器接管】
当前 phase: {X}  |  下一步: {next-step}  |  续接 SKILL: {skill}
```

3. 从当前节点续接，不跳步，不重置

---

## 🛡️ 门禁速查

### G-00 项目资产（每次调用必过）

| 规则 | 行为 |
|------|------|
| 资产文件存在 + 7层索引齐备 | 否 → 🔴 阻断，路由 `project-assets-update-skill §3` |
| 距 lastAuditedAt ≤ 30天 | 否 → 🟡 警告（不阻断）|

```bash
ae-sdd gates check --only G-00
```

### G-RA 需求分析准入（dr-generate / story-generate / task-generate 前必过）

| 规则 | 行为 |
|------|------|
| RA 文档存在 + 8核心维度齐全 | 否 → 🔴 阻断 |
| RAModel 12维完整 + RA-G01~G16全过 | 否 → 🔴 阻断 |
| 实现视角七要素完整（数据源/数据流/定义/复用证据/成本反驳/开发疑问/DR交接）| 否 → 🔴 阻断（G-RA-6）|
| 5问自检阻断项=0 + 所有🔴缺口已解决 | 否 → 🔴 阻断 |
| RA距今 ≤30天 | 否 → 🟡 警告 |
| **微任务/BUG/配置类豁免** | 走 coding-process-skill 从 CodingPlan 系列入（无文档直出，v3.10.0 砍 Task）|

```bash
ae-sdd gate ra-required --project <project-root>
```

覆盖：G-RA-1~G-RA-6 + G-RA-FLOW-VIOLATION。

### G-CODEPLAN-SRC（CodingPlan → ⑤Coding 前）

| 规则 | 行为 |
|------|------|
| 关键类骨架每个新增/修改类附【已读源码：{路径}】标记 | 否 → 🔴 阻断 |
| 标记文件真实存在 | 否 → 🔴 阻断 |
| 待核实清单为空 | 否 → 🔴 阻断（视为草案）|
| 微任务（无骨架章节）豁免 | skipped |

```bash
ae-sdd gates check --only G-CODEPLAN-SRC
```

### G-DOC-STORAGE（任何产出文档落地前）

| 规则 | 行为 |
|------|------|
| 产出文档落在合规根目录（由 document-storage `resolve_path()` 定位）| 否 → 🔴 阻断 |
| 落地前必须调 resolve_path() | 硬编码绝对路径 → 🔴 阻断 |

```bash
ae-sdd gates check --only G-DOC-STORAGE
ae-sdd gate doc-storage --path <路径> --intent <intent> --project <projectKey>
```

### G-DOC-CONSISTENCY（文档落地前 + G-00后）

项目侧记忆（AGENTS.md / MEMORY.md / agent.md / CLAUDE.md）中"文档工作区/文档根"表述须与 `.ae-sdd/config.yaml` 的 `docWorkspacePath` 一致，config 是 SSOT。

```bash
ae-sdd gates check --only G-DOC-CONSISTENCY
```

### G-14 CodingPlan-Story 一致性（CodingPlan → ⑤Coding 前，与 G-CODEPLAN-SRC 正交）

| 规则 | 行为 |
|------|------|
| CodingPlan 含 Story 文档引用且文件存在 | 否 → 🔴 阻断 |
| 测试章节 AC ID 与 Story AC 对齐（Story有AC时至少1个） | 否 → 🔴 阻断 |
| 偏离设计有 Proposal 引用 | 否 → 🔴 阻断 |

```bash
ae-sdd gates check --only G-14
```

### G-AUTO-CONSENSUS 自动化联审共识（🆕 v3.8.0，仅自动化模式启用）

| 规则 | 行为 |
|------|------|
| 自动化模式开启 + 当前审核点在 `automatedReviewPoints` 白名单 | `state.reviewConsensus[point].passed=true` 否 → 🔴 阻断 |
| reviewer 独立性（复用 G-09B）| `state.activeAgents` 有 ≥3 个 sessionId≠root 的 reviewer，否 → 🔴 阻断 |
| 交叉对比完成（§8.4.3）| `reviewConsensus[point].reviewers` 字段 3 份报告齐备，否 → 🔴 阻断 |
| 非自动化模式 / 审核点不在白名单 | skipped（回退人工审核）|

```bash
ae-sdd gates check --only G-AUTO-CONSENSUS
ae-sdd state register-review-consensus --point {1\|1.5\|2\|2.5\|4\|5} --tier 3 --passed {true\|false} --rounds {N}
```

---

## 🚀 自动化模式（🆕 v3.8.0 — 输入→结果全自动化）

> **定位：** 默认关闭的"全自动化开关"。开启后 6 个人工审核点（1/1.5/2/2.5/4/5）改走 Tier 3 多 reviewer 联审共识，跳过所有人工✅，实现 ae-sdd 输入→结果。联审机制复用 `agent-orchestration-skill §8.4`（已存在），本节只管开关与审核点行为分叉。

### 配置（`.ae-sdd/config.yaml` 的 `automation` 段，SSOT）

```yaml
automation:
  enabled: false              # 总开关（默认关）
  reviewerTier: 3             # 强制三审
  preflightInfoCollection: true
  onConsensusStall: pause     # pause=paused等用户 / fail=标记失败
  automatedReviewPoints: [1, 1.5, 2, 2.5, 4, 5]
  enabledAt: ""               # 审计时间戳，AI 不得自行改
```

### 行为分叉（每个审核点）

| 模式 | 审核点行为 |
|------|----------|
| 默认（enabled=false）| 现状：AI 讲解 → 等用户 ✅/⚠️/❌ |
| 自动化（enabled=true 且点在白名单）| AI 讲解（听众变 reviewer）→ 强制 Tier 3 派 3 独立 session reviewer → §8.4.3 交叉对比 → 写 `reviewConsensus[point]` → G-09B+G-REVIEW-LOOP+G-AUTO-CONSENSUS 全过即自动推进 phase |
| 自动化但点不在白名单 | 回退人工审核（该点仍等用户✅）|

### 阻断出口

联审 2 轮矫正未决 → 按 `onConsensusStall`：
- `pause`：`state.phase=paused`，输出完整问题清单等用户介入（**默认**，避免 AI 带病狂奔）
- `fail`：标记失败，终止流程

### 开工前信息预收集（Step 1.5，仅自动化模式）

开工前一次性向用户收集所有必需信息，开工后不再打断。识别 6 类待补信息：
1. 第三方平台凭证（极光/融云 Key、Secret 等）
2. 复用项选择（AI 找不到时问用哪个已有实现）
3. 环境配置（DB/Redis/MQ 地址）
4. 命名约定
5. 已有对接方信息
6. 数据初始化要求

详见 Step 1.5 协议与 `ae-sdd preflight collect`。

### 禁止事项（自动化模式专属）

| 禁止 | 正确做法 |
|------|---------|
| AI 自行 `automation enable` | 必须用户显式操作；`enabledAt` 由 CLI 写入 |
| 自动化模式用逻辑多视角降级 | 必须物理 3 独立 session；环境不支持 → paused（见 `agent-orchestration-skill §8.4.5`）|
| 联审不通过仍推进 phase | G-AUTO-CONSENSUS 阻断；2 轮未决 → paused |

---

## 🔴 输出核心原则（最高优先级）

| 原则 | 要求 |
|------|------|
| 基于事实 | 所有输出必须有明确来源（DR/PRD/资产/用户告知/代码读取）|
| 禁止猜测 | 不确定 → 标 `{待确认}` 并主动询问；禁止推断后输出 |
| 禁止杜撰 | 不得编造不存在于输入材料中的规则/字段/类名/配置项 |
| 🆕 禁止文档承载 changelog | 设计/架构/模板/标准类文档只写**当前生效内容**；历史变更走 `source/CHANGELOG/{YYYY-MM-DD}-{主题}.md`，文档内仅用一句引用（如「详见 CHANGELOG/...」）指向。混写会破坏主题连续性、加剧检索劣化、让 git blame 失真——属于坏习惯，不是细节。 |
| 🆕 Story 输出边界 | Story 只写当前生效的需求与实现契约；不得写生成过程、CHANGELOG 或 DR 文档。DR 在 Story 链路中仅作只读输入，缺失时阻断，不得自动生成。 |

---

## 🔴 实现方案决策基线（Story→Task→Coding 全链路）

| 步骤 | 要求 |
|------|------|
| ① 现有能力复用扫描 | 所有实现点先扫项目资产/历史Task/公共组件；有则复用，不复用必须写明原因 |
| ② 业内成熟方案参考 | 非平凡实现点（状态机/幂等/消息/集成）列业内方案并说明选择理由 |
| ③ 五维代码质量 | 可用性/高效性/可维护性/健壮性/可读性，任一不达标 → 🔴 阻断 |
| ④ 核心能力归属唯一 | 每个核心业务能力只能有一个唯一实现点，多入口只能调用它 |

---

## 🎯 智能路由

**调用顺序：** ① 自更新识别（🆕 v3.10.2 含消歧：代码上下文 → 不进自更新） → ② 任务类型（编码） → ③ 规格裁定 → ④ G-RA 门禁（大任务时）

> 🆕 v3.10.2 **micro 意图分流**：`/ae-sdd 优化这部分实现` / `/ae-sdd CodeReview 这段` 不再误进自更新、也不走完整 Coding 全链。进微链后按 `entry_node`（OPTIMIZE/CODE_REVIEW）只调相应能力。消歧优先级：**self-update 上下文（ae-sdd/SKILL/流程）> 代码上下文**——`优化 ae-sdd` 走自更新，`优化这段实现` 走 micro-optimize。详见下方路由表「微-优化 / 微-审查」两行 + code-review-skill §无文档轻量准入。

> 🆕 v3.9.0 **路由自动 state 匹配**：路由时 `classify.match_state()` 自动分析需求特征（提取 PRD/DR/Story ID + 判定 Bug/改 Story）→ 扫描现有嵌套 state → 命中则 relocate/absorb，未命中则 create_nested。匹配优先级：
> 1. R4 Bug/微任务不改 Story → `create_flat`
> 2. R5 Story 命中现有 state → `relocate` + 重置子状态（若下游已动）
> 3. R2 DR 归入已存在的 PRD state → `absorb_into_prd`
> 4. R2 Story 归入已存在的 DR state → `absorb_into_dr`
> 5. R7 无匹配 → `create_nested`（以当前主体为顶层，R6 顶层命名）

### 路由表（编码类）

| 规格 | 已有产物 / 场景 | 入口系列 | G-RA |
|------|----------------|---------|------|
| **大** | 已有 DR | `dr-generate-skill.md`（DR 系列）| **必过**（RA 为前置）|
| **中** | 已有 Story | `story-generate-skill.md`（Story 系列）| **必过** |
| **小** | 已有 Story+TestCase | CodingPlan 系列 |
| **微** | BUG / 改逻辑 / 调整代码（无完整产物链）| CodingPlan 系列 |
| **🆕 微-优化** | 优化/重构/改进代码（无文档，entryNode=OPTIMIZE）| coding-process（轻量：跳骨架分解，直 CodeAnalysis→Coding）| — |
| **🆕 微-审查** | 审查/CodeReview/评审代码（无文档，entryNode=CODE_REVIEW）| code-review（无文档轻量准入，对话内出结论）| — |
| — | 新需求无任何产物且非BUG | 🔴 阻断 | — |

> 🆕 v3.10.2 **微-优化 / 微-审查 意图分流**：进微链后按 `state.entryNode` 只调单个能力，跳过无关步骤。
> - 微-优化：`initialized → coding-process（轻量）→ coding → completed`，跳 test-running/code-reviewed。
> - 微-审查：`initialized → code-reviewed → completed`，跳 coding-process/coding/test-running；gate 跨步跳跃对 OPTIMIZE/CODE_REVIEW + scale=微 放行（复用 v3.5.15 BUG 豁免范式）。
> - 消歧（classify.py）：`优化/重构/改进` + 代码上下文 + 非 self-update 上下文 → OPTIMIZE；含 ae-sdd/SKILL/流程 词 → 仍走自更新。

**非编码类路由：**

| 输入/场景 | 路由 |
|----------|------|
| 修改SKILL / 优化ae-sdd | `ae-sdd-update-skill.md`（监管器全权交接）。🆕 v3.10.2 消歧：仅当含 ae-sdd/SKILL/流程 上下文词时进本路由；`优化这部分实现` 等代码上下文 → 微-优化（见上表）|
| 安装/升级ae-sdd | `ae-sdd-install-skill.md` |
| 修改/BUG/生产故障 proposal | `proposal-skill.md` |
| 放文档哪里 / 命名 | `document-storage-skill.md` + G-DOC-STORAGE |
| 从X继续 / 重入 | 读 state.json 判重入点再路由 |

### 4类规格判定

| 规格 | 判定依据 | 入口节点 |
|------|---------|---------|
| **大** | 已有 DR | `dr-generate-skill.md`（DR 系列）| **必过**（RA 为前置）|
| **中** | 已有 Story（无DR）| Story 系列 |
| **小** | 已有 Story+TestCase | CodingPlan 系列 |
| **微** | BUG / 改逻辑 / 调整代码（无完整产物链）| CodingPlan 系列 |
| **🆕 微-优化** | 优化/重构/改进 + 代码上下文（非 ae-sdd）+ 无文档 | coding-process 轻量→coding |
| **🆕 微-审查** | 审查/CodeReview/评审代码（非 ae-sdd）+ 无文档 | code-review 轻量准入→对话结论 |

### 状态机子链（实际 state.json phase 值）

| scale | phase 序列 | 适用 |
|-------|-----------|------|
| 大(10) | initialized->ra-generated->dr-generated->story-generated->testcase-generated->coding-process->coding->test-running->code-reviewed->completed | 4 loop（RA-DR-Story-TestCase）+ Coding/Testing |
| 中(9) | initialized->dr-generated->story-generated->testcase-generated->coding-process->coding->test-running->code-reviewed->completed | 3 loop（DR-Story-TestCase）+ Coding/Testing，跳 RA |
| 小(6) | initialized->coding-process->coding->test-running->code-reviewed->completed | 有Story+TestCase，直出CodingPlan |
| 微(6) | initialized->coding-process->coding->test-running->code-reviewed->completed | BUG/调整，无文档直出CodingPlan |
| 🆕 微-优化 | initialized->coding-process（轻量）->coding->completed | OPTIMIZE entryNode；跳 test-running/code-reviewed；gate 跨步放行 |
| 🆕 微-审查 | initialized->code-reviewed->completed | CODE_REVIEW entryNode；跳 coding-process/coding/test-running；gate 跨步放行 |

---

## 📖 人工审核主动讲解规范

**双支柱（缺一不可）：** ① 先讲故事（叙述性，讲清背景/意图）+ ② 对话内直接呈现（结构化，用户无需开文件即可审核）

| 审核节点 | 讲解主体 | 模板位置 |
|---------|---------|---------|
| Story Review | 业务设计故事 | `story-review-skill.md §📖` |
| CodingProcess | 骨架分解+CodeAnalysis->CodingPlan | `coding-process-skill.md §A1.5` |
| Code Review | 代码实现故事 | `code-review-skill.md §📖` |

反模式：`❌ "文档已生成请审核"` / `❌ 只讲故事不展示内容` / `❌ 一次抛大坨等整体确认`

---

## 🤖 多 Agent 机制摘要

主会话职责：编排/调用声明/用户对话/状态落盘/审核点讲解。具体生成/读源码/写文档→派 sub-agent。

**何时启用**：3+独立Story / 需独立验证 / Review节点Tier2+（默认双/多reviewer）/ 跨多源多工具

**角色库**：`story-writer` / `story-reviewer`（Tier判定1-3个）/ `testcase-writer` / `task-writer` / `coder` / `test-runner` / `code-reviewer`（Tier判定1-3个）/ `test-verifier`（Test Review / ⑥.10 强制）

⑥.10 `test-verifier` 是硬门禁：主 agent 自称"测试通过"无效，必须 test-verifier 独立验证。

详细任务卡模板/报告协议 → [`agent-orchestration-skill.md`](../cross-cutting/agent-orchestration-skill.md)

**🆕 v3.8.1 S-3 文件意图锁**（防多 sub-agent 并发写同一产物丢更新）：
- 规则：禁止多个 sub-agent 并发写同一文件/同一目录（`agent-orchestration-skill §禁止模式`）
- 落地：`state.fileLocks` 中央意图锁 + PreToolUse hook 冲突告警（≥2 活跃 agent 时触发）
- 用法：sub-agent 写产物前 `ae-sdd state lock --path <相对路径> --agent <agentId>`，写完 `state unlock`
- TTL 30 分钟防崩溃死锁（惰性失效，过期锁可被抢占）；首版 hook 仅 warn 不阻断

---

## ⏱️ 节点级上下文压力（6个审核点边界必调）

```bash
ae-sdd context-pressure --story {STORY-ID}   # report-only，不阻断，不改state
```

| 审核点 | 时机 |
|--------|------|
| 1 | Phase 1 末，用户✅后 |
| 1.5 | Phase 2 头，用户✅后 |
| 2 | Task文档完成，用户✅后 |
| 2.5 | CodingPlan评审，用户✅后 |
| 4 | CodeReview完成，用户✅后 |
| 5 | PRD完成，critical时强烈建议进入PRD收尾+compact |

---

## 整体流程骨架

```
Phase 1 设计阶段
  ① 生成Story（story-generate-skill）
  ①bis 前端视角接口审视（6维度→story-review-skill §①bis）
  ② Story Review（story-review-skill，含F-Stage前端契约）
  ③ 生成测试用例（testcase-generate-skill）
  ③bis TestCase Review（testcase-review-skill，TC-1~TC-9循环，2轮无新增退出）
  ③ter 业务逻辑汇总
  🔍 审核点1：设计完成确认 + context-pressure

Phase 2 实现阶段
  🔍 审核点1.5：实现方案预确认 + context-pressure
  ④ CodingProcess（coding-process-skill）：骨架分解->CodeAnalysis->CodingPlan（v3.10.0 砍 Task，骨架分解合并进 §A1.5）
  ④bis CodingProcess（coding-skill能力库）：CodeAnalysis→CodingPlan→Execute
        state.phase=coding-process；G-CODEPLAN-SRC + G-14 通过后才能 Execute
  🔍 审核点2.5：CodingPlan评审（14条门禁+CodingModel 11维）+ context-pressure
  🔍 审核点2.5：CodingPlan评审（14条门禁+CodingModel 11维）+ context-pressure
  ⑤ Execute（按确认后的CodingPlan编码）

Phase 3 验证阶段
  ⑥ Test 系列（test-generate-skill→test-review-skill，按监管器4步子流程；⑥.10由test-verifier独立验证）
  ⑥bis 编码后全切面一致性核查闸（→ code-review-skill §闸1）
  ⑦ CodeReview报告（→ code-review-skill，templates/coding/be-codereview-template.md）
  ⑦bis 全链路对称性核查闸（→ code-review-skill §闸2）
  🔍 审核点4：CodeReview完成确认 + context-pressure
  ⑦ter 流程收尾合规自检（5维度：7t-1~7t-5，禁止裸✅收尾）
  ⑧ 完成输出

PRD收尾（可选）：
  🔍 审核点5：PRD完成确认（4层AND：G-PRD-1~4）+ context-pressure
  ae-sdd state prd-check-complete → prd-complete → compact
```

---

## 流程状态与再启动

**state.json** 路径：`.auto-engineering/{WORKITEM-ID}/state.json`（🆕 v3.10.1 WORKITEM-ID 带随机 UUID 前缀，如 `{uuid}-PRD-001`，保证同业务名不撞目录）

关键字段：`scale` / `entryNode` / `phase` / `stateMachineId`（带 UUID 前缀）/ `stateMachineName`（纯业务名）/ `stateUuid` / `currentWorkItem` / `currentStory` / `completedSteps` / `correctionCounts`

**再启动判定**：

| 用户意图 | AI动作 |
|---------|--------|
| 继续/接着做 | 读 state.json，恢复当前步骤 |
| 从某步骤继续 | 从指定步骤恢复（不可倒退已完成关键门禁）|
| 放弃重新开始 | 确认后删 state.json，从 Phase 1 ① 重启 |
| 切换WorkItem/Story | 保留当前 state.json，切换 `.auto-engineering/{WORKITEM-ID}/state.json` |
| 偏离流程 | 简短回应，不更新 state |
| PRD收尾 | 跑 prd-check-complete → 4层AND全过 → 审核点5 → prd-complete |
| 🆕 v3.9.0 改已管理 Story | `ae-sdd state relocate --story <ID>` 重定位到所属 state + 只重置该 Story 子状态到 story-generated（R5）。🆕 v3.9.22：重置同时写入 `artifactInvalidated` 信号，下次 task-generate 检测到即强制全量重建 Task（详见 CHANGELOG/2026-07-10-v3.9.22-reset-artifact-invalidation.md）|

**paused 恢复**：输出原因 + pausedFromPhase，三选一：恢复/新任务/查看状态

---

## Phase 1 关键节点

### ①bis 前端视角接口审视
6维度（契约完整性/调用流程/状态展示/错误码/边界场景/联调支持）→ 详细清单见 `story-review-skill.md §📋 ①bis`

AE编排层门禁：① 完成后必做 ①bis；② Story含"接口契约"章节（v3.9.5 起合并，含 ①bis 6 维度）；③ Story Review含F-Stage

### ② Story Review
循环：挖掘→判定→Proposal→按Proposal修复→再挖掘→退出（连续2轮无新增）

退出协议 → `review-loop-skill.md`；F-Stage未通过 → Story Review不完整

### 🔍 审核点1 对话内呈现（必须直接输出）
AC验收标准完整列表 + 核心接口一览 + 关键设计决策 + 已识别风险点 + 测试用例数量

### 🔍 审核点1.5 实现方案预确认（AI先答，用户确认）
AI直接输出：核心业务理解 + 接口/依赖 + 分层实现思路 + 并发/事务策略 + 异常处理思路

---

## Phase 2 关键节点

### ④bis CodingProcess（🆕 v3.5.17）
持有 Task→Coding 全流程编排，调 `coding-skill` 能力库：
1. **Phase A CodeAnalysis**：加载5上下文 + 调 `CodingSkill.Plan`
2. **产物核查**：G-CODEPLAN-SRC + G-14
3. **审核点2.5**：用户确认CodingPlan（14条门禁全过）
4. **Phase B Execute**：`CodingSkill.Execute`，严格按确认后的CodingPlan

④→⑤ 调用协议9项前置条件（任一未满足禁止 Execute）：
1. Task 0~N全部已生成
2. 每个Task含 CodingModel决策记录（11维）
3. 每个Task含任务级CodePlan
4. TR-1~TR-7全通过
5. 统一版CodingPlan.md生成+14条门禁通过
6. CodingPlan已 resolve_path 落地（G-DOC-STORAGE ✅）
7. 用户明确确认CodingPlan
8. CodingModel决策记录已复核
9. G-CODEPLAN-SRC + G-14 通过

### 🔍 审核点2 逐文件核对（强制，禁止一锅端）
按文件名字典序逐文件：AI完整读出→AI指出重点→用户逐文件✅/⚠️/⏸️→⚠️则重走该文件

### 🔍 审核点2.5 CodingPlan评审（必须直接输出）
14条门禁通过状态 + CodingModel 11维决策 + 风险Task + 关键类骨架（含【已读源码】标记）

---

## Phase 3 关键节点

### ⑥ 完成判定（6.1~6.10）

| # | 条件 | 通过标准 |
|---|------|---------|
| 6.1 | mvn compile | BUILD SUCCESS |
| 6.2 | 服务启动 | Started XxxApplication；端口监听；无BeanCreationException |
| 6.3 | 主流程接口 | L2 HTTP 100% Pass（能跑真实HTTP禁用MockMvc）|
| 6.4 | 错误码映射 | 业务异常→对应错误码 |
| 6.5 | DB写落库 | L3 INSERT后SELECT可查到 |
| 6.6 | 事务边界 | 失败全回滚；事务外操作异步 |
| 6.7 | 所有测试 | L1/L2/L3/L4全Pass |
| 6.8 | 无Open问题 | 开发记录无Open |
| 6.9 | 测试报告已出 | `TEST_REPORT`（`{story}-Report-v{N}-r{M}.md`）存在 |
| 6.10🔴 | 测试真实性 | test-verifier独立验证，BLOCKER=0；详见 `test-review-skill.md` |

### ⑦ter 流程收尾合规自检（5维度，禁止裸✅收尾）

| 维度 | 命令 | 通过标准 |
|------|------|---------|
| 7t-1 全门禁 | `ae-sdd gates check` | failed=0 |
| 7t-2 文档位置 | `gates check --only G-DOC-STORAGE` | stray=[] |
| 7t-3 state完整 | `ae-sdd state read --work-item <WORKITEM-ID> --json` | phase合法+currentWorkItem非空；Story 流程需 currentStory非空 |
| 7t-4 产出物齐全 | 逐项核§⑧产出物表 | 全部文件真实存在 |
| 7t-5 无遗留🔴 | 读CodeReview报告 | 无Open态🔴问题 |

🟢 可自愈→修复后重跑该维度；🔴 阻断→升级用户，禁止裸✅进⑧

### ⑧ 完成产出物

| 产出物 | 路径定位 |
|--------|---------|
| Story文档 | `resolve_path(intent="STORY", storyId)` |
| Task文档 | `resolve_path(intent="TASK", workItemId, storyId?, taskId)` |
| Coding报告 | `resolve_path(intent="CODING_REPORT", workItemId, storyId?, version={v,r})` |
| CodeReview报告 | `resolve_path(intent="CODE_REVIEW", workItemId, storyId?, version={v,r})` |
| 测试用例 | `resolve_path(intent="TESTCASE", workItemId, storyId?)` |
| 测试报告 | `resolve_path(intent="TEST_REPORT", workItemId, storyId?, version={v,r})` |
| 源代码 | 工程目录 |

路径均通过 `documentStorage.resolve_path()` 定位，禁止硬编码（路径模板见 `document-storage-skill.md` §1.3）。

---

## PRD级完成判定（v3.3.0）

4层AND闸：
- G-PRD-1：∀ Story 的 codeReviewReport存在 + sevenBisPassed + userConfirmedAt非空
- G-PRD-2：∀ Story 的 sevenBisMatrix 无🔴断链
- G-PRD-3：crossStoryDeps全部verifiedAt + crossStoryResidualRisks全部有mitigationPlan
- G-PRD-4：prdReview.confirmedAt非空

```bash
ae-sdd state prd-check-complete --prd {PRD-ID}   # 校验4层AND，不改状态
ae-sdd state prd-complete --prd {PRD-ID} --runtime {runtime}   # 4层AND通过后执行
```

**PRD收尾强制3步**：① prd-check-complete（all_pass==true）→ ② prd-complete → ③ compact+写next-prd指针

**🆕 v3.8.1 S-5 runtime 差异化**（`prd-complete` 按 `--runtime` 分支生成 summary.md + 差异化交接）：

| runtime | prd-complete 行为 | compact 交接方式 |
|---------|-------------------|------------------|
| `mavis` | 生成 summary.md + prdStatus→awaiting_compact | `mavis session rotate --handoff-file summary.md` |
| `claude-code` | 生成 summary.md + 写 `.ae-sdd/compact-trigger` | UserPromptSubmit hook 读 trigger 注入 `/compact` |
| `codex` | 生成 summary.md + 标注"待调研" | codex 无原生 compact（人工衔接 summary.md） |

---

## 子 SKILL 索引

| SKILL | 文件 | 职责 |
|-------|------|------|
| Requirement Analysis | `requirement-analysis-skill.md` | PRD→RA文档+规模裁定 |
| DR Generate | `dr-generate-skill.md` | RA→DR草稿（规模=大）|
| DR Review | `dr-review-skill.md` | DR 5阶段评审 |
| Story Generate | `story-generate-skill.md` | DR→Story（7阶段挖掘）|
| Story Review | `story-review-skill.md` | Story缺陷挖掘循环 |
| TestCase Generate | `testcase-generate-skill.md` | 测试用例生成 |
| TestCase Review | `testcase-review-skill.md` | 测试用例缺陷挖掘循环（TC-1~TC-9）|
| Story Update | `story-update-skill.md` | Story文档更新 |
| CodingProcess | `coding-process-skill.md` | 骨架分解+CodeAnalysis+CodingPlan（v3.10.0 砍 Task）|
| Coding Process | `coding-process-skill.md` | Task→Coding 编排：CodeAnalysis→CodingPlan→Execute |
| Coding | `coding-skill.md` | CodingSkill.Plan/Execute 能力库 |
| Coding Report | `coding-report-skill.md` | Coding 报告生成 |
| Test Generate | `test-generate-skill.md` | 运行编译/启动/L1-L4 测试并生成测试报告 |
| Test Review | `test-review-skill.md` | test-verifier 独立复核测试真实性与证据链 |
| Code Review | `code-review-skill.md` | Phase 3 代码评审与一致性/对称性核查 |
| DR Update | `dr-update-skill.md` | DR文档更新 |
| Project Assets Update | `project-assets-update-skill.md` | G-00门卫SOP |
| Document Storage | `document-storage-skill.md` | 路径解析/命名/重入判定 |
| Proposal | `proposal-skill.md` | 跨域改动4段SOP |
| Review Loop | `review-loop-skill.md` | Review Loop公共协议 |
| Agent Orchestration | `agent-orchestration-skill.md` | 多Agent编排/派活/Tier判定 |
| ae-sdd Update | `ae-sdd-update-skill.md` | ae-sdd自身维护 |
| ae-sdd Install | `ae-sdd-install-skill.md` | 安装引导 |

---

## 🛠️ 工具 API 速查

| 分组 | 命令 | 用途 |
|------|------|------|
| **资产** | `ae-sdd gates check --only G-00` | 项目资产门卫 |
| | `ae-sdd assets read/outline/section/query/stats` | 资产读取 |
| **状态机** | `ae-sdd state read/write/next-step/confirm` | phase读写/推进/审核token |
| | `ae-sdd state prd-check-complete/prd-complete/prd-archive` | PRD级完成判定 |
| **路由** | `ae-sdd classify` | 4维判定 |
| **门禁** | `ae-sdd gates check [--only <G-XX>]` | 30门禁扫描（🆕 v3.8.0 +G-AUTO-CONSENSUS） |
| | `ae-sdd gate ra-required/coding-required/doc-storage` | 单点校验 |
| | `ae-sdd flow-violation-scan` | RA流程违规审计 |
| | `ae-sdd ra-depth-scan` | RA机械派生深度扫描 |
| | `ae-sdd ra-implementation-scan` | RA实现视角七要素扫描（G-RA-6） |
| | `ae-sdd enter` | 入口凭证（关卡1）|
| **Toolset** | `ae-sdd memory enter/write/exit/read/search` | Phase-aware memory gate |
| | `ae-sdd db profiles/query/explain/audit` | 本地profile DB |
| | `ae-sdd git status/diff/log/blame/impact` | 只读Git证据 |
| **自动化** | `ae-sdd automation status/enable/disable` | 🆕 v3.8.0 自动化开关配置 |
| | `ae-sdd preflight collect` | 🆕 v3.8.0 开工前信息预收集 |
| | `ae-sdd state register-review-consensus` | 🆕 v3.8.0 写联审共识结果 |
| | `ae-sdd state lock/unlock` | 🆕 v3.8.1 文件意图锁（防多 agent 并发写） |
| **维护** | `ae-sdd health` | 10项健康度自检（🆕 v3.8.1 第10项规则-工具同步状态） |
| | `ae-sdd update-check` | UC-01~16更新依赖图谱 |
| | `ae-sdd iteration-check` | 设计-实现一致性迭代检查 |
| | `ae-sdd context-pressure [--story <ID>]` | 上下文压力软提示 |
| | `ae-sdd perf report/doctor/clear` | Runtime Stats 统计、诊断、清理 |
| | `ae-sdd version/bump/init` | 版本/初始化 |
| | `ae-sdd plugin list/validate/trace/init` | 三层SKILL注册表 |
| | `ae-sdd runtime compact` | compact适配层 |
| **增量质量** | `ae-sdd baseline inspect/create/diff` | G-CODE-1 历史 debt baseline 与 Story delta；创建必须显式批准 |
| | `ae-sdd verify plan` | 按变更类别生成最小验证计划 |
| | `ae-sdd evidence record/lookup` | 写入/查询 input-command-toolchain-artifact fingerprint 对齐的成功证据 |
| **Review Batch** | `ae-sdd review start/collect/status/verify-exit/abort/retry-role` | Review Batch v2；`review-loop` 保留为兼容入口 |

---

## 🔧 维护工作流

```
修改 source/SKILL.md 或子SKILL
    → ae-sdd update-check（UC-01~16全绿）
    → scripts/dev-sync.sh（分发到 ~/.claude/skills/）
    → post-commit hook 自动触发
```

SSOT：`source/SKILL.md` + 子SKILL + `source/standards/`
派生：`tools/bin/ae-sdd` + `tools/lib/*.py`（规则层→工具层）

---

## 禁止事项

| 禁止 | 正确做法 |
|------|---------|
| 跳过Phase 1直接写代码 | 必须先完成设计阶段 |
| 跳过CodingPlan | ⑤前必须有CodingPlan+14条门禁+用户确认 |
| 测试伪造通过（8类手段）| 见 `test-review-skill.md` + G-09 |
| "修复测试"代替"修复代码" | 修改测试代码必须标注原因+获用户确认 |
| 人工审核节点自动决策 | 待讨论项必须询问用户 |
| Task审核一锅端 | 逐文件自上而下核对，每文件单独✅ |
| 审核节点只丢文档不讲解 | 先讲"故事"（叙述性）再对话内直接呈现 |
| 裸✅收尾 | ⑦ter自检全过才能进⑧ |

---

## 执行清单（TodoWrite 1:1 映射）

| # | 动作 | 门禁 |
|---|------|------|
| 0 | 工作区确认（projectKey/gitPath已知）| 已知才继续 |
| 0b | G-00项目资产检查 | 通过才继续 |
| 1 | 路由判定（1.5自更新→1.6来源→1.7规模→1.8 G-RA）| 路由明确才继续 |
| 2 | 生成Story（story-generate-skill）| 文件存在 |
| 2 | Story loop【Generate-Review】（story-generate + story-review 含F-Stage）| 循环退出（3轮无新增）= phase story-generated |
| 2b | ①bis 前端视角接口审视 | 6维度通过；Story含"接口契约"章节（含①bis 6维度） |
| 3 | （v3.10.1 已合并入步骤2 loop）| - |
| 4 | TestCase loop【Generate-Review】（testcase-generate + testcase-review TC-1~TC-9）| 循环退出（3轮无新增）= phase testcase-generated |
| 4a | （v3.10.1 已合并入步骤4 loop）| - |
| 4b | 审核点1（设计阶段完成确认）| 用户✅；context-pressure |
| 4c | 审核点1.5（实现方案预确认）| 用户✅；context-pressure |
| 5 | CodingProcess 骨架分解+CodeAnalysis（coding-process-skill §A1.5+§A2）| CodingPlan 骨架已产出 |
| 5a | 跑门禁 G-CODEPLAN-SRC+G-14+G-08 | 全过才进审核点2.5 |
| 5b-📖 | AI主动讲解CodingPlan故事 | 5维度讲清才进审核点2.5 |
| 5b | 审核点2.5（CodingPlan评审）| 14条门禁全过+用户✅；context-pressure |
| 5c | ④bis CodingProcess（CodeAnalysis→CodingPlan）| G-CODEPLAN-SRC+G-14通过 |
| 5b.5 | （v3.10.0 已合并入步骤5b）| - |
| 6 | Execute（coding-skill）| 代码按 CodingPlan 落地，编译预检无阻断 |
| 7 | Test Generate（test-generate-skill）| TEST_REPORT 已生成，证据链齐 |
| 7a | Test Review（test-review-skill）| test-verifier 独立复核通过；G-09/G-10 通过 |
| 8 | 出具Coding报告 | 文件已生成 |
| 8a | ⑥bis 全切面一致性核查 | 无🔴漂移 |
| 8b | 出具CodeReview报告 | 含"零、"章节；无阻断型问题 |
| 8c | ⑦bis 全链路对称性核查 | 无🔴断链 |
| 9 | 完成判定（6.1~6.10）| 全部✅（⑥.10由test-verifier独立验证）|
| 9-📖 | AI主动讲解Code故事 | 7维度+文件:行号+代码片段才进审核点4 |
| 10 | 审核点4（CodeReview完成确认）| 用户✅；context-pressure |
| 10a | ⑦ter 流程收尾合规自检 | 5维度全✅或自愈完毕才进⑧ |
| 11 | ⑧ 完成输出 | state.phase=completed |
