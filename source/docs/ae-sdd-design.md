# ae-sdd 系统能力说明书

> v3.7.4 · 面向开发者、LLM Agent 与项目接入方
>
> 本文档是**系统能力设计入口**，说明 ae-sdd 的能力语义、边界和当前实现状态。代码分层、模块职责、运行时数据流和变更闭环统一维护在 [`ae-sdd-implementation-architecture.md`](ae-sdd-implementation-architecture.md)。若本文与代码实现冲突，以 CLI/测试输出为准，并同步修正文档。

---

## 1. 端到端流程编排（Phase 1/2/3）

### 设计

将整个研发流程切分为三个强制有序的 Phase，每个 Phase 有明确入口门禁、执行节点、人工审核点和出口产物。AI 负责驱动节点推进，不能跳过任何节点；每个人工审核节点 AI 必须主动"讲故事"（在对话内直接呈现），不能只丢文档链接。

- Phase 1 设计阶段：需求分析(RA) → DR 生成 → Story 生成(①) → 前端视角审视(①bis) → Story Review(②) → 测试用例生成(③) → 业务逻辑汇总(③bis) → 人工审核点 1
- Phase 2 实现阶段：实现方案预确认(人工审核 1.5) → Task 生成 + 全局 Task Review(④) → CodingPlan 生成(④bis) → CodingPlan 评审(人工审核 2.5) → Coding(⑤)
- Phase 3 验证阶段：完成判定⑥（10 项条件）→ 全切面一致性核查(⑥bis) → CodeReview 报告(⑦) → 全链路对称性核查(⑦bis) → 人工审核点 4
- 共 5 个人工审核节点，内容必须输出在对话中，不能要求用户自行打开文件查看

**v3.5.2 流程收尾合规自检**：在两处流程结束点增加"自检合规 → 不合规就修复"环节，防止 AI 裸 ✅ 收尾。Story 级 ⑦ter（人工审核点 4 后）5 维度自检；PRD 级 §1.7（`prd-complete` 前）强制先跑 `prd-check-complete`。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Phase/节点定义 | `source/skills/orchestration/ae-sdd-update-skill.md` 及各 phase SKILL 文件描述节点顺序 |
| 节点进度持久化 | `state.json.phase` 字段，见 §3 流程状态持久化 |
| Story 级 ⑦ter 自检 | AI 编排执行：`gates check` 全量 + `gates check --only G-DOC-STORAGE` + `state read` + 产出物核对，SKILL 文字描述，非独立 CLI |
| PRD 级 §1.7 强制顺序 | `ae-sdd state prd-check-complete --prd <ID>`（`tools/bin/ae-sdd` state 子命令）必须先于 `ae-sdd state prd-complete --prd <ID>` |
| 4 层 AND 校验 | `tools/lib/state.py:check_prd_4_layers()`（G-PRD-1~4） |
| 审核点用户确认 token | `ae-sdd state confirm --phase <审核点>`（防 AI 自填，`tools/bin/ae-sdd` state confirm 子命令） |

**颗粒度与边界**：流程节点粒度；Phase 1 完成确认、CodingPlan 审核等关键节点锁定后不允许重跑；不可跳过任何节点，微任务走快速通道也须经路由判定后才豁免；自检不替代人工审核，不得跳过自检直接收尾。🆕 2026-07-03(B1)：明确"快速通道"仅指 `/ae-sdd-quick` escape hatch（豁免 G-00/路由判定维度，详见 conventions §3.1），**不等于 scale=微 的子链豁免**；微链（scale=微）同样必须走 code-reviewed 节点出 CodeReview 报告（conventions §3.1 质量底线：Plan-first/CodeReview 报告/state.json 更新 ❌不豁免）。

---

## 2. 智能路由（4 类需求 + 4 维判定）

### 设计

统一入口层，对所有用户输入做需求分类后路由到对应 SKILL，两套路由机制并存互补：4 维判定（来源 × 规模 × 现有产物 × 项目类型）优先，分类不明时 fallback 到 4 类需求传统路径（套 Story 7 区模板判定规模）。

路由决策算法 7 步：工作区检查(0) → 自更新识别(1.5，短路到 update-skill) → 来源识别(1.6) → 规模识别(1.7) → G-RA 准入门禁(1.8) → 关键词匹配(2) → 加载执行(3-5)。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 分类算法核心 | `tools/lib/classify.py:classify()`（行235），综合标题信号/文件名信号/关键词匹配/规模推断 |
| 规模推断（4 维判定） | `classify.py:_infer_scale_from_lines()`（行131）/ `_infer_scale_from_project_context()`（行143） |
| CLI 入口 | `ae-sdd classify --text "..."`（`tools/bin/ae-sdd` classify 子命令，行2960） |
| 合法规模枚举 | `tools/lib/state.py:VALID_SCALES = ("大", "中", "小", "微")`（行88） |
| 路由到子链 | `tools/lib/state.py:PHASE_FLOWS` 字典按 scale 选链（见 §3 表格） |

**颗粒度与边界**：路由到节点级（requirement-analysis / dr-generate / story-generate / task-generate / coding），不进行节点内子步骤路由；路由决策权归 ae-sdd 流程与用户，不归 AI 临时直觉。

---

## 3. 流程状态持久化（state.json）

### 设计

每个新需求必须先创建一个独立状态机，再进入 phase 流转。WorkItem（PRD / BUG / OPT / Story 均可）的执行进度持久化到独立 `state.json`，支持跨 session 中断恢复与多任务并行执行。AI 重入时读对应 WorkItem state 跳过已完成步骤，避免重复执行。

**WorkItem 标识约定（2026-07-09 修正）**：每个状态机必须归属于真实顶层工作项。CLI 入口为 `ae-sdd state new --id <ID> --entry-node <PRD|DR|STORY|TASK>`；物理目录名采用 R6 顶层名（如 `PRD-001` / `DR-005` / `Story-006` / `Task-BUG-LIFE-001`），并写入 `workItemKey`、`stateMachineId`、`currentWorkItem`。`--work-item <ID|WORKITEM-KEY>` 只定位 `.auto-engineering/{WORKITEM-KEY}/state.json`。项目级 `.ae-sdd/state.json` 不允许作为 active state、mirror 或 fallback；未能唯一定位 work-item 时必须拒绝并要求显式选择。

**v3.5.15 多子链状态机**：单条 PHASE_FLOW 拆为 4 条 PHASE_FLOWS（大/中/小/微），按 scale 路由，微链最短单步合法，修复微任务 next-step 误建议跑 RA 的问题。旧 state 无 scale 字段时按 completedSteps 反推，默认"大"（最保守）。

**v3.6 暂停态**：`paused` 作为一级 phase，任何 phase 可跳入，用于 Level 3 人工升级（见 §15 流程偏移检测与矫正）。

**🆕 v3.9.0/v3.9.13 嵌套状态模型（Nested State Model）**：取代 v3.8.x 前的项目级/扁平状态模型。一个顶层 work-item 的 state.json 内维护主流程所有子系列——`prdState` + `drStates{N个DR}` + `storyStates{Story索引}`；每个 `drStates[DR-ID]` 下再维护自己的 `storyStates{N个Story各自完整流程状态}`。按 `entryNode` 选填容器：

| entryNode | state 内含容器 | 适用场景 |
|---|---|---|
| PRD | prdState + drStates + storyStates | 全新 PRD，从 PRD 出发走完整流程，允许多个 DR 子系列 |
| DR | drState + storyStates | 已有 DR，从 DR 出发（DR 所属 PRD 的 state 不存在时） |
| STORY | storyStates | 已有 Story 草稿，从 Story 出发（上层 DR/PRD state 不存在时） |
| TASK/PLAN | （flat state，不嵌套） | Bug/微任务不改 Story（R4 保留微链） |

**R2 任意节点出发 + 递归向上归入**：PRD / DR / Story / Task / BUG 都可作为顶层任务入口，但如果当前节点有合法父级，必须继续向上遍历直到真实顶层。例：Story-007 → DR-005 → PRD-001 时，最终只创建/使用 `.auto-engineering/PRD-001/state.json`；DR 写入 `drStates["DR-005"]`，Story 写入 `drStates["DR-005"].storyStates["STORY-007"]`，并同步到顶层 `storyStates` 索引。只有 DR 没有合法 PRD 父级时才创建 DR 根 state；只有 Story 没有合法 DR/PRD 父级时才创建 Story 根 state。

**R5 改已管理 Story 重定位 + 重置子状态**：检测到改 Story 且该 Story 已属某 state 的 `storyStates` → `ae-sdd state relocate --story <ID>` 重定位到该 state + 只重置该 Story 子状态到 `story-generated`（兄弟 Story 不动，resetHistory 保留审计）。

**R6 顶层主体命名**：只以最顶层主体特征命名——`PRD-{特征}` / `DR-{特征}` / `Story-{特征}`（多 Story 合并如 `Story-003-004-005`）。由 `paths.build_state_machine_name()` 生成。

**R7 路由自动匹配/新建**：`/ae-sdd` 路由时 `classify.match_state()` 自动分析需求特征（提取 PRD/DR/Story ID + 判定 Bug/改 Story）→ 扫描现有嵌套 state → 命中则 relocate/absorb，未命中则 create_nested。匹配优先级：R4 微任务 → R5 Story 命中 → R2 DR 归入 PRD → R2 Story 归入 DR → R7 新建。

**v1 扁平 state 完全兼容**：旧 workitem（`stateModel` 缺省或 `"flat"`）保留可读，所有读取点通过 `state.is_nested_state()` 分流。旧 workitem 不迁移、不动。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 存储路径 | `.auto-engineering/{WORKITEM-KEY}/state.json`。`.ae-sdd/state.json` 禁止作为状态源、active mirror 或 fallback；hook/gate/CLI 均必须通过 work-item/session resolver 定位任务级 state |
| 4 条子链定义 | `tools/lib/state.py:PHASE_FLOWS`（行69-92），大链 14 phase / 中链 13 / 小链 12 / 微链 8（🆕 2026-07-03 B1：微链加回 code-reviewed，与"CodeReview 报告不豁免"对齐；含 TestCase 系列后已扩容，🆕 v3.7.x） |
| 向后兼容别名 | `PHASE_FLOW = PHASE_FLOWS["大"]`（行95，🟡 deprecated） |
| 合法 scale 枚举 | `VALID_SCALES = ("大", "中", "小", "微")`（行88） |
| phase 允许工具集 | `tools/lib/gate_intercept.py:PHASE_PERMIT`（行64） |
| 暂停/恢复 API | `state.py:pause_state()`（行663）/ `resume_state()`（行684） |
| 矫正计数 API | `state.py:increment_correction()`（行706）/ `get_correction_count()`（行725） |
| 多 Agent 状态字段 API | `state.py` 行472注释起：`activeAgents` 写入 + `agentReports` 归档（见 §4） |
| memory 生命周期强制校验 | `tools/lib/memory_gate.py:check_state_transition()`（行51），`state write --phase` 切相前调用 |
| 终态投影不变量 | `state.py:set_phase()` / `set_story_substate_phase()` 写 phase 时同步 `currentPhase/currentStep/completedSteps/pendingOutputs/codingRound`；`write_state()` 拒绝 `phase=completed` 但投影仍处于中间态的 state |
| PRD 4 层 AND 校验 | `state.py:check_prd_4_layers()` |
| CLI 入口 | `ae-sdd state new / read / write / next-step / confirm / prd-init / prd-check-complete / prd-complete / prd-archive`（`tools/bin/ae-sdd` state 子命令组） |
| 🆕 v3.9.0 嵌套 state schema | `state.py:init_nested_state()` / `reset_story_substate()` / `set_story_substate_phase()` / `get_active_phase()` / `get_active_story()` / `ENTRY_NODE_CONTAINERS` |
| 🆕 v3.9.0 嵌套 state 命名 | `paths.build_state_machine_name(top_node, features)`（R6 顶层命名） |
| 🆕 v3.9.0 向上归入查找 | `paths.find_nested_state_by_story_id/dr_id/prd_id`（R2 归入） |
| 🆕 v3.9.0 路由自动匹配 | `classify.match_state()` + `extract_requirement_features()`（R7） |
| 🆕 v3.9.0 entryNode 容器选择器 | `flow_enums.FlowNode.container_fields()` + `is_nested_entry()` |
| 🆕 v3.9.0 R5 relocate CLI | `ae-sdd state relocate --story <ID>`（重定位+重置子状态） |
| 🆕 v3.9.0 嵌套 state write | `ae-sdd state write --sub-story <ID> --phase <phase>` / `--add-story <ID>` |

**颗粒度与边界**：step 级（如 `step-4-coding-r2`）；不可倒退的关键门禁步骤标记为 locked；🆕 v3.9.0 嵌套模型下多个 Story 的子状态在同一个 state 的 `storyStates{}` 内各自独立流转、互不干扰（R5 重置只动目标 Story）；state 仅记录进度，不存储业务产物内容。`phase/history` 是生命周期状态，`currentPhase/currentStep/completedSteps/pendingOutputs/codingRound` 是可恢复执行用的工作流投影；终态写入必须同步两套字段，不能出现 `phase=completed` 但 `currentStep` 仍等待人工确认、`pendingOutputs` 未清空或 `codingRound` 仍为 r0 的组合。

---

## 4. 多 Agent 编排（角色库 + 派活协议）

### 设计

当单节点内存在无强依赖的子任务时，root agent 按角色库拆分派活；Review 节点支持多 reviewer 交叉审，避免自圆其说。8 个预定义角色：`story-writer / story-reviewer / testcase-writer / task-writer / coder / code-reviewer / test-verifier（强制）`。

派活协议要求结构化 YAML 任务分配卡（必填 `agent_role / story_id / input / output / standards / context / deadline / report_back`）；故障补救 4 级 SOP（重试→重新分配→降级→升级用户）；多 reviewer 框架按 Tier 判定单/双/三审。

**⚠️ 设计-实现落差**：这一整套角色库、派活协议卡格式、故障补救 SOP、多 reviewer 框架，目前**只存在于 SKILL 文本描述**（`source/skills/cross-cutting/agent-orchestration-skill.md`），是对 AI 行为的软约束，代码层没有强制执行或校验派活卡格式是否合规。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 角色库/派活协议/故障 SOP/多 reviewer 框架 | `source/skills/cross-cutting/agent-orchestration-skill.md`（纯文字描述，AI 自律执行，**无代码校验**） |
| 多 Agent 状态可见性（唯一落地的代码支撑） | `tools/lib/state.py`：`activeAgents` 字段（启动 sub-agent 时写入，行477起）/ `agentReports` 字段（完成后移入，行494起） |
| test-verifier 独立性约束 | ⑥.10 测试真实性硬门禁 G-09 要求报告带独立 `session_id`（`tools/lib/gates.py` G-09 check 函数），是唯一有 CLI 侧校验的角色约束 |

**颗粒度与边界**：节点内子任务级并行；降级为逻辑多视角时必须在报告头部标注 `reviewerMode: "logical-multi-perspective"`；root agent 保留最终冲突裁决权；除 activeAgents/agentReports 状态记录和 test-verifier session_id 校验外，其余编排规则完全依赖 AI 自律，无物理拦截。

---

## 5. 门禁体系（G-XX 三种强度）

### 设计

三种强度形成纵深防御：软约束依赖 LLM 自律，硬门禁通过 CLI 阻断，扫描器兜底覆盖硬门禁无法检测的场景。

- **软约束**：SKILL 文本说明，LLM 自律执行
- **硬门禁**：`GATE_REGISTRY` 注册，CLI `ae-sdd gates check` 阻断，不通过无法继续
- **检查器**：`*_scan.py` 静态扫描报告，兜底补硬门禁覆盖不到的场景

G-00 项目资产门卫每次 SKILL 启动前验证资产存在；G-RA 系列在需求分析阶段把关；G-CODE-1 在 Coding 完成/CodeReview 前扫描反模式；中段门禁（G-14/G-CODEPLAN-SRC/G-DOC-STORAGE）补"两头强中间空"；入口关卡三道闸（entry token / 产物落地凭证 / 代码改动准入）管住流程入口。

🆕 v3.9.1 上下文加载准入门禁（G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX）补齐 DR/Story/TestCase/Task 四组的"第零步准入检查"——此前这四组只有 prose 清单（dr-review/task-generate 还官方自认 report-only），AI 可不读 PRD/DR/项目资产/约束就过门禁切相。采用**注册表模式**：一个 `_check_context_loaded` 函数 + `CONTEXT_GATE_REGISTRY` 注册表服务 4 个门禁，流程一致用单函数封装，上下文差异（DR 查 RA+PRD，Story 查 DR+PRD，TestCase 查 Story，Task 查 Story+TestCase）走注册表 `required` 字段；读文件统一走 `document-storage-skill` 的 `get_constraints/get_assets` API。微链 G-TASK-CTX 用 `required_micro` 豁免 Story/TestCase。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 门禁注册表 | `tools/lib/gates.py:GATE_REGISTRY`（list，**实际 34 个**：G-00~G-14 + G-09B + G-CODEPLAN-SRC + G-DOC-STORAGE + G-DOC-CONSISTENCY + G-PATH + G-RA-1~6 + G-RA-FLOW-VIOLATION + G-CODE-1 + G-REVIEW-LOOP + G-AUTO-CONSENSUS + 🆕 v3.9.1 G-DR-CTX/G-STORY-CTX/G-TESTCASE-CTX/G-TASK-CTX） |
| CLI 统一扫描入口 | `ae-sdd gates check`（`tools/bin/ae-sdd` 行2688，帮助文本自带门禁清单） |
| 单个门禁定向检查 | `ae-sdd gates check --only <gate_id>` |
| G-00 资产门卫 | `gates.py` G-00 check 函数；不通过时可运行 `ae-sdd assets generate --project <key>` 生成/修复 baseline 资产，再用 `ae-sdd assets check` 校验 |
| G-RA-1~4 需求分析门卫 | `gates.py`，调 `scripts/ra_authenticity_scan.py`（G-RA-4，`_locate_ra_authenticity_scanner` 行1116） |
| G-RA-5 机械派生深度 | `gates.py` 行1292起，调 `scripts/ra_depth_scan.py` |
| G-RA-6 实现视角完整性 | `gates.py` 行1378起，调 `scripts/ra_implementation_scan.py`（I1~I7） |
| G-RA-FLOW-VIOLATION | `gates.py` 行1208起，调 `scripts/flow_violation_scan.py`（R1~R3规则） |
| G-CODE-1 Coding 真实性 | `gates.py` 行697起，调 `scripts/coding_authenticity_scan.py`（AP-1~AP-6反模式） |
| G-09 测试真实性 | `gates.py` 行578起，调 `scripts/test_authenticity_scan.py`（8类禁止手段） |
| G-CODEPLAN-SRC / G-DOC-STORAGE / G-DOC-CONSISTENCY / G-PATH | `gates.py` 对应 check 函数，中段门禁补齐 |
| 🆕 v3.9.1 G-DR-CTX / G-STORY-CTX / G-TESTCASE-CTX / G-TASK-CTX | `gates.py` 注册表 `CONTEXT_GATE_REGISTRY` + 统一 `_check_context_loaded` 实现 + 4 个薄封装；`gate_intercept.py:PHASE_ENTRY_GATES` 大/中/小/微链各 phase 入口挂载；复用 `get_constraints/get_assets` + `_iter_ra_files/_find_prd_files/paths.find_doc` |
| G-REVIEW-LOOP | `gates.py` + `tools/lib/review_loop.py`，`ae-sdd review-loop` 子命令 |
| 入口关卡1（entry token） | `ae-sdd enter <projectKey> [--story <ID>]`（`tools/bin/ae-sdd`），UserPromptSubmit hook 检测 `/ae-sdd` 触发词强提醒 |
| 入口关卡2（产物落地凭证） | `tools/lib/gate_intercept.py` PreToolUse 拦截，产物路径 + entry token + 产物-Phase 映射校验 |
| 入口关卡3（代码改动准入） | `gate_intercept.py:PHASE_PERMIT`，非 coding phase 或无审核点 2.5 确认 → 禁写 src/ |
| 设计-实现对齐反查 | `tools/lib/alignment_audit.py`（UC-08~UC-13，6个 check_* 函数），CLI `ae-sdd update-check`；见 §9 真实性扫描后段 |

**颗粒度与边界**：G-00 每次 SKILL 调用前必跑；G-RA 豁免场景（微任务、BUG/配置类、重入已完成步骤）；改门禁强度必须改 `gates.py`，不能只改 SKILL.md 文字；`ae-sdd update-check` 自动检测文档-实现一致性，防文档撒谎复发。

---

## 6. 项目资产体系（7 层索引）

### 设计

每个接入项目维护一份标准化资产文件，是所有 SKILL 的上下文基础，通过 ES 倒排索引支持按需读取。AI 读资产而非扫描全仓库，保证上下文质量与读取效率。

资产生成分两层：`ae-sdd init` 首次运行会自动生成可通过 G-00 的 baseline 资产；`ae-sdd assets generate/check` 提供可执行的生成/修复/校验入口。深度业务资产仍由 `project-assets-update-skill` 引导 AI 跑探查 SOP（读 CLAUDE.md/AGENTS.md → 扫描工程结构 → 抽典型类 → 识别分层/命名/约束）后增量完善；`assets update/audit` 仍未实现。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 资产文件路径 | `source/assets/{projectKey}/{projectKey}.assets.md`（7 层索引 §A-§G） |
| 生成/增量更新/审计引导 | `tools/lib/project_assets.py` + `ae-sdd assets generate/check` 负责 baseline 生成/校验；`source/skills/cross-cutting/project-assets-update-skill.md` §3/§4/§5 负责深度业务资产更新/审计引导 |
| ES 倒排索引 + BM25 评分 | `tools/lib/assets_index.py`（`AssetsIndex` 类：outline/query/section/stats 核心逻辑） |
| CLI 入口 | `ae-sdd assets generate / check / read / outline / section / query / stats`（`tools/bin/ae-sdd` assets 子命令组） |
| G-00 门卫自动检查 | `tools/lib/gates.py` G-00 check 函数，7 层索引缺任一层即阻断；距 lastAuditedAt > 30 天触发警告 |

**颗粒度与边界**：项目级隔离（按 projectKey）；7 层索引 §A-§G 必须齐备；RA SKILL 强制通过 `ae-sdd assets read` 接口读取，禁止直接读文件路径。

---

## 7. 文档存取层（document-storage）

### 设计

统一文档落地/读取/归档的横切能力，五维定位模型 + WorkItem 隔离键（projectKey × workItemId × intent × storyId? × 版本号 × ChangeLog）取代散点式手写路径拼接。所有 SKILL 读写 ae-sdd 生成的文档（DR/Story/TestCase/Task/CodingPlan 等）必须通过本层 API，禁止裸拼路径或裸调 `ae-sdd assets read`。

核心原则：intent 驱动定位（同一 API 按 intent 参数分流到不同文档类型的路径规则），Task/Coding/Test/CR 以 workItemId 作为分桶键，版本号与 ChangeLog 策略集中管理，避免每个 SKILL 各自实现一套路径逻辑导致漂移。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 动态定位 API 契约 | `source/skills/cross-cutting/document-storage-skill.md` §4（14个API 契约，§4.10 intent 枚举表 34个intent） |
| 路径解析 | `tools/lib/document_storage.py:resolve_path()`（行149） |
| 文档保存（带版本号+ChangeLog） | `document_storage.py:save_doc()`（行366） |
| 文档定稿 | `document_storage.py:finalize_doc()`（行444） |
| 项目约束读取 | `document_storage.py:get_constraints()`（行119） |
| 项目资产列表读取 | `document_storage.py:get_assets()`（行140） |
| Git 路径/服务根路径 | `document_storage.py:get_git_path()`（行99）/ `get_service_root()`（行108） |
| 版本号推断 | `document_storage.py:get_latest_version()`（行259）/ `_normalize_version()`（行239） |
| ChangeLog 读取 | `document_storage.py:get_changelog()`（行286） |
| RA 前置条件校验 | `document_storage.py:check_ra_prerequisites()`（行320） |
| 存量文档迁移 | `document_storage.py:migrate_old_docs()`（行615） |
| CLI 入口 | `ae-sdd doc save / resolve / finalize`（`tools/bin/ae-sdd` 行2722起 doc 子命令组） |

**已补齐**：`get_thinking_engine(projectKey)` 已在 `tools/lib/document_storage.py` 实现，并收录到 `document-storage-skill.md` §4.2。编码流程引用该 API 时会优先读取项目/文档工作区覆盖版本，找不到时回退到 ae-sdd 自带 `standards/thinking/be-coding-thinking-engine.md`，返回 `path/source/content/sha256`。

**颗粒度与边界**：所有 ae-sdd 生成文档的读写必须走本层 API，不允许 SKILL 各自维护路径拼接逻辑；version/ChangeLog 策略变更须同步 §4.10 intent 枚举表的"实现状态"列（✅已实现/📝待实现）。

---

## 8. 实例化体系（4 层架构）

### 设计

母版到项目落地的 4 层分发体系，保证 SSOT 的同时支持项目级 override。Layer 1 母版是唯一编辑点，经脚本构建为分发包，再由安装脚本装入用户环境；接入项目通过 Layer 4 实例做 override，不修改母版。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| Layer 1 母版（SSOT） | `source/`，开发者唯一编辑点，git 跟踪 |
| Layer 2 分发包构建 | `scripts/build_dist.py` → `dist/ae-sdd/`（注入 VERSION + plugin.json，剥离 CHANGELOG/docs），git ignored |
| Layer 3 用户安装 | `scripts/install.py` → `~/.claude/skills/ae-sdd/`，Claude Code 实际加载 |
| Layer 4 项目实例创建 | `ae-sdd init <dir> <key>`（`tools/bin/ae-sdd` init 子命令），生成 `config.yaml/state.json/assets//overrides/` |
| Override 解析 | 项目 `overrides/` 优先于母版 defaults，由各 SKILL 读取时的路径解析规则实现 |
| 版本号同步 | `ae-sdd bump <ver>`（`tools/bin/ae-sdd` 行3056），同步 SKILL.md / paths.py / README.md 三处 |
| 一步到位开发流 | `scripts/dev-sync.sh` → `scripts/dev_sync.py`（build + install），跑前必须 `ae-sdd update-check` 全绿 |

**颗粒度与边界**：Layer 2/3/4 不手工改；fork（完整复制）是显式 opt-in；override 优先级：项目 overrides/ > 母版 defaults。

---

## 9. Harness 适配层

### 设计

将 ae-sdd SKILL.md 自动转译为 Mavis harness 格式的 agent.md，使 ae-sdd 能作为 Mavis 团队级 agent 被编排。不需要手工维护两套定义，转译脚本从 SKILL.md + HARNESS.md 生成 agent.md，母版升级后重跑即可同步。

**⚠️ 文档滞后修正**：原文档写转译脚本是 `convert-ae-sdd-to-harness.ps1`——该文件已不存在。实际实现是 `scripts/build_harness.py`（Python 重写版，脚本头部注释明确写"由 convert-ae-sdd-to-harness.ps1 迁移而来"），逐功能对齐原 PS1 版本（版本号 fallback / frontmatter 解析 / 多维幂等锁 / 模板渲染 / mount 失败回滚等）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 转译脚本 | `scripts/build_harness.py`（非 `.ps1`，PS1→Python 迁移已完成） |
| 产物路径 | `.harness/agent.md` + `.harness/README.md` |
| 幂等锁 | `.harness/.adapter.lock`（JSON：`adapter_version/ae_sdd_version/source_input_sha256/source_commit/templateHash/converted_at`），`build_harness.py:read_adapter_lock()` |
| 版本号三级 fallback | `build_harness.py:get_ae_sdd_version()`（行88，SKILL.md frontmatter → commit msg vX.Y.Z → git short hash） |
| tree-hash amend 检测 | `build_harness.py:get_tree_hash()`（行147，🆕 v3.5.6，区分 amend 和真实内容变更） |
| SKILL frontmatter 解析 | `build_harness.py:parse_skill_frontmatter()`（行168） |
| 模板渲染 | `build_harness.py:render_template()`（行207，`{{VAR}}` 占位符替换） |
| mavis CLI 探测与 mount | `build_harness.py:find_mavis_cmd()`（行233）/ `run_mavis()`（行252），mount 失败自动回滚产物（行488起） |
| 备份轮转 | `build_harness.py:cleanup_old_bak()`（行68，保留最近3个 `.bak.<ts>`） |
| CLI 用法 | `python scripts/build_harness.py [--dry-run/--force/--unmount/--clean/--no-mount]` |

**颗粒度与边界**：禁止手工编辑 agent.md；母版（source/SKILL.md / source/HARNESS.md / harness 模板）升级后必须重跑 `build_harness.py` 重新生成；harness 格式由 Mavis 规范决定，转译脚本负责映射；`.adapter.lock` 多维比对（source_input_sha256 + ae_sdd_version + adapter_version + templateHash）任一漂移触发重转，`source_commit` 只作诊断，不参与幂等判断，避免提交生成物后继续漂移。

---

## 10. 记忆层（5 层分级 + 阶段强制门禁）

### 设计

phase-aware 的分区持久化记忆，在关联节点（RA/design/coding-plan/coding/review）强制执行 enter→write→exit 生命周期，提供跨 session 上下文连续性。Agent-facing 主分区只有两个：`task`（任务级，默认写入，映射 L1）与 `project`（项目级，跨任务复用，映射 L2）。记忆是 compact context index，不是日志/报告；写入质量门禁防止 LLM 把猜测或长段过程写成记忆——task/project 每条记忆必须有证据（文件路径:行号/报告路径/用户确认/工具结果），无证据只能留 scratch。

底层仍保留 5 层存储：scratch/L0 会话草稿（session 后可删）/ task/L1 Story 级记忆 / project/L2 项目级记忆 / pattern/L3 跨项目 pattern / archive/L4 冷归档（postmortem/ADR）。冲突处理：新证据与记忆冲突时写 `kind=conflict`，禁止静默覆盖。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 生命周期 API | `tools/lib/memory_store.py:enter()`（行164）/ `write()`（行193）/ `exit_phase()`（行286） |
| exit 前置校验 | `memory_store.py:check_exit_ready()`（行236），exit 无 write 则失败 |
| phase→memory 映射 | `tools/lib/memory_gate.py:memory_phase_for_state_phase()`（行29） |
| state 切相前自动校验 | `memory_gate.py:check_state_transition()`（行51），`ae-sdd state write --phase` 调用，未完成 enter→write 则阻断 |
| 校验结果格式化 | `memory_gate.py:format_transition_block()`（行107） |
| 存储格式 | JSONL，`memory_store.py:_jsonl_path()`（行109）/ `_append_jsonl()`（行131） |
| scope→layer 映射 | `memory_store.py:MEMORY_SCOPE_TO_LAYER`：scratch→L0、task→L1、project→L2、pattern→L3、archive→L4；`--layer` 仅作兼容参数 |
| compact 写入校验 | `memory_store.py:_validate_compact_memory()`：task<=180 字符、project<=140、pattern<=120；task/project 强制 evidence；project/pattern 禁 `kind=observation`；拒绝多行/代码块/长证据 |
| CLI 入口 | `ae-sdd memory write/read/search --scope task|project`；`memory promote --from-scope task --to-scope project`；兼容 `--layer L1/L2` |

**颗粒度与边界**：仅 5 个关联节点强制触发；kind 字段：decision/constraint/finding/issue/risk/fix/conflict/observation；任务之间不互读 task memory，跨任务复用必须进入 project memory；archive 只读为主，禁止整体注入上下文。UserPromptSubmit 仅注入 active scope 下的 task/project compact memory，且 task 优先、project 只补充剩余预算。

---

## 11. Plan-First 编排（CodingPlan 16 章节 + 14 门禁）

### 设计

编码前必须先生成并经用户确认 CodingPlan，将架构决策、实现顺序、类骨架、验证点全部锁定。CodingPlan 是 Coding 阶段的唯一依据，AI 不得自行调整 Plan 内容；14 条门禁全部通过才允许进入 Coding。

CodingModel 11 维决策（嵌入每个 Task 文档）：并发控制/幂等策略/事务边界/缓存策略/错误码/异常处理/状态机实现/外部依赖/数据模型/可观测性/复用能力。实现方案决策基线（最高优先级 4 步强制）：① 现有能力复用扫描 → ② 业内成熟方案参考 → ③ 五维代码质量评估 → ④ 核心能力归属唯一。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| CodingPlan 生成时机 | Phase 2 ④bis，由 task-writer 汇总，见 `source/skills/phase2-coding/coding-process-skill.md` |
| 14 条门禁关键词校验 | `tools/lib/gates.py:CODINGPLAN_14GATES_KEYWORDS`（行459-474） |
| 关键词缺失检测 | `gates.py` 行492：`missing_kw = [k for k in CODINGPLAN_14GATES_KEYWORDS if k not in content]` |
| G-08 门禁 | `gates.py`，CodingPlan 文档存在且 14 门禁全过才放行 coding-process phase |
| 5 上下文加载（🆕 v3.7.3） | `coding-process-skill.md`：项目约束/技术约束/Story/Task/TestCase，统一走 `document-storage-skill` API 读取（见 §7） |
| 人工审核点 2.5 | AI 主动 walkthrough（SKILL 文字流程，非 CLI），逐条等用户确认 |

**颗粒度与边界**：CodingPlan 在编码前必须生成并经用户确认，不可跳过；编码过程中偏离 Plan 需用户实时确认；CodingPlan 变更触发 storyVersion 累加。

---

## 12. 真实性扫描（静态扫描器 + 设计-实现对齐验证器）

### 设计

防止 LLM 伪造测试通过、伪造需求分析内容、伪造设计-实现对齐的静态扫描器，作为不可绕过的硬门禁运行时依赖。输出统一 JSON 契约，BLOCKER=0 才算通过。

**⚠️ 文档滞后修正**：原文档标题写"3 个静态扫描器"，实际共 6 个扫描器（测试/Coding/RA真实性 3个 + RA流程违规/RA机械派生深度/RA实现视角完整性 3个），外加 2 个对齐验证工具（AA全维对齐验证器 + IC迭代检查器）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 测试真实性扫描（⑥.10/G-09硬门禁） | `scripts/test_authenticity_scan.py`：8类禁止手段 + Surefire XML 解析 + AC覆盖率100%验证；由 test-verifier sub-agent 独立执行 |
| Coding 真实性扫描（G-CODE-1） | `scripts/coding_authenticity_scan.py`：AP-1~AP-6反模式库；`ae-sdd gate coding-required` 自动触发 |
| RA 真实性扫描（G-RA-4） | `scripts/ra_authenticity_scan.py`：8类禁止规则（vague-ellipsis/no-evidence/fabricated-field等） |
| RA 流程违规审计（G-RA-FLOW-VIOLATION） | `scripts/flow_violation_scan.py`：R1~R3规则（12维决策记录/8维度挖掘/缺口管理） |
| RA 机械派生深度（G-RA-5） | `scripts/ra_depth_scan.py`：验证每条规则 R 机械追问 6 问 → 衍生 R′ |
| RA 实现视角完整性（G-RA-6） | `scripts/ra_implementation_scan.py`：I1~I7 检查 |
| 外挂内容安全扫描（插件加载防护） | `scripts/plugin_content_scan.py`：PC-001~PC-010（危险删除/任意命令执行/远程脚本执行等），由 `tools/lib/plugin_loader.py:_scan_plugin_content()`（行725起）调用 |
| 设计-实现对齐验证器（AA） | `tools/lib/alignment_audit.py`：UC-08~UC-13（6个 check_uc0x 函数），反向对账"doc 承诺门禁↔gates 注册↔实现真实性"，CLI `ae-sdd update-check` |
| 设计-实现一致性迭代检查（IC） | `tools/lib/iteration_check.py`：IC-1~IC-4 机器粗筛（report-only 不阻断），CLI `ae-sdd iteration-check` |

**颗粒度与边界**：测试真实性扫描是 ⑥.10 硬门禁；扫描器均不可被 SKILL 文字描述替代；AA（UC-08~13）阻断式，IC（IC-1~4）report-only 不阻断；扫描器路径变更须同步更新 update-graph.json。

---

## 13. 工具链 CLI

### 设计

ae-sdd Python CLI，将 SKILL 规则工具化，实现"规则描述 + 工具执行"双轨 SSOT。规则在 SKILL.md 描述，执行在 CLI 实现，两者通过 `ae-sdd update-check` 自动验证一致性；CLI 是门禁、状态、资产、记忆等能力的统一执行入口。

**⚠️ 文档滞后修正**：原文档标题写"14 大类子命令"，实际顶层子命令组已达 30 个（新增 `doc / enter / context-pressure / ra-gate / flow-violation-scan / ra-depth-scan / ra-implementation-scan / review-loop / plugin / iteration-check / perf` 等）；原表格中列出的 `route / sync-tools / run / quick / proposal` 顶层命令**不存在**，已从下表删除。

### 实现

| 入口 | `tools/bin/ae-sdd`（Python 3），lib 模块见各章节 |
| --- | --- |
| 输出协议 | JSON 走 stdout，日志走 stderr，pipeline 友好 |

| 类别 | 实际子命令（按 `tools/bin/ae-sdd` 现状） |
| --- | --- |
| 资产类 | `assets generate / check / read / outline / section / query / stats` |
| 文档存取类 | `doc save / resolve / finalize` |
| 状态机类 | `state read / write / next-step / confirm / prd-init / prd-check-complete / prd-complete / prd-archive` |
| 入口凭证类 | `enter <projectKey> [--story <ID>]` |
| 路由类 | `classify` |
| 门禁类 | `gates check / gate ra-required / gate coding-required / gate doc-storage / gate-intercept / ra-gate` |
| 记忆类 | `memory enter / write / exit / read / search / promote / summarize` |
| 数据库类（只读） | `db profiles / query / explain / audit` |
| Git 类（只读） | `git status / diff / log / blame / impact` |
| 更新图谱 | `update-check [--only UC-XX] / update-check --affected <文件>` |
| 迭代检查 | `iteration-check` |
| 上下文压力 | `context-pressure` |
| RA 扫描类 | `ra-depth-scan / ra-implementation-scan / flow-violation-scan` |
| Review 类 | `review-loop` |
| 性能诊断类 | `perf report / perf doctor / perf clear` |
| 版本类 | `bump <ver> / version` |
| 维护类 | `health / init / init-hooks / runtime / plugin / scripts-dir / prompt-inject / stop-check` |

- `ae-sdd health` 9 项自检：子 SKILL 章节完整性 / 项目资产双源一致 / 规则-工具同步 / 门禁覆盖度 / TR-1~7 / 扫描器就绪 / CHANGELOG 版本一致
- `ae-sdd update-check`：权威源 `source/standards/update-graph.json`（UC-01~UC-16，比早期 UC-01~06 更完整），版本号三处一致 / 门禁注册一致 / 命令契约闭环 / 扫描器分发 / 健康度清单覆盖 / 文档-实现一致性 / runtime 编译一致性 / 自动化级联一致性等；dev-sync 前必须全绿
- `ae-sdd iteration-check`：IC-1~4 机器粗筛（report-only），接管人工 SOP 步骤 2/3/4
- `ae-sdd perf report/doctor/clear`：运行时统计查询、慢点建议和本地统计清理；统计只记录命令/span 耗时、退出码、脱敏 argv 与有限属性，不记录业务文档正文

**颗粒度与边界**：db/git 工具为只读；G-00 由 AI Agent 手动 `gates check --only G-00`（非 CLI 自动触发）；update-check 权威源是 `source/standards/update-graph.json`；改 CLI 命令契约须同步 update-graph.json；iteration-check/context-pressure/perf 均 report-only，不阻断 dev-sync。

---

## 14. state.json events 操作日志设计

### 设计

v3.4.0 引入 PRD 级 state.json + phase 状态机，但子任务级操作的可追溯性弱：`phase` 字段只记"当前阶段"没有轨迹，`history` 字段粒度过粗，门禁拦截只在触发时记录结果没有"何时由谁触发"的 audit trail。

**方案**：state.json 增加 append-only `events` 数组，每条记录一次"流程动作"（路由/门禁/阶段切换/用户确认等）。**不替代**现有字段——events 与 `phase`/`history`/`currentStory`/`currentTask` 共存：phase = 当前阶段（高频读），history = 阶段切换摘要（低频读），events = 细粒度操作（运维审计/复盘）。

关键设计决策：① 继承 `str, Enum` 使 JSON 序列化直接得字符串，日志人工可读；② append-only 不去重，保证 audit trail 真实性；③ `txnName` 区分 PRD/Story/Task/Plan 事件；④ 不强制落盘，事件写入与持久化解耦，调用方按需 write_state；⑤ 向后兼容，旧 v1 state.json 无 events 字段时不报错。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 3 个枚举 | `tools/lib/flow_enums.py`：`FlowNode`(6成员) / `FlowSkill`(15成员) / `FlowEventType`(8成员：routed-to/skill-completed/gate-blocked/gate-cleared/user-confirmed/phase-changed/reopened/aborted) |
| 事件数据类 | `FlowEvent`，必填 `seq/ts/event/node/by`，条件必填 `skill/gate_id/phase`，可选 `txnName/from_node/reason/output/meta` |
| 5 个工厂函数 | `make_routed_to` / `make_skill_completed` / `make_gate_blocked` / `make_phase_changed` / `make_user_confirmed` |
| 追加事件 API | `tools/lib/state.py:append_event(state, event)`：原地追加，自动填 seq/ts，不自动落盘 |
| 查询事件 API | `tools/lib/state.py:get_events(state, *, txn_name=None, event_type=None, node=None)`：三维过滤 |

**⚠️ 已知缺口（业务调用方尚未全部接入）**：

| 缺失点 | 预期接入方 | 状态 |
| --- | --- | --- |
| 路由时记录 `routed-to` | `cmd_classify` / router | 待接入 |
| 阶段切换时记录 `phase-changed` | `cmd_state_write` | 待接入 |
| 门禁检查时记录 `gate-blocked`/`gate-cleared` | `gates.py` 各 check_gXX() | 待接入 |
| SKILL 完成时记录 `skill-completed` | 各 SKILL orchestrator | 待接入 |
| 用户确认审核点时记录 `user-confirmed` | ae-sdd-update-skill §审核点双支柱 | 待接入 |

lib 层 schema 已就绪且测试通过；一旦业务调用方接入，events 立即生效，无需 schema 升级。

**颗粒度与边界**：events 只追加不修改历史记录；txnName 为 None 表示 PRD 级事件；事件写入与磁盘持久化解耦，避免高频事件导致 IO 风暴。

---

## 15. Hook 层（三层拦截体系）

### 设计

三个 Claude Code hook 实现物理级流程纪律，覆盖工具调用拦截、上下文注入、响应后校验三个时机，不依赖 LLM 自律。

- **PreToolUse**：AI 每次调用工具前触发，按 phase 限定允许工具集
- **UserPromptSubmit**：用户每次提交消息前触发，注入状态上下文 + 主流程监管器逻辑（见 §16）
- **Stop**：AI 每次回复结束后触发，防无限循环 + 检测结构性错误

**决策 1B（v3.6）**：废弃 `◆ STATE:`/`◆ LOADED:` 自报标记检测（"防君子不防小人，可谎报"），改为纯产物核查（gates check 结果为唯一权威判定）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| PreToolUse hook | `tools/lib/gate_intercept.py`：`PHASE_PERMIT` 字典按 phase 限定工具集；只读工具任何 phase 放行；`paused` phase 禁全部写操作；链式 Bash（含 `&&`/`\|\|`/`;`/`\|`）不认为只读；输出 `permissionDecision: "deny"` + `systemMessage`，exit 始终 0 |
| UserPromptSubmit hook | `tools/lib/prompt_inject.py`：读 state 注入阶段上下文 + `/ae-sdd` 触发词强提醒（关卡1）+ 快速通道标记 + 重置 Stop hook 重试计数 + 母版版本漂移探测（行198起）+ 主流程监管器调用（`_run_flow_monitor()`，行242，见§16）+ active scope 下 compact memory 注入 |
| Stop hook | `tools/lib/stop_check.py`：`.stop_retry_count` 持久化计数 MAX_RETRY=2；已废弃 `◆ STATE:`/`◆ LOADED:` 自报标记检测（行5/152注释自述"已由 flow_monitor 产物核查替代"）；补 `_check_compact_failure()` 检测卡住态 |
| Hook 安装 | `ae-sdd init-hooks` 写三段配置到 `.claude/settings.json` |
| 快速通道 | 用户说 `/ae-sdd-quick` → `.quick_channel` 写入 → PreToolUse 跳过 phase 工具拦截（产物落地关卡仍生效） |

**颗粒度与边界**：hook 层只做物理拦截（工具权限/上下文注入/结构校验），不做业务逻辑判断；hook 任何异常降级不阻断主流程（全流程 try/except，exit 始终 0）；改 hook 须同步 HARNESS.md HS 规则；禁止手工编辑已安装的 hook 脚本，改完须重跑 `ae-sdd init-hooks`。

---

## 16. 主流程监管器（v3.6，已实现）

> 文档修正说明：本节原标"🆕 v3.6 规划中"，经代码核实（`tools/lib/flow_monitor.py` 全文 326 行 + `prompt_inject.py:_run_flow_monitor()` 行242 + `state.py` 暂停/恢复/矫正计数 API 均已存在并接入 UserPromptSubmit hook），确认**已完整实现并生效**，非规划态。

### 设计

`/ae-sdd` 触发后全生命周期的编排角色，负责工作区初始化、资产检查、智能路由、各系列循环执行、收尾交付，不执行任何具体业务工作——是 ae-sdd 的"项目经理"，只管"下一步该做什么、由谁做、做完了没有"。角色定义在 SKILL.md，执行实体是 UserPromptSubmit hook 的 Python 逻辑（决策 2B）。

5 步标准启动序列：Step 1 工作区确认与初始化 → Step 2 项目资产检查(G-00) → Step 3 智能路由（双层裁定）→ Step 4 按 scale 子链执行编码流程 → Step 5 收尾交付。每系列执行协议含 sub-step 0~5（compact清理 → SKILL调用声明 → 生成 → Review → Loop控制 → 人工审核）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 偏移检测核心逻辑 | `tools/lib/flow_monitor.py:detect_drift(state, ade_sdd)`（行144），Layer 1 产物核查 + Layer 3 矫正次数升级 |
| phase→gate 映射表 | `flow_monitor.py:get_phase_gate_map()`（行70），覆盖 ra-generated~test-running 各 phase |
| 升级判定 | `flow_monitor.py:should_escalate(state)`（行236），矫正次数 ≥ `CORRECTION_THRESHOLD_PAUSE`(3) |
| 矫正消息生成 | `flow_monitor.py:build_correction_message(drift)`（行272），severity 1/2/3 三级文本模板 |
| gates check 子进程调用 | `flow_monitor.py:_run_gates_check(gate_id, ade_sdd)`（行115），10s 超时降级放行 |
| Hook 侧调用入口 | `tools/lib/prompt_inject.py:_run_flow_monitor(ade_sdd, state)`（行242），每轮 UserPromptSubmit 执行，异常降级返回 None |
| 状态暂停/恢复 | `tools/lib/state.py:pause_state()`（行663）/ `resume_state()`（行684） |
| 矫正次数持久化 | `state.py:increment_correction()`（行706）/ `get_correction_count()`（行725），写入 `state.correctionCounts` |
| SKILL 侧角色定义 | `source/skills/orchestration/ae-sdd-update-skill.md`（5步启动序列文字描述） |

**颗粒度与边界**：监管器角色定义在 SKILL.md，执行实体在 UserPromptSubmit hook（Python，决策 2B）；监管器不执行业务工作，只做编排+校验；ae-sdd 自更新触发时监管器休眠；退出条件：`phase=completed` 或用户手动中止；每系列入口必做 compact（防上下文污染）。

---

## 17. 流程偏移检测与矫正（v3.6，已实现）

> 文档修正说明：本节原标"🆕 v3.6 规划中"，实际与 §16 共用同一套 `flow_monitor.py` 实现，已完整生效。

### 设计

识别 AI 在执行过程中的语义漂移行为，按 3 级矫正机制自动纠正，超出阈值升级人工干预并暂停流程。流程偏移分两类：物理越权（已由 PreToolUse hook 拦截）和语义漂移（本模块处理）。

漂移类型：B1 跳步（跳过 Review 宣布进入下一系列）/ B2 停滞（同一 phase 经 N 轮产物未过 gates check）/ B3 伪完成（声称完成但门禁未过，原依赖自报检测，v3.6 改产物核查）/ B4 旁路（题外话后未回到流程）。

矫正级别：Level 1 静默注入（用户不可见，AI 可感知）/ Level 2 矫正提示词（AI 须说明修复计划，同步骤最多3次）/ Level 3 人工升级（`state.phase=paused`，流程暂停待用户决策）。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 漂移结果数据类 | `flow_monitor.py:DriftResult`（行45起）：`drift_type/severity/gate_id/gate_passed/gate_message/phase/correction_count/message` |
| 阈值常量 | `flow_monitor.py`：`CORRECTION_THRESHOLD_WARN=5`（行36）/ `CORRECTION_THRESHOLD_PAUSE=3`（行38）/ `GATES_CHECK_TIMEOUT=10`（行40） |
| Layer 1 产物核查（解 B1/B3） | `detect_drift()`（行144-225）依据 `get_phase_gate_map()` 跑 gates check，不通过判定漂移，不信任 AI 自报 |
| Layer 3 矫正次数升级（解 B2） | `detect_drift()` 行204：`correction_count >= CORRECTION_THRESHOLD_PAUSE` → severity=3 |
| severity=1 矫正文本 | `build_correction_message()` 行290-296：静默提醒 |
| severity=2 矫正文本 | `build_correction_message()` 行298-309：含矫正次数/失败门禁详情 |
| severity=3 暂停文本 | `build_correction_message()` 行311-323：含继续/跳过/回退/查看四个用户决策选项 |
| phase→gate 映射（9个phase） | `get_phase_gate_map()`（行70-89）：ra-generated→G-RA-1~4，dr-generated→G-01，story-generated→G-02，story-reviewed→G-03，task-generated/task-reviewed→G-04，coding-process→G-08，coding→G-CODEPLAN-SRC，code-reviewed→G-05，test-running→G-09 |
| Stop hook 职责精简 | `tools/lib/stop_check.py`：已删除 `◆ STATE:`/`◆ LOADED:` 自报检测，职责简化为防空响应+防无限循环 |
| 全流程降级保护 | `detect_drift()` 行227-233：任何异常返回 `drift_type="none"` 不阻断主流程 |

**颗粒度与边界**：只检测语义漂移（物理越权由 PreToolUse 拦截）；gates check 结果为唯一权威判定，不信任 AI 自报（决策1B）；`correctionCounts` 写入 state.json 跨 session 持久化；`flow_monitor.py` 是纯计算模块（只返回结论不直接写 state），写 state 由 `prompt_inject.py` 调用 state API 执行；Level 3 暂停后续接检测自动识别 `phase=paused` 并播报。

---

## 18. SKILL 编译与 Runtime IR（v3.8 设计）

### 设计

ae-sdd 分为两种物理版本：`source/` 是未编译母版，面向维护者；`dist/ae-sdd/` 是编译后的实例化运行包，面向各 Agent。正式发版只能分发编译后版本，不能直接把 `source/` 安装到 Agent skills 目录。

编译目标不是把 SKILL 变成不可读"机械码"，而是生成短、结构化、可审查的 runtime compact slices。Agent 主入口 `dist/ae-sdd/SKILL.md` 变为 bootloader，只声明加载顺序、冲突优先级和 fallback 规则；子 SKILL 在 `dist/ae-sdd/skills/**/*.md` 中也必须变为 compiled bootloader，完整 Markdown 原文只保存在 runtime fallback 中，只有 compact 不足时才延迟读取。

源 SKILL 入口在进入 runtime 编译前允许标准化瘦身，但瘦身不是自由删减。`scripts/slim_source_skills.py` 必须先把完整原文锚定到 `source/skill-fallbacks/**`，再按 `source/standards/skill-source-slimming-standard.md` 和 `source/templates/skill/source-skill-slim-entry-template.md` 渲染 slim entry。每个 slim entry 必须包含语义识别清单，覆盖身份/触发、流程/路由、门禁/约束、工具/API、状态/数据、产物/文档、资源引用、设计对齐和 fallback-only 细节；已瘦身文件默认跳过，schema 升级只能从 fallback 重渲染，禁止从 slim entry 二次瘦身。

源瘦身的语义边界：slim entry 是索引和加载契约，`source/skill-fallbacks/**` 是完整语义锚点。runtime 编译器遇到 `source_slimmed: true` 时，fallback 和 outline 抽取必须来自 `source_fallback`，不能来自 slim entry。

编译后运行包的核心结构：

```text
dist/ae-sdd/
├── SKILL.md                         # compiled bootloader
└── runtime/
    ├── manifest.json                # 版本、source hash、runtime_fingerprint、load_order
    ├── boot.compact.md              # 加载契约
    ├── route.compact.md             # 路由压缩表
    ├── subskills.compact.md         # 子 SKILL 编译入口索引
    ├── gates.compact.md             # 门禁压缩表
    ├── flow.compact.md              # 状态机压缩表
    ├── macros.compact.md            # 公共动作宏
    ├── skills/**                    # 子 SKILL 局部 manifest/boot/outline/fallback
    └── fallback/SKILL.full.md       # 原始主入口 fallback
├── skills/**/*.md                   # 子 SKILL compiled bootloader，非原文
```

冲突优先级：用户最新明确指令（不得绕过硬门禁） > CLI/gate/state 真实输出 > runtime compact > 子 SKILL compiled bootloader/outline > 子 SKILL fallback > 主入口 fallback > 历史说明文档。

完整说明见 [`source/docs/skill-runtime-compiler.md`](skill-runtime-compiler.md)。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 未编译母版 | `source/`，唯一人工编辑点 |
| 源 SKILL 瘦身 | `scripts/slim_source_skills.py`，按 v2 标准生成 slim entry；完整原文进入 `source/skill-fallbacks/**` |
| 源瘦身标准/模板 | `source/standards/skill-source-slimming-standard.md` + `source/templates/skill/source-skill-slim-entry-template.md` |
| 编译实例包 | `dist/ae-sdd/`，由 `scripts/build_dist.py` 生成，git ignored |
| Runtime 编译器 | `scripts/compile_skill_runtime.py`，生成 `runtime/*.compact.md`、`runtime/skills/**`，并替换 dist 主入口和子 SKILL 入口为 bootloader |
| 通用编译器 SKILL | `standalone-skills/skill-runtime-compiler/`，可复制到其它 agent/仓库，用于把任意 `SKILL.md` 包编译成同级 `<name>-compiled/` |
| 编译接入点 | `scripts/build_dist.py` 在复制 source/tools/scripts 后调用 runtime 编译器 |
| 门禁 compact 数据源 | `tools/lib/gates.py:GATE_REGISTRY` |
| 状态机 compact 数据源 | `tools/lib/state.py:PHASE_FLOWS` |
| 机器校验 manifest | `runtime/manifest.json`：`compiled=true`、`deterministic=true`、`runtime_fingerprint`、`load_order`、`source_checksums` |
| 分发入口 | `scripts/distribute.py` 只接受 `dist/ae-sdd/` 或基于它生成的 Agent 专属产物 |

**颗粒度与边界**：第一期编译 boot/route/subskills/gates/flow/macros 六类高收益规则，并把所有子 SKILL 入口编译为薄 bootloader；子 SKILL 的完整长流程、模板正文、设计背景仍按需 fallback。源瘦身只压入口，不压语义；`source_fallback_sha256` 和语义 inventory hash 是防丢语义的机器锚点。compact runtime 不替代 CLI gate，任何硬门禁判断以 `ae-sdd gates check`、`state` 等工具输出为准。runtime 编译产物必须字节级幂等，不写入墙钟时间；同一份 `source/`、同一版编译器、同一份 `GATE_REGISTRY` 与 `PHASE_FLOWS` 重复编译，`dist/ae-sdd/SKILL.md`、`dist/ae-sdd/runtime/**` 和 `dist/ae-sdd/skills/**/*.md` 必须完全一致。

**当前实现状态**：源 SKILL 标准化瘦身 v2、瘦身标准/模板、第一期编译器、构建接入、runtime manifest、六类 compact slice、全量子 SKILL compiled bootloader、fallback 原文锚点、字节级幂等测试、`ae-sdd runtime verify`、`update-check` UC-15 runtime 编译一致性检查、分发器 compiled-only 校验均已落地。另有通用 `skill-runtime-compiler` standalone SKILL，可把任意 SKILL 包编译到同级 `<name>-compiled/`，源包不变，并由 UC-15 覆盖幂等性。后续扩展重点是外层 `dist` 包可复现构建、子 SKILL 语义 compact 增强、Agent 专属二次编译约束细化。详细清单见 [`source/docs/skill-runtime-compiler.md`](skill-runtime-compiler.md) §14~§15。

---

## 19. 自动化模式（v3.8.0，已实现）

### 设计

默认关闭的"全自动化开关"。开启后 6 个人工审核点（1/1.5/2/2.5/4/5）改走 Tier 3 多 reviewer 联审共识，跳过所有人工✅，实现 ae-sdd 输入→结果。联审机制复用 `agent-orchestration-skill §8.4`（Tier 判定 + 视角正交 + 交叉对比 + 降级规则），本模块只管开关与审核点行为分叉。

核心立场：
- **默认关闭**：`automation.enabled=false`，回退现状（每审核点等用户✅）
- **开启即强制 Tier 3**：覆盖规模/关键决策判定，自动化模式跳过人工✅必须最高强度联审兜底
- **禁逻辑多视角降级**：自动化模式必须物理 3 独立 session reviewer，环境不支持 → `state.phase=paused`（不得用 logical-multi-perspective 凑数）
- **开工前信息预收集**：开工前一次性向用户收集所有必需信息（第三方凭证/复用选择/环境配置/命名约定/对接方/数据初始化），开工后不再打断
- **阻断出口**：联审 3 轮矫正未决 → `state.phase=paused`（默认），输出完整问题清单等用户介入，避免 AI 带病狂奔

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 配置 SSOT | `.ae-sdd/config.yaml` 的 `automation` 段（`scripts/init.py:CONFIG_TEMPLATE` 生成，默认 `enabled: false`）|
| 配置加载 | `tools/lib/config.py`：`AUTOMATION_DEFAULTS` + `load_automation_config()` + `is_automation_enabled()`/`get_reviewer_tier()`/`get_automated_points()` |
| CLI 开关 | `tools/bin/ae-sdd`：`automation status/enable/disable`（enable 写 `enabledAt` 审计时间戳，AI 不得自行改）|
| 开工前预收集 | `ae-sdd preflight collect`：扫输入材料+资产 7 层索引，识别 6 类待补信息，写 `.ae-sdd/preflight-info.yaml` |
| 联审共识 state | `tools/lib/state.py`：`reviewConsensus[point]` 字段 + `register_review_consensus()`/`get_review_consensus()` |
| 联审共识写入 | `ae-sdd state register-review-consensus --point {N} --passed {true\|false}` |
| 联审共识门禁 | `tools/lib/gates.py:G-AUTO-CONSENSUS`（blocker，34 门禁之一）：自动化模式下 review 节点切相前校验 `reviewConsensus[point].passed=true` + reviewer 独立性（复用 G-09B 模式）|
| Tier 强制 | `agent-orchestration-skill.md §8.4.1`：`automation.enabled=true` → 强制 Tier 3，覆盖规模判定 |
| 降级禁止 | `agent-orchestration-skill.md §8.4.5`：自动化模式禁逻辑多视角降级，必须物理 3 独立 session |
| 流程编排 | `source/SKILL.md §🚀 自动化模式` + Step1 自动化检测 + Step1.5 预收集 + 监管器步骤4 联审共识双模式 |
| 级联检查 | `tools/lib/update_graph.py:check_uc16_automation_cascade`（UC-16）：校验 config.py/gates/state/CLI/init/SKILL 六处齐备 |
| 级联图谱 | `source/standards/update-graph.json:UG-20`：trigger 含 config.py/init.py/gates.py/state.py/ae-sdd |

**颗粒度与边界**：自动化模式不新增 phase，复用现有 PHASE_FLOWS 状态机骨架，只在审核点行为分叉（默认 vs 联审共识）；reviewConsensus 仅作用于 review 节点，不挂 PRD 4 层 AND 闸；6 审核点讲解规范（§📖）不变，自动化模式仍讲解，听众从"用户"变成"3 个 reviewer"；G-09B/G-REVIEW-LOOP 现有逻辑复用，不重复实现。

---

## 20. ae-sdd Monitor（只读可视化投影）

### 设计

ae-sdd Monitor 是 ae-sdd 的本地桌面可视化投影层，用于在一个父目录下发现多个 ae-sdd 工作区，并集中查看每个工作区的当前 phase、派生状态、最近活动、工作项、Memory 状态和 Runtime Stats。它服务的是“监管运行状态”和“快速定位异常/暂停/完成态”，不改变 ae-sdd 主流程。

核心立场：

- **只读投影**：Monitor 不写 `.ae-sdd/`、不写 `.auto-engineering/`、不执行 gate，不把 UI 状态反写为 ae-sdd 状态。
- **主设计优先**：phase、scale、entry node、门禁语义和 Runtime Stats 含义以本文档其它章节与实现架构文档为准，Monitor 只能跟随展示。
- **多工作区入口**：用户选择父目录，Monitor 扫描其中所有包含 `.ae-sdd/` 的工作区；左侧列表用于选择，右侧详情用于观察。
- **两级导航/看板**：左侧必须分项目与任务两级且项目可折叠，点击项目切项目级看板，点击任务切任务级看板，点击折叠控件只展开/收起任务；右侧顶部必须同时体现当前项目与当前任务。
- **局部响应式切换**：同一项目下点击任务不得清空或重建整个详情页；Monitor 只能更新当前任务、指标、Tab 内容和侧边栏选中态。
- **React renderer**：Monitor 的桌面壳仍由 Electron 提供，本地 UI 内胆必须由 React + TypeScript 组件实现；侧边栏项目/任务树必须通过稳定 key 保持节点连续性，禁止回退到整块 DOM 替换造成闪烁。
- **结束态也可观察**：`completed`、`paused`、`idle`、`invalid` 等状态都必须可展示，不能只服务正在运行的工作区。
- **多活跃任务可见**：Monitor 必须同时展示根 state、activeAgents 和 `.auto-engineering/{WORKITEM-KEY}` 中的活跃/未完成工作项，不能压缩成单个 activeWorkItem。
- **Memory 状态可见**：Monitor 必须展示 `.ae-sdd/memory` 的项目级/任务级 memory 数量、最近记录、活跃 scope 与阻断 scope，不能只看 phase/state。
- **阶段轴可见**：时间线必须展示完整 phase 链和当前节点说明，即使 state history/events 为空也能判断工作区处于哪个节点。
- **响应式观察**：默认通过文件系统事件监听 `.ae-sdd/` 与 `.auto-engineering/` 变化并静默刷新当前项目/任务；低频轮询只作为兜底；自动刷新仍然只读，不执行 gate 或 memory 命令。
- **动效连续性**：项目折叠、Tab 切换和按钮按压可以使用轻量 iOS 风格动效，但加载、任务切换和响应式刷新不得持续闪烁；动效只能表达 UI 反馈，不得暗示 ae-sdd 状态被写入或 gate 被执行；必须支持系统 reduced-motion 降级。
- **体验连续性**：目录选择必须有即时反馈；重启后必须恢复上次父目录、选中工作区和选中任务；类 Mac 窗口三点必须是真实窗口控制。

### 实现

| 设计点 | 实现方式 |
| --- | --- |
| 独立设计文档 | [`source/docs/ae-sdd-monitor-design.md`](ae-sdd-monitor-design.md) |
| 应用位置 | `apps/ae-sdd-monitor/` |
| 扫描入口 | `apps/ae-sdd-monitor/src/workspace.js:scanForWorkspaces()` |
| 状态读取 | 只读取 `.auto-engineering/{WORKITEM-KEY}/state.json`；展示 `workItemKey/stateMachineId/currentWorkItem`，字段缺失时从 R6 顶层目录名回退推导 |
| Memory 读取 | `.ae-sdd/memory/**/*.jsonl`、`.ae-sdd/memory/.stage/*.json`；展示项目/任务 memory、活跃 scope 与阻断 scope |
| 性能读取 | `.ae-sdd/runtime-stats/*.jsonl` |
| 展示结构 | 左侧可折叠项目/任务两级列表 + 右侧当前项目/当前任务两级看板 + 总览/阶段轴与事件流/Memory/活跃任务与工作项/性能/原始状态 |
| UI 动效 | `src/styles.css` 与 `renderer/src/App.tsx` 只实现本地交互反馈、折叠过渡、Tab/detail 过渡和 reduced-motion 降级；响应式刷新不得让看板闪烁，不改变 ae-sdd 项目文件 |
| 偏好保存 | Electron userData `preferences.json`，保存父目录、选中工作区、选中任务、项目折叠状态、自动刷新开关和主题 |
| 发包目标 | Windows setup exe/zip + macOS dmg/zip |
| 同步闭环 | `source/standards/update-graph.json:UG-22` |

**颗粒度与边界**：Monitor 是伴随工具，不是 ae-sdd runtime 的一部分；它可以把文件状态可视化，但不能成为 gate、state 或 Runtime Stats 的权威源。任何涉及状态 schema、阶段链、Runtime Stats JSONL 字段或项目侧路径的变化，必须同步 Monitor 独立设计文档、解析代码、测试和 README。
