# ae-sdd 系统能力说明书

> v3.2.5 · 面向开发者、LLM Agent 与项目接入方

## 端到端流程编排（Phase 1/2/3）

> 将整个研发流程切分为三个强制有序的 Phase，每个 Phase 有明确入口门禁、执行节点、人工审核点和出口产物。

**是什么**：ae-sdd 把研发生命周期分为设计、实现、验证三个 Phase，AI 负责驱动节点推进，不能跳过任何节点。每个人工审核节点 AI 必须主动"讲故事"（在对话内直接呈现），不能只丢文档链接。

**设计实现**：

- Phase 1 设计阶段：需求分析(RA) → DR 生成 → Story 生成(①) → 前端视角审视(①bis) → Story Review(②) → 测试用例生成(③) → 业务逻辑汇总(③bis) → 人工审核点 1
- Phase 2 实现阶段：实现方案预确认(人工审核 1.5) → Task 生成 + 全局 Task Review(④) → CodingPlan 生成(④bis) → CodingPlan 评审(人工审核 2.5) → Coding(⑤)
- Phase 3 验证阶段：完成判定⑥（10 项条件）→ 全切面一致性核查(⑥bis) → CodeReview 报告(⑦) → 全链路对称性核查(⑦bis) → 人工审核点 4
- 共 5 个人工审核节点，每个节点内容必须输出在对话中，不能要求用户自行打开文件查看
- state.json 记录 step 级进度，不可倒退的关键门禁步骤标记为 locked

**颗粒度与边界**：流程节点粒度；Phase 1 完成确认、CodingPlan 审核等关键节点锁定后不允许重跑；不可跳过任何节点，微任务走 §0.5 快速通道也须经路由判定后才豁免。

**🆕 v3.5.2 流程收尾合规自检**：在两处流程结束点增加"自检合规 → 不合规就修复"环节，防止 AI 裸 ✅ 收尾：

- **Story 级 ⑦ter**（人工审核点 4 后、⑧完成输出前）：5 维度自检 —— ① `gates check` 全量门禁通过 ② `gates check --only G-DOC-STORAGE` 无游离产物 ③ `state read` phase/currentStory/events 完整 ④ §⑧ 产出物表 8 类文件真实存在 ⑤ 本轮 CodeReview 无 Open 态 🔴。自愈策略：🟢 可自愈项（游离文档移位 / state 补推进 / 缺失产物补生成）AI 直接修后重跑；🔴 阻断项（逻辑性门禁失败 / 未整改 🔴）阻断升级用户。
- **PRD 级 §1.7**（`prd-complete` 前）：强制 `prd-check-complete` 先于 `prd-complete`，堵住 `cmd_state_prd_complete` 跳过 4 层 AND 校验直接 compact 的漏洞。自愈映射：G-PRD-1 缺→回 Story 级 ⑦ter；G-PRD-3 缺→补 mitigationPlan；G-PRD-4 缺→触发审核点 5。
- **落地强度**：SKILL 编排层文字 + 复用现有 CLI（不新建硬门禁），符合"规则描述 + 工具执行"双轨设计；7t-1/2/3 复用已有硬阻断命令，7t-4/5 由 AI 按编排文字执行清单齐全性核对。

**颗粒度与边界（v3.5.2）**：自检维度级；自检不替代人工审核（在审核点之后做最终核对，确保用户确认内容与实际落地一致）；不得跳过自检直接收尾，跳过即视为事后回溯。

---

## 智能路由（4 类需求 + 4 维判定）

> 统一入口层，对所有用户输入做需求分类后路由到对应 SKILL，两套路由机制并存互补。

**是什么**：所有用户输入经过 7 步路由决策算法分类，优先走 4 维增强路径，分类不明时 fallback 到 4 类需求传统路径，最终路由到对应节点。

**设计实现**：

- 路由决策算法 7 步：工作区检查(0) → 自更新识别(1.5，短路到 update-skill) → 来源识别(1.6) → 规模识别(1.7) → G-RA 准入门禁(1.8) → 关键词匹配(2) → 加载执行(3-5)
- 4 类需求（传统路径，fallback）：套 Story 7 区模板——填满 4+ 区=中大任务(类型2)，2-3 区=小任务(类型3)，0-1 区=微任务(类型4)
- 4 维判定（增强路径，优先）：来源(PRD/Issue/对话/BUG) × 规模(大/中/小/微) × 现有产物 × 项目类型
- 两套并存，4 维判定优先；来源/规模不明时 fallback 到 4 类需求
- 路由表 20+ 关键词触发词，微任务和 BUG/配置类豁免 G-RA 前置
- CLI：`ae-sdd classify --text "..."` 输出判定结果

**颗粒度与边界**：路由到节点级（requirement-analysis / dr-generate / story-generate / task-generate / coding），不进行节点内子步骤路由；路由决策权归 ae-sdd 流程与用户，不归 AI 临时直觉。

---

## 流程状态持久化（state.json）

> 每个 Story 的执行进度持久化到 state.json，支持跨 session 中断恢复与多 Story 并行执行。

**是什么**：每个 Story 对应一个独立 state.json，记录当前阶段、已完成步骤、Coding 轮次等，AI 重入时读 state 跳过已完成步骤，避免重复执行。

**设计实现**：

- 存储路径：`.auto-engineering/{STORY-ID}/state.json`，多个 Story 各自独立互不干扰
- 核心字段：`storyId / currentPhase / currentStep / completedSteps / storyVersion / codingRound / pendingOutputs / lastUpdated`
- 多 Agent 扩展字段：`activeAgents[] / agentReports[]` 追踪并行 agent 状态
- storyVersion：Story 文档内容变更时累加；codingRound：每开启新一轮 Coding 前累加
- 重入机制：读 state.json → 解析 currentStep → 路由到对应 SKILL → 跳过 completedSteps 中已完成的步骤
- v3.2.3 起：`ae-sdd state write --phase <next>` 切相前自动校验 memory 生命周期（enter→write 是否完成），未完成则阻断切相
- 🆕 v3.4.0：PHASE_FLOW 新增 `ra-generated`（initialized → ra-generated → dr-generated），RA 阶段 memory 强制（修复 B3-6）；审核点 token 机制（`ae-sdd state confirm --phase <审核点>` 写 session.json 的 userConfirmedPhases，防 AI 自填，关卡3 校验）
- CLI：`ae-sdd state read / write / next-step / confirm / validate / show / diff / lock`

**颗粒度与边界**：step 级（如 `step-4-coding-r2`）；不可倒退的关键门禁步骤标记为 locked；多个 Story 的 state 文件互不干扰；state 仅记录进度，不存储业务产物内容。

---

## 多 Agent 编排（角色库 + 派活协议）

> 在同一流程节点内把子任务拆给多个 sub-agent 并行执行，root agent 负责调度、汇总、冲突决策。

**是什么**：当单节点内存在无强依赖的子任务时，root agent 按角色库拆分派活；Review 节点支持多 reviewer 交叉审，避免自圆其说。

**设计实现**：

- 拆分原则：只拆同一节点内无强依赖的子任务，硬上限 5 个并行 sub-agent，禁止多 agent 并发写同一文件
- 8 个预定义角色：`story-writer / story-reviewer / testcase-writer / task-writer / coder / code-reviewer / test-verifier（强制）`
- 派活协议：结构化 YAML 任务分配卡，必填字段：`agent_role / story_id / input / output / standards / context / deadline / report_back`；缺字段视为派活不完整
- sub-agent 回传：结构化 Markdown 报告，必含：结果 / 完成情况 / 关键决策 / 风险点 / 待 root 决策
- 故障补救 4 级 SOP：重试(最多3次) → 重新分配 → 降级(合并/拆更细) → 升级用户
- 多 reviewer 框架：Tier 判定(1/2/3 对应单审/双审/三审) → 视角正交切分 → 交叉对比(缺陷ID × reviewer × 评级三维表) → 冲突决策树
- `test-verifier` sub-agent 是 ⑥.10 硬门禁，主 agent 不得自我验证，必须独立运行

**颗粒度与边界**：节点内子任务级并行；降级为逻辑多视角时必须在报告头部标注 `reviewerMode: "logical-multi-perspective"`；root agent 保留最终冲突裁决权。

---

## 门禁体系（G-XX 三种强度）

> 贯穿流程的强制检查点，阻止不满足条件的节点推进，三种强度覆盖从文字约束到 CLI 硬阻断再到静态扫描的完整防线。

**是什么**：三种强度形成纵深防御：软约束依赖 LLM 自律，硬门禁通过 CLI 阻断，扫描器兜底覆盖硬门禁无法检测的场景。

**设计实现**：

- **软约束**：SKILL 文本说明，LLM 自律执行
- **硬门禁**：`tools/lib/gates.py` GATE_REGISTRY 注册，CLI `ae-sdd gates check` 阻断，不通过无法继续
- **检查器**：`*_scan.py` 静态扫描报告，兜底补硬门禁覆盖不到的场景
- G-00 项目资产门卫：每次 SKILL 启动前验证资产存在 + 7 层索引齐备 + 距 lastAuditedAt ≤ 30 天；缺失由 AI Agent 路由到 `project-assets-update-skill §3` 生成（🆕 v3.4.0：原"CLI 自动调用 assets check/generate"为文档撒谎，已改为 agent 手动 `gates check --only G-00`）
- G-RA 需求分析准入门卫（v3.2）：进 dr-generate/story-generate/task-generate 前，验证 RA 文档存在 + 8 维度齐全 + 12 维 RAModel 完整 + RA-G01~G16 全过 + 5 问自检阻断项=0。🆕 v3.4.0：RA 在 `ra-generated` phase 进行（PHASE_FLOW 新增，修复 B3-6 memory 覆盖）；G-RA 由 agent 手动调用（不在 PHASE_ENTRY_GATES）
- G-CODE-1 Coding 真实性门卫（v3.2.1）：Coding 完成/CodeReview 前扫描 AI Coding 反模式 AP-1~AP-6
- 🆕 v3.4.0 中段门禁（建议书1/2/4，补齐"两头强中间空"）：G-14 CodingPlan-Story 一致性（AC 对齐 + 偏离 Proposal）/ G-CODEPLAN-SRC 源码核对（类骨架附已读/待核实标记）/ G-DOC-STORAGE 文档落地存放合规（禁游离位置）
- 🆕 v3.4.0 入口关卡三道闸（建议书4）：关卡1 entry token（`ae-sdd enter`，UserPromptSubmit 检测 `/ae-sdd` 注入强提醒）/ 关卡2 产物落地凭证校验（PreToolUse 拦产物路径 + entry token + 产物-Phase 映射）/ 关卡3 代码改动准入（非 coding phase 或无审核点 2.5 确认 → 禁写 src/）
- 共 22 个门禁（G-00~G-14 + G-CODEPLAN-SRC + G-DOC-STORAGE + G-RA-1~4 + G-CODE-1），`ae-sdd gates check` 统一扫描
- ⑥.10 测试真实性：8 类禁止手段 + Surefire XML 解析 + AC 覆盖率 100%。🆕 v3.4.0：test-verifier 报告须带独立 session_id（≠ 主 agent），G-09 校验
- ⑥bis 全切面一致性：以代码为锚反向核查 DR/Story/Task/测试用例/代码五方一致
- ⑦bis 全链路对称性：DR-Story-Task-实现-测试用例五层双向追溯，无断链
- 🆕 v3.4.0 F-1 假门禁修复：Stop hook 交叉验证 `◆ GATE: ✅ CLEAR` 与实际文档一致（AI 谎报 G-08 通过但 CodingPlan 缺关键词 → block）

**颗粒度与边界**：G-00 每次 SKILL 调用前必跑；G-RA 豁免场景（微任务、BUG/配置类、重入已完成步骤）；改门禁强度必须改 `gates.py`，不能只改 SKILL.md 文字；🆕 v3.4.0 UC-06 自动检测文档-实现一致性（SKILL/子SKILL 命令 + HARNESS HS 规则），防文档撒谎复发。

---

## 项目资产体系（7 层索引）

> 每个接入项目维护一份标准化资产文件，是所有 SKILL 的上下文基础，通过 ES 倒排索引支持按需读取。

**是什么**：资产文件记录微服务清单、分层规则、命名约定、技术约束等结构化信息，AI 读资产而非扫描全仓库，保证上下文质量与读取效率。

**设计实现**：

- 资产文件路径：`source/assets/{projectKey}/{projectKey}.assets.md`（7 层索引 §A-§G）
- 生成：由 `project-assets-update-skill §3` 引导 AI 跑 9 步探查 SOP（读 CLAUDE.md/AGENTS.md → 扫描工程结构 → 抽典型类 → 识别分层/命名/约束）。🆕 v3.4.0 修正：无独立 `ae-sdd assets generate` CLI 子命令（原描述为文档撒谎）
- 增量更新/审计：由 `project-assets-update-skill §4/§5` 引导（无独立 CLI 子命令）
- 按需读取（ES 倒排索引 + BM25 评分）：`assets read <method>`（基线读取）/ `assets outline`（5秒总览）/ `assets query "<词>"`（精准查）/ `assets section <§X.Y>`（按需拉章节）/ `assets stats`（索引统计）
- G-00 门卫自动检查：7 层索引缺任一层即 G-00 阻断；距 lastAuditedAt > 30 天触发警告

**颗粒度与边界**：项目级隔离（按 projectKey）；7 层索引 §A-§G 必须齐备；RA SKILL 强制通过 `ae-sdd assets read` 接口读取，禁止直接读文件路径。

---

## 实例化体系（4 层架构）

> 母版到项目落地的 4 层分发体系，保证 SSOT 的同时支持项目级 override，脚本自动完成从母版到用户安装的全链路构建。

**是什么**：Layer 1 母版是唯一编辑点，经脚本构建为分发包，再由安装脚本装入用户环境；接入项目通过 Layer 4 实例做 override，不修改母版。

**设计实现**：

- Layer 1 母版（SSOT）：`source/`，开发者唯一编辑点，git 跟踪
- Layer 2 实例化分发包：`dist/ae-sdd/`，由 `scripts/build_dist.py` 构建（注入 VERSION + plugin.json，剥离 CHANGELOG/docs），git ignored
- Layer 3 用户安装：`~/.claude/skills/ae-sdd/`，由 `scripts/install.py` 从 dist 装入，Claude Code 实际加载
- Layer 4 项目实例：`<project>/.ae-sdd/`，由 `ae-sdd init <dir> <key>` 创建，包含 `config.yaml（指向母版）/ state.json（空模板）/ assets/（引用）/ overrides/（项目特定规则）`
- Override 解析：项目有效规则 = 母版 defaults + overrides/（同名文件覆盖）
- 版本同步：`ae-sdd bump <ver>` 同步三处版本号（SKILL.md / paths.py / README.md:5）
- 开发者工作流：改 source/ → `bash scripts/dev-sync.sh`（build + install 一步到位）→ dev-sync 前必须 `ae-sdd update-check` 全绿

**颗粒度与边界**：Layer 2/3/4 不手工改；fork（完整复制）是显式 opt-in；override 优先级：项目 overrides/ > 母版 defaults。

---

## Harness 适配层

> 将 ae-sdd SKILL.md 自动转译为 Mavis harness 格式的 agent.md，使 ae-sdd 能作为 Mavis 团队级 agent 被编排。

**是什么**：不需要手工维护两套定义，转译脚本从 SKILL.md + HARNESS.md 生成 agent.md，母版升级后重跑即可同步。

**设计实现**：

- 产物路径：`harness/.harness/agent.md`，由 `ae-sdd-harness-adapter` SKILL 自动生成
- 转译内容：SKILL.md + HARNESS.md → harness 格式，包含 14 门禁(G-00~G-13) + 10 阶段状态机 + HS-1~HS-6 硬停止规则 + per-domain routing rules
- `.adapter.lock` 记录来源 commit hash，母版升级后 hash 变化即需重新生成
- 转译脚本：`convert-ae-sdd-to-harness.ps1`

**颗粒度与边界**：禁止手工编辑 agent.md；母版（source/SKILL.md）升级后必须重跑 adapter SKILL 重新生成；harness 格式由 Mavis 规范决定，转译脚本负责映射。

---

## 记忆层（5 层分级 + 阶段强制门禁）

> phase-aware 的 5 层持久化记忆，在关联节点强制执行 enter→write→exit 生命周期，提供跨 session 上下文连续性。

**是什么**：记忆分 5 层，关联节点（RA/design/coding-plan/coding/review）强制走完整生命周期，写入质量门禁防止 LLM 把猜测写成记忆。

**设计实现**：

- 5 层架构：L0 会话草稿（session 后可删）/ L1 Story 级记忆 / L2 项目级记忆 / L3 跨项目 pattern / L4 冷归档（postmortem/ADR）
- 强制生命周期（关联节点）：`memory enter` → 节点工作 → `memory write` → `memory exit`；exit 无 write 则失败
- v3.2.3 自动强制：`ae-sdd state write --phase <next>` 切相前自动校验 `check_state_transition()`，未完成 enter→write 则阻断
- 写入质量要求：L1+ 每条记忆必须有证据（文件路径:行号 / 报告路径 / 用户确认 / 工具结果），无证据只能留 L0
- 冲突处理：新证据与记忆冲突时写 `kind=conflict`，引用新旧两份证据，禁止静默覆盖
- 晋升规则：L0→L1 需具体证据；L2→L3 需跨项目多次发生或用户批准
- CLI：`ae-sdd memory enter / write / exit / read / search / promote / summarize`

**颗粒度与边界**：仅 5 个关联节点强制触发；kind 字段：decision/finding/issue/risk/fix/conflict/observation；L4 只读为主，禁止整体注入上下文。

---

## Plan-First 编排（CodingPlan 16 章节 + 14 门禁）

> 编码前必须先生成并经用户确认 CodingPlan，将架构决策、实现顺序、类骨架、验证点全部锁定。

**是什么**：CodingPlan 是 Coding 阶段的唯一依据，AI 不得自行调整 Plan 内容；14 条门禁全部通过才允许进入 Coding。

**设计实现**：

- CodingPlan 生成时机：Phase 2 ④bis，由 task-writer 汇总所有 Task 的任务级 CodePlan 为统一版 `{STORY-ID}-CodingPlan.md`
- CodingModel 11 维决策（嵌入每个 Task 文档）：并发控制 / 幂等策略 / 事务边界 / 缓存策略 / 错误码 / 异常处理 / 状态机实现 / 外部依赖 / 数据模型 / 可观测性 / 复用能力
- 14 条 CodingPlan 门禁：Task 0~N 全部生成、CodingModel 11 维均有明确结论、核心链路保护、资源隔离等，全部通过才允许进入 Coding
- Phase ④→⑤ 调用协议 7 项前置条件：Task 文档齐 + 每 Task 含 CodingModel 决策记录 + 每 Task 含任务级 CodePlan + TR-1~7 全通过 + 14 门禁全过 + 用户明确确认 + 决策无冲突
- 实现方案决策基线（最高优先级 4 步强制）：① 现有能力复用扫描 → ② 业内成熟方案参考 → ③ 五维代码质量评估（可用/高效/可维护/健壮/可读）→ ④ 核心能力归属唯一
- 人工审核点 2.5：AI 主动 walkthrough CodingPlan 内容 + 14 门禁状态 + CodingModel 摘要 + 风险 Task + 类骨架预览，逐条等用户确认

**颗粒度与边界**：CodingPlan 在编码前必须生成并经用户确认，不可跳过；编码过程中偏离 Plan 需用户实时确认；CodingPlan 变更触发 storyVersion 累加。

---

## 真实性扫描（3 个静态扫描器）

> 防止 LLM 伪造测试通过或伪造需求分析内容的静态扫描器，作为不可绕过的硬门禁运行时依赖。

**是什么**：三个扫描器分别覆盖测试真实性、Coding 真实性、RA 真实性，输出统一 JSON 契约，BLOCKER=0 才算通过。

**设计实现**：

- **测试真实性扫描**（`scripts/test_authenticity_scan.py`，⑥.10 硬门禁）：
  - 扫描 8 类禁止手段：`@Disabled` / `assertTrue(true)` / catch 吞异常 / 全 Mock 替代 / 期望值=实际值 / 无效测试数据 / `Thread.sleep` 绕过 / 凑覆盖率
  - 解析 Maven Surefire/Failsafe XML，与报告统计对账
  - AC × 测试方法覆盖率验证，要求 100% 覆盖；检测跳测参数（`-DskipTests` 等）
  - 由 `test-verifier` sub-agent 独立执行，主 agent 不得自我验证

- **Coding 真实性扫描**（`scripts/coding_authenticity_scan.py`，G-CODE-1）：
  - 扫描 AI Coding 反模式库 AP-1~AP-6：桩代码伪装完整实现、TODO 留空关键路径、测试数据硬编码来源不明等
  - 由 `ae-sdd gate coding-required` 在 Coding 完成/CodeReview 前自动触发

- **RA 真实性扫描**（`scripts/ra_authenticity_scan.py`，G-RA-4）：
  - 8 类禁止规则：vague-ellipsis / no-evidence / fabricated-field / hidden-conflict / masked-gap / placeholder-fill / assumed-no-derivative / missing-timeliness
  - 输出 JSON 契约，与 test_authenticity_scan.py 格式一致

**颗粒度与边界**：测试真实性扫描是 ⑥.10 硬门禁；三个扫描器均不可被 SKILL 文字描述替代；扫描器路径变更须同步更新 UC-04 分发检查。

---

## 工具链 CLI（14 大类子命令）

> ae-sdd Python CLI，将 SKILL 规则工具化，实现"规则描述 + 工具执行"双轨 SSOT。

**是什么**：规则在 SKILL.md 描述，执行在 CLI 实现，两者通过 `ae-sdd update-check` 自动验证一致性；CLI 是门禁、状态、资产、记忆等能力的统一执行入口。

**设计实现**：

- 入口：`tools/bin/ae-sdd`（Python 3），lib 模块（`assets_index / classify / db_tool / gates / gate_intercept / git_insight / memory_gate / memory_store / output / paths / session / state / update_graph` 等）
- 输出协议：JSON 走 stdout，日志走 stderr，pipeline 友好

| 类别 | 核心子命令 |
| --- | --- |
| 资产类 | `assets read / outline / section / query / stats`（🆕 v3.4.0 修正：原 check/generate/update/audit 为文档撒谎，已删除；G-00 走 `gates check --only G-00`，资产生成走 project-assets-update-skill §3）|
| 状态机类 | `state read / write / next-step / confirm / validate / show / diff / lock` |
| 入口凭证类（🆕 v3.4.0） | `enter <projectKey> [--story <ID>]`（关卡1 入口 token）|
| 路由类 | `classify / route` |
| 门禁类 | `gates check / gate ra-required / gate coding-required / gate doc-storage / review` |
| 记忆类 | `memory enter / write / exit / read / search / promote / summarize` |
| 数据库类 | `db profiles / query / explain / audit`（只读，本地 profile） |
| Git 类 | `git status / diff / log / blame / impact`（只读，结构化 JSON 输出） |
| 更新图谱 | `update-check [--only UC-XX] / update-check --affected <文件>` |
| 版本类 | `bump <ver> / version` |
| 维护类 | `health / sync-tools / init / run / quick / proposal` |

- `ae-sdd health` 9 项自检：子 SKILL 章节完整性 / 项目资产双源一致 / 规则-工具同步 / 门禁覆盖度 / TR-1~7 / 扫描器就绪 / CHANGELOG 版本一致
- `ae-sdd update-check` 6 项检查（UC-01~06）：版本号三处一致 / 门禁注册一致 / 命令契约闭环 / 扫描器分发 / 健康度清单覆盖 / 🆕 v3.4.0 文档-实现一致性（SKILL/子SKILL 命令 + HARNESS HS 规则）；dev-sync 前必须全绿
- 🆕 v3.5.3 设计-实现一致性迭代检查（人工/Agent 深度核对，补 UC 盲区）：每月/重大变更后跑，4 步 SOP —— ① 跑 UC+health+gates 作基线 ② HS 规则 vs 物理实现交叉核对（查"声明物理拦截实为零"的撒谎）③ CLI 命令契约深挖（查幽灵命令整段描述 + F-1 交叉验证覆盖面）④ 已实现未接入扫描（查 untracked + 未 import 模块）。补 UC 查不到的 4 类盲区：HS 物理拦截实现齐全度 / 幽灵命令整段描述 / 交叉验证覆盖面 / 已实现未接入。定位为 UC 的"深挖层"，不替代 UC（UC 是改完即跑的快速防线，本节是周期性深度体检）。

**颗粒度与边界**：db/git 工具为只读；🆕 v3.4.0 G-00 由 AI Agent 手动 `gates check --only G-00`（非 CLI 自动触发）；update-check 权威源是 `source/standards/update-graph.json`；改 CLI 命令契约须同步 update-graph.json（UC-03/UC-06 兜底）；🆕 v3.5.3 迭代检查不阻断 dev-sync（报告 + 修复建议，由用户决定是否本次迭代修）。

---

## 🆕 v3.4.2 state.json events 操作日志设计

### 背景与动机

v3.4.0 引入 PRD 级 state.json + 11 phase 状态机（`initialized → ra-generated → dr-generated → ... → completed`），但**子任务级操作的可追溯性弱**：

- 现有 `phase` 字段只记"当前阶段"，**没有"如何到达此处"的轨迹**
- 现有 `history` 字段只记阶段切换（`{phase, timestamp, by}`），**粒度过粗**（如 router 把请求路由到 story-generate-skill、随后该 SKILL 完成、随后用户确认 → 这三步之间发生了什么无法回溯）
- 22 门禁拦截（`G-09` 测试真实性、`G-CODEPLAN-SRC` 源码核对等）只在被触发时记录结果，**没有"何时由谁触发拦截"的 audit trail**

→ 运维场景痛点：用户问"STORY-020-BE 昨天为什么 Phase 2 没走完？"时，AI 只能查 `phase` + `history`，无法还原"哪一步门禁拦截了、是哪个 gate_id、谁确认的"。

### 方案：events 操作日志

**核心思路**：state.json 增加 append-only `events` 数组，每条记录一次"流程动作"（路由/门禁/阶段切换/用户确认等），提供"全程 audit trail"能力。

**不替代现有字段**：events 与 `phase` / `history` / `currentStory` / `currentTask` **共存**。phase = "当前阶段"（高频读），history = "阶段切换摘要"（低频读），events = "细粒度操作"（运维审计/复盘）。

### 数据结构（v3.4.2 schema v2）

**3 个枚举**（`tools/lib/flow_enums.py`）：

| 枚举 | 成员数 | 用途 |
|---|---|---|
| `FlowNode` | 6 | 流程节点原语（PRD / RA / DR / STORY / TASK / PLAN）|
| `FlowSkill` | 15 | SKILL 标识符（与 `source/skills/` 目录文件一一对应）|
| `FlowEventType` | 8 | 事件类型（routed-to / skill-completed / gate-blocked / gate-cleared / user-confirmed / phase-changed / reopened / aborted）|

**事件数据类** `FlowEvent`（继承 `str, Enum` → JSON 序列化直接得字符串，无需额外转换）：
- 必填：`seq`（自增）/ `ts`（ISO 8601 UTC）/ `event` / `node` / `by`
- 条件必填：`skill`（routed-to/skill-completed）/ `gate_id`（gate-blocked/cleared）/ `phase`（phase-changed）
- 可选：`txnName`（子任务标识，PRD 级事件为 None）/ `from_node`（路由来源）/ `reason` / `output`（产物描述）/ `meta`（预留扩展）

**5 个工厂函数**（防字段拼写错）：`make_routed_to` / `make_skill_completed` / `make_gate_blocked` / `make_phase_changed` / `make_user_confirmed`。

**2 个 lib API**（`tools/lib/state.py`）：
- `append_event(state, event)` — 原地追加，自动填 `seq` / `ts`，**不**自动落盘（调用方决定何时 write_state）
- `get_events(state, *, txn_name=None, event_type=None, node=None)` — 三维过滤（txn / event 类型 / node），按 seq 升序返回

### 关键设计决策

1. **继承 `str, Enum`**：避免 `.value` 解包，JSON 序列化直接得字符串，**日志人工读也能秒懂**（无需查枚举定义）。
2. **`append-only` + 不去重**：保证 audit trail 真实性，不做"看起来更整洁"的合并/裁剪。
3. **`txnName` 区分 PRD/Story/Task/Plan 事件**：PRD 级事件 `txnName=null`，Story/Task/Plan 用各自的 ID；`get_events(txn_name=...)` 单一过滤就能拿到"某个子任务的全过程"。
4. **不强制落盘**：事件写入与 state.json 持久化解耦，避免高频事件导致磁盘 IO 风暴。调用方按需落盘（如 SKILL 完成时批量 write_state）。
5. **向后兼容**：旧 v1 state.json 无 `events` 字段时 `get_events()` 返回 `[]` 不报错，`append_event()` 自动初始化 `events: []`。

### 已知缺口（v3.4.2 不闭环、留待后续 PR）

按 ae-sdd-update-skill §改⑤工具链 SOP 第 3 步"新增/修改门禁/CLI → 同步"原则，**当前 lib 层已发布，但业务调用方尚未接入**：

| 缺失点 | 预期接入方 | v3.4.2 状态 |
|---|---|---|
| 路由时记录 `routed-to` | `tools/bin/ae-sdd:cmd_classify` / router | ❌ 后续 PR |
| 阶段切换时记录 `phase-changed` | `tools/bin/ae-sdd:cmd_state_write` | ❌ 后续 PR |
| 门禁检查时记录 `gate-blocked` / `gate-cleared` | `tools/lib/gates.py` 各 `check_gXX()` | ❌ 后续 PR |
| SKILL 完成时记录 `skill-completed` | 各 SKILL orchestrator | ❌ 后续 PR |
| 用户确认审核点时记录 `user-confirmed` | ae-sdd-update-skill §审核点双支柱 | ❌ 后续 PR |

**当前 schema 已就绪，lib 测试已通过（32/32 PASS）**——一旦后续 PR 接入调用方，events 立即生效，无需 schema 升级。
