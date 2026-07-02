---
name: agent-orchestration
description: Agent 编排 SKILL — 任务节点**内**的子任务拆分 + 多 Agent 并行 + 负载均衡 + 故障检测 + 故障补救 + 多 reviewer 默认编排。**澄清（2026-06-06）：** 任务拆分不是按流程节点（流程串行无法并行），而是按同一节点内的子任务（可并行）。🆕 2026-06-25：新增 §8.4 多 reviewer 默认编排框架（横切规范 SSOT）——所有 Review 节点的"是否启用多 reviewer / 启用几个 / 视角怎么切 / 冲突怎么解"统一归本 SKILL 管理，对抗 AI 逻辑自洽陷阱。
---

# Agent Orchestration — 任务节点内子任务编排 Skill

> **🔴 核心定位（2026-06-06 修订 / 2026-06-25 扩展）：**
> - **不是**按流程节点拆（Phase 1 ① → Phase 1 ② → ... 是串行的，无法并行）
> - **是**按**同一节点内**的子任务拆（如 Coding 一个 Task 时，可拆成"写 Domain 类"/"写 Application 类"/"写 Infrastructure 类"/"写 Tests"等多个子任务并行跑）
>
> **与现有 SKILL 的关系：**
> - `SKILL.md` = 流程编排 + 智能路由（决定走哪个节点 SKILL）
> - **`agent-orchestration-skill.md`（本文件）** = 任务节点**内**的子任务编排（决定同节点内如何拆/分 Agent/补救）**+ 多 reviewer 默认编排（§8.4，所有 Review 节点的多 reviewer 决策 SSOT）**
> - 节点 SKILL（`story-generate / coding / code-review` 等）= 各自节点的执行
>
> **🔴 触发场景：** 任何节点 SKILL 在执行复杂任务时，可调用本 SKILL 决定"是否拆子任务 / 拆多少 / 分几个 Agent / 故障如何补救"。**Review 类节点额外调用 §8.4 决定"是否启用多 reviewer / Tier 几 / 视角怎么切 / 冲突怎么解"。**

---

## 0. 目标

- 让**同一节点内**的子任务可并行执行（提升效率）
- 控制**负载均衡**（避免单个 Agent 跑太多 / 资源争抢）
- 故障**早发现 + 早补救**（超时 / 输出错 / 闸门未过）
- 故障补救有 SOP（重试 / 重新分配 / 降级 / 升级用户）
- 结果可**汇总 + 交叉对比**（避免多 Agent 输出漂移）

---

## 总则（🔴 贯穿全 SKILL）

### 标尺 1：独立性判定（🔴 子任务必须真正独立才能并行）

- 拆出的子任务必须**无强依赖**（不互相等结果 / 不互相改同一文件）
- 强依赖 = 串行（不是并行）
- 弱依赖（只读对方输出）= 可并行但需最后对齐

### 标尺 2：粒度节制（🔴 不为并行而并行）

- 子任务数量 ≤ 5（多了管理成本爆炸）
- 每个子任务有明确输入/输出/判定标准
- 拆与不拆的判定（详见 §1）

### 标尺 3：故障早发现

- 每次子任务派活后**必设超时**
- 每次子任务完成**必跑闸门**
- 异常立即**标记 + 通知 root agent**

### 标尺 4：补救有上限

- 同一子任务**最多重试 3 次**
- 3 次仍失败 → **降级**（拆更细 或 合并） / **升级用户**
- **禁止**无限重试（浪费 token + 拖延流程）

---

## 1. 子任务拆分原则（🔴 关键：拆与不拆的判定）

### 1.1 拆与不拆决策表

| 场景 | 是否拆 | 拆成几块 | 说明 |
|------|-------|---------|------|
| 节点 SKILL 是简单任务（单一输出）| ❌ 不拆 | 0 | 1 Agent 跑完 |
| 节点 SKILL 是复杂任务（多输出）| ✅ 拆 | 2-5 | 按"输出维度"拆 |
| 节点 SKILL 涉及多个独立文件 | ✅ 拆 | N（文件数）| 每个文件 1 Agent |
| 节点 SKILL 涉及多维度评审 | ✅ 拆 | N（评审维度）| 每个维度 1 Agent |
| 节点 SKILL 涉及强依赖的子任务 | ❌ 不拆 | 0 | 串行（不是并行）|
| 节点 SKILL 涉及弱依赖子任务 | ✅ 拆 | 2-3 | 并行 + 最后对齐 |

### 1.2 拆法（按"输出维度"）

| 节点 SKILL | 拆法 | 子任务数 |
|----------|------|---------|
| `coding-skill.md` §⑥ Coding | 按 DDD 分层拆：Domain / Infrastructure / Application / Interfaces / Tests | 4-5 |
| `code-review-skill.md` §2 6 阶段评审 | 按评审维度拆：A 业务 + B 分层 + C DB + D 测试 + E 项目资产 + F 跨文档 | 3-6 |
| `testcase-generate-skill.md` §3 3 层覆盖 | 按覆盖层拆：第一层（按类型）+ 第二层（按通用维度） + 第三层（按测试层级）| 2-3 |
| `story-generate-skill.md` §2 7 阶段 | 按章节拆：业务背景 + 主流程 + AC + 接口 + 数据 + Task + ①bis | 5-7 |
| `task-generate-skill.md` §4 生成 Task | 按 Task 拆：每个 Task 1 个 sub-agent | N（Task 数）|

> **🆕 2026-06-25 与多 reviewer 的关系：** 上表是"5 阶段并行挖掘"（按检查维度拆）。Review 节点还可能叠加"多 reviewer 交叉审"（按审视视角拆，见 §8.4）。两者正交，但 sub-agent 总数受 §2.2 硬上限 5 约束。叠加时按 §8.4.6 实践建议处理（如 Tier 3 reviewer 各自只跑相关阶段，避免 3×5=15 超上限）。

### 1.3 不拆的反例（🔴 严禁拆）

| 反例 | 为什么不拆 |
|------|----------|
| Phase 1 ① → ② → ③ 顺序执行 | 流程是串行的，无法并行 |
| Task 1 完成后才能做 Task 2 | 强依赖 → 串行 |
| 同一文件的同一行 | 写冲突 |
| 子任务间互相改对方输出 | 写冲突 |
| 拆分后管理开销 > 并行收益 | 不拆 |

---

## 2. Agent 数量决策

### 2.1 数量决策矩阵

| 任务复杂度 | 子任务数 | Agent 数 | 策略 |
|----------|---------|---------|------|
| 简单（单一输出）| 0 | 1 | 单 Agent 串行 |
| 中等（2-3 个独立子任务）| 2-3 | 2-3 | 多 Agent 并行 |
| 复杂（4-5 个独立子任务）| 4-5 | 4-5 | 多 Agent 并行（上限）|
| 超复杂（> 5）| 6+ | **❌ 禁止** | 必须先合并 / 重新拆分 |

### 2.2 Agent 数量上限

- **🔴 硬上限：5 个**（避免 token 爆炸 + 管理成本失控）
- 如需 > 5 个，**先合并**子任务，再考虑拆分

### 2.3 资源评估

- **每个 Agent 单独工作目录**（避免 git/worktree 冲突）
- **每个 Agent 单独上下文**（避免 context 互相污染）
- **资源争抢检测**：如 5 个 Agent 同时跑大文件 → token 爆 → 降级到 3 个

---

## 3. Agent 角色分配（从角色库选）

> **🔴 来源（AE-skill 角色 1-8）：** 8 个角色已沉淀，本 SKILL 复用。

### 3.1 角色库

| 角色 | 适用节点 | 适用场景 |
|------|---------|---------|
| `story-writer` | Phase 1 ① | Story 起草 |
| `story-reviewer` | Phase 1 ② | Story 评审（按 §8.4 Tier 判定选 1/2/3 个：设计实现 / 前端契约 / 数据模型 视角）|
| `testcase-writer` | Phase 1 ③ | 测试用例生成 |
| `testcase-reviewer` | Phase 1 ③bis 🆕 v3.7.0 | TestCase Review（TC-1~TC-9，按 §8.4 Tier 判定选 1/2/3 个） |
| `task-writer` | Phase 2 ④ | Task 生成（含任务级 CodePlan）|
| `plan-writer` | Phase 2 ④ter | 🆕 2026-06-06 合并入 task-writer |
| `coder` | Phase 2 ⑤ | Coding |
| `test-runner` | Phase 3 ⑥ | Test Generate：运行编译/启动/L1-L4 测试并生成测试报告 |
| `code-reviewer` | Phase 3 ⑦ | Code Review（按 §8.4 Tier 判定选 1/2/3 个：BE / AR / QA 视角，即 A/B/C 模式）|
| `test-verifier` | Phase 3 ⑥.10 | 测试真实性验证 🔴 强制 |

> **🆕 2026-06-25 对齐说明：** reviewer 类角色（`story-reviewer` / `code-reviewer`）的"选几个"统一由 **§8.4 多 reviewer 默认编排框架** 的 Tier 判定决定，不再在各角色描述里硬编码"1-2 个"。节点 SKILL 负责"视角怎么切"，本 SKILL 负责"选几个 + 怎么交叉"，职责分离。

### 3.2 角色与子任务匹配

- **子任务 A（业务逻辑）** → `story-writer` / `coder` / `code-reviewer`(业务评审)
- **子任务 B（分层）** → `coder` / `code-reviewer`(架构评审)
- **子任务 C（数据库）** → `coder` / `code-reviewer`(架构评审)
- **子任务 D（测试）** → `coder` / `code-reviewer`(测试评审) / `test-verifier`
- **子任务 E（项目资产）** → `code-reviewer`(规范评审) / `project-assets-update-skill.md`

---

## 4. 任务分配卡（Prompt 模板）

> **🔴 统一格式：** 所有 sub-agent 派活必须用以下 YAML 结构。

```yaml
# 任务分配卡
agent_role: {角色名}  # 如 story-writer / code-reviewer
story_id: STORY-XXX-BE
sub_task_id: {parent_task_id}-sub-{N}  # 如 coding-r1-sub-1
parent_skill: {父 SKILL 名称}  # 如 coding-skill.md
priority: P0 / P1 / P2

input:
  - {文件路径 1}  # 如 Story 文档
  - {文件路径 2}  # 如项目资产
  - {约束/模板路径}

output:
  deliverable: {产出物文件路径}  # 如 `ae-sdd doc resolve --intent STORY --story-id {STORY-ID}`（路径由 document-storage-skill 动态定位，禁止硬编码）
  report: {报告文件路径}  # 如 {STORY-ID}-Story-WriterReport.md

standards:
  - {本任务必须满足的标准 1}  # 如 覆盖 7 阶段挖掘
  - {本任务必须满足的标准 2}  # 如 7 道闸全过
  - {门禁/红线}  # 如 🔴 阻断型问题必须 0

context:
  - {必要的背景信息}
  - {同节点其他子任务的进度}
  - {跨任务约束}

deadline: {最长执行时间}  # 如 30 分钟

report_back:
  channel: mavis communication  # 或其他
  target: {root session id}
  format: {报告模板路径}
```

### 4.1 任务分配卡填写原则

| 原则 | 说明 |
|------|------|
| **input 必填具体文件路径** | 不写"看 Story"，写 `ae-sdd doc resolve --intent STORY --story-id STORY-001-BE`（路径由 document-storage-skill 动态定位，禁止硬编码） |
| **output 必填具体产出路径** | 不写"出报告"，写 `ae-sdd doc save --intent PROPOSAL --story-id STORY-001-BE --content-file 草稿.md`（路径由代码tle="示例")`" 🆕 2026-06-17 修复 P1-3 |
| **standards 必填可验证标准** | 不写"做对"，写"7 道闸全 ✅" |
| **context 必填足够上下文** | sub-agent 没有 root 的全上下文，必须给够 |
| **deadline 必填** | 不写"尽快"，写"30 分钟" |
| **report_back 必填** | 失败时如何汇报 |

---

## 5. 负载均衡策略

### 5.1 平衡原则

- **🔴 不让单个 Agent 跑太多**（> 5 个文件 / > 1 个 Task / > 1 个 Story）
- **🔴 同时跑多个 Agent 时**确保资源不冲突
- **🔴 优先跑瓶颈任务**（关键路径上的任务）

### 5.2 资源评估公式

```
每个 Agent 预估成本：
- Token：~50K-200K（视任务复杂度）
- 时间：5-30 分钟（视任务复杂度）
- 文件 I/O：~10-100 个文件

总并发能力：
- token 预算：~500K（5 个 Agent × 100K）
- 时间预算：视 deadline
- 资源：1 个工作目录（避免 worktree 冲突）
```

### 5.3 平衡策略

| 策略 | 适用 | 反模式 |
|------|------|--------|
| **均分负载** | 同优先级多子任务 | 1 个 Agent 跑 10 个文件 |
| **优先瓶颈** | 有强依赖链 | 链尾任务提前跑 |
| **错峰并发** | 多 Agent 写不同文件 | 多 Agent 写同一文件 |
| **串行优先** | 弱依赖但需对齐 | 全并行后再统一对齐（漂移） |

---

## 6. 故障检测（🔴 4 大故障源）

### 6.1 4 大故障源 + 检测方法

| 故障源 | 检测方法 | 判定 |
|--------|---------|------|
| **超时** | deadline 到达但 sub-agent 未完成 | 🔴 故障 → 触发补救 |
| **输出格式错** | sub-agent 产出物不匹配 deliverables 规范 | 🔴 故障 → 触发补救 |
| **产物不存在** | 报告的产出物路径无文件 | 🔴 故障 → 触发补救 |
| **闸门未过** | sub-agent 自检 7 道闸有 🔴 未过 | 🔴 故障 → 触发补救 |

### 6.2 故障检测时机

```
派活后 → 立即开始检测
    ↓
    ├─ 每 5 分钟检查 1 次（轻量）
    ├─ deadline 到达时检查 1 次（关键）
    └─ sub-agent 报告完成时检查 1 次（最终）
```

### 6.3 故障日志

每次故障必须记录：
- sub-agent ID
- 派活时间 / 检测到故障时间
- 故障类型（超时 / 格式错 / 产物缺失 / 闸门未过）
- 故障详情（错误信息 / 缺失文件路径 / 未过闸门列表）
- 后续动作（重试 / 重新分配 / 降级 / 升级用户）

---

## 7. 故障补救（🔴 4 级补救 SOP）

### 7.1 补救决策树

```
故障检测
    ↓
第 1 次故障 → 重试（同 sub-agent 同任务）
    ├─ 成功 → 继续
    └─ 失败 → 第 2 次
    ↓
第 2 次故障 → 重新分配（新 sub-agent 同任务）
    ├─ 成功 → 继续
    └─ 失败 → 第 3 次
    ↓
第 3 次故障 → 降级（合并到其他子任务 或 拆更细）
    ├─ 成功 → 继续
    └─ 失败 → 升级用户
    ↓
3 次均失败 → 🔴 升级用户决策
    ↓
用户决策：
    ├─ 重试 → 回到第 1 次
    ├─ 改子任务 → 重新设计子任务
    └─ 暂停 → 标记 Proposal blocked
```

### 7.2 重试（Retry）

- **条件：** 故障是"瞬时"（网络超时 / 临时资源不足）
- **操作：** 同 sub-agent 同任务，**新增 context 描述故障**（让 sub-agent 知道之前失败原因）
- **上限：** 2 次（同任务）

### 7.3 重新分配（Re-assign）

- **条件：** 重试仍失败，或 sub-agent 本身有问题
- **操作：** 派**新** sub-agent 跑**同任务**，**完整 context 传递**（故障历史 + 之前 sub-agent 报告 + 用户输入）
- **上限：** 1 次

### 7.4 降级（Degrade）

- **条件：** 重新分配仍失败
- **操作：**
  - **方案 A：合并**（将该子任务合并到其他子任务，降低并行度）
  - **方案 B：拆更细**（将该子任务再拆成 2 个更小子任务，重新派活）
- **优先方案 A**（拆更细可能继续失败）

### 7.5 升级用户（Escalate）

- **条件：** 降级仍失败
- **操作：**
  - 暂停流程
  - 向用户呈现：故障详情 + 已尝试的补救 + 建议方案
  - 等待用户决策
- **禁止：** 自动跳过故障子任务（破坏流程完整性）

---

## 8. 结果汇总（🔴 root agent 的关键职责）

### 8.1 汇总流程

```
所有 sub-agent 完成
    ↓
root agent 收集所有产出物
    ↓
跑一致性检查（详见 §8.2）
    ↓
生成最终报告（按对应 SKILL 的模板）
    ↓
触发原 SKILL 的"循环判定"（如 code-review-skill.md §第六步）
```

### 8.2 一致性检查（🔴 多 Agent 输出的漂移风险）

- **跨 Agent 命名一致性**（如 sub-agent A 命名 `BossUser`，sub-agent B 命名 `User`，需要统一）
- **跨 Agent 引用一致性**（如 sub-agent A 引用 `BossUserService`，sub-agent B 引用 `IUserService`，需要统一）
- **跨 Agent 格式一致性**（如 sub-agent A 输出 markdown 表格，sub-agent B 输出纯文本）

### 8.3 交叉对比（🔴 多 reviewer 模式）

- sub-agent A 评审结果 vs sub-agent B 评审结果
- 一致 → 接受
- 不一致 → 走 Proposal 流程（用户决策）

> **🔴 本节 2026-06-25 升级说明：** 原 3 行简表不足以支撑"默认多 reviewer"。下方新增 **§8.4 多 reviewer 默认编排框架**（Tier 判定 + 视角切分 + 交叉对比算法 + 冲突决策树 + 降级规则），作为所有 Review 节点的横切规范。本 §8.3 的 3 行结论作为 §8.4 的速记结论保留，详细算法以 §8.4 为准。

---

## 8.4 多 reviewer 默认编排框架（🔴 横切规范 — 所有 Review 节点适用）

> **📍 定位（2026-06-25 新增）：** 本节是**多 reviewer 决策的 SSOT（唯一真相源）**。所有 Review 节点（DR Review / Story Review / Task Review / Code Review / RA Review 等）的"是否启用多 reviewer / 启用几个 / 视角怎么切 / 冲突怎么解"**统一查本节**，节点 SKILL 只负责"本节点的 reviewer 视角分工"（见各节点 SKILL 的 §多 reviewer 视角切分小节）。
>
> **为什么需要本节：**
> 1. **对抗 AI 逻辑自洽陷阱**——AI 审自己产出的东西，天然倾向沿生成时的推理路径自我合理化（把疑似缺陷判为误报）。单 reviewer 无法自纠此偏差。
> 2. **对抗同模型同盲区**——多个 reviewer 若是同一模型（如都是 GLM-5.2），会共享同一套认知盲区。单纯堆 reviewer 数量无法消除盲区，必须靠**视角切分**制造人为差异。
> 3. **结构化对账 > 主观意见**——多 reviewer 互审防的是"主观合理化"，但更致命的"凭空编造"要靠结构化对账门禁（如 Code Review 7 道闸、报告-代码对账）。本节负责前者，后者仍归各节点门禁。
>
> **与 §8.3 / 各节点 SKILL 的关系：**
> - §8.3 = 本节的速记结论（一致→接受 / 不一致→升级），详细算法见本节 §8.4.3
> - 本节（agent-orchestration）= **编排规则 SSOT**：Tier 判定 + 视角切分原则 + 交叉对比算法 + 冲突决策树 + 降级规则
> - 各节点 SKILL = **节点专属配置**：本节点的 reviewer 视角分工（如 Story Review = 设计实现视角 + 前端契约视角）
> - `code-review-skill.md §多 Agent 评审编排` = Code Review 节点对本节的应用范例（A/B/C 模式 = 本节 Tier 1/2/3 的实例化，已在 Code Review 落地验证）

### 8.4.1 何时启用多 reviewer（Tier 判定 — 默认启用，按复杂度分级）

> **🔴 核心立场：多 reviewer 是默认能力，不是反应式兜底。** 旧版"准确率 < 70% 才升级交叉验证"是**事后补救**，违反"缺陷越早发现代价越低"原则。本节改为**按 Tier 默认分级启用**。

**Tier 判定标准（统一口径，与 Code Review A/B/C 模式对齐）：**

| Tier | 判定（任一命中即取高 Tier） | reviewer 数 | 对应 Code Review 模式 |
|------|--------------------------|------------|---------------------|
| **Tier 1** | 微/小规模 **且** 无状态机/事务/资金/权限/外部集成等关键决策 | 1（单审） | 模式 A |
| **Tier 2** | 中规模 **或** 含状态机/事务/接口契约/跨服务集成等关键决策 | 2（双审交叉） | 模式 B |
| **Tier 3** | 大规模 **或** 全新微服务/全新表/全新 SPI/涉及资金·状态·权限/跨 4 Task+ | 3（三审交叉） | 模式 C |

**Tier 判定输入来源：**
- **规模**：来自 RA（requirement-analysis-skill）的 5 维评分输出（大/中/小/微）
- **关键决策点**：各节点本地判定（节点 SKILL 在准入检查时标注本节点是否含关键决策）
- **🆕 v3.8.0 自动化模式**：`.ae-sdd/config.yaml` 的 `automation.enabled=true` → **强制 Tier 3**，覆盖上述规模/关键决策判定（自动化模式跳过人工✅，必须最高强度联审兜底）

**Tier 判定时机：**
- Review 节点**准入检查通过后、第一步挖掘前**完成 Tier 判定
- 判定结果写入报告头部（`## Review 元信息` 的 `reviewerTier` 字段），供追溯

**降级豁免（用户显式确认才生效）：**
- 用户显式说"这次单审就行" / "Tier 降一级" → 可降级，但**必须在报告标注"用户豁免降级"**
- AI **不得自行降级**（违反 §8.4.5 降级规则）

### 8.4.2 reviewer 视角切分原则（🔴 反"同模型同盲区"的核心）

> **🔴 核心原则：reviewer 之间的差异靠"视角 lens"制造，不是靠"派两个一样的"。** 同模型 reviewer 若看同一批检查项，只会得到高度相似的结论（同盲区）。必须让每个 reviewer 聚焦**不同的审视维度**，人为制造认知差异。

**视角切分三原则：**

| 原则 | 要求 | 反模式 |
|------|------|--------|
| **① 视角正交** | 各 reviewer 的审视维度尽量不重叠（如"业务实现"vs"架构规范"） | 两个 reviewer 都跑完整 6 阶段（重叠 → 同盲区） |
| **② 覆盖本节点全部硬门禁** | N 个 reviewer 的视角并集 = 本节点全部 HARD 检查项 | 某维度无人审 → 漏审 |
| **③ 与本节点已有机制对齐** | 视角切分尽量复用本节点已有概念（如 Story Review 复用 F-Stage 前端契约） | 凭空发明新视角，与节点 SKILL 脱节 |

**各节点 reviewer 视角切分（节点 SKILL 专属配置，本表为速查索引）：**

| 节点 | Tier 2 双审视角 | Tier 3 三审视角 | 视角配置出处 |
|------|---------------|---------------|------------|
| RA Review | 需求完整性 + 场景可行性 | + 风险预判闭环 | `requirement-analysis-skill.md` |
| DR Review | 业务价值对齐 + 架构拆分合理性 | + 跨 Story 边界 | `dr-review-skill.md` |
| Story Review | 设计实现 + 前端契约(F-Stage) | + 数据模型/DB | `story-review-skill.md` |
| Task Review | Story 覆盖完整性 + 实现可行性 | + 风险 Task 专项 | `task-generate-skill.md §5bis` |
| Code Review | BE 业务实现 + AR 架构规范 | + QA 测试真实性 | `code-review-skill.md §多 Agent 评审编排`（已落地 A/B/C） |

> **📍 节点 SKILL 落地要求：** 上述每个节点 SKILL 应新增 `### 多 reviewer 视角切分`（或等价小节）写明本节点 Tier 2/3 的具体视角分工。**落地状态（2026-06-25 已全量落地）**：Story Review / Code Review / DR Review / Task Review 四节点均已新增视角切分小节；**RA Review 无独立节点**（requirement-analysis 无独立 RA Review 流程，RA 修订走 requirement-analysis §修订影响），故本表 RA Review 行仅作概念占位，实际不触发多 reviewer 编排。

### 8.4.3 交叉对比算法（root agent 执行，reviewer 全部返回后跑）

> **🔴 适用场景：** Tier 2/3 启用 ≥ 2 个 reviewer 时，必须跑本算法。Tier 1 单 reviewer 跳过本节。

**算法 5 步：**

```
1. 收集所有 reviewer 报告（2-3 份，按位置聚合）
   ↓
2. 建立"缺陷-评级-位置"三维表
   行 = 缺陷 ID（按位置聚合：章节号/文件:行号/检查项编号）
   列 = reviewer 名称（如 reviewer-BE / reviewer-AR / reviewer-QA）
   值 = 该 reviewer 对该缺陷的评级（🔴/🟠/🟡/🟢/未发现）
   ↓
3. 分类判定（逐行）：
   ├─ 所有 reviewer 一致（同一评级）        → 接受该评级
   ├─ 评级不同但有 ≥1 名给 🔴             → 升级为 🔴 阻断型（取最高）
   ├─ 评级不同但都 ≤ 🟠                   → 取最高评级（🟠 严重型）
   └─ 某 reviewer 发现某缺陷但其他未发现    → 列为"存疑项"，走 §8.4.4 决策树
   ↓
4. 一致性核查：每个最终 🔴/🟠 缺陷必须有对应客观证据
   （引用到 DR/Story/Task/代码 的具体位置，禁裸结论）
   ↓
5. 生成最终 Review 报告（按聚合表 + 节点模板生成）
```

**三维表示例：**

| 缺陷 ID | 位置 | reviewer-BE | reviewer-AR | reviewer-QA | 最终判定 |
|---------|------|------------|------------|------------|---------|
| D-001 | Story §3.2 状态机 | 🔴 | ✅ | — | 🔴（取最高） |
| D-002 | Task-2 事务边界 | 🟠 | 🟡 | — | 🟠（取最高） |
| D-003 | 接口契约字段 | ✅ | 未发现 | 🔴 | 🔴（存疑→决策树） |

### 8.4.4 不一致项处理决策树

> **🔴 适用：** §8.4.3 第 3 步的"存疑项"（某 reviewer 发现、其他未发现，或评级严重冲突）。

```
存疑项判定
    ↓
情况 1：业务视角 reviewer 给 🔴 / 其他给 ✅
  → 视为 🔴 阻断型（业务实现错是核心，宁严勿松）
  → 触发对应节点的修复循环

情况 2：架构视角 reviewer 给 🔴 / 其他给 ✅
  → 视为 🔴 阻断型（架构错影响全局）
  → 触发修复循环 + 评估是否需更新项目资产

情况 3：评级仅差一级（如 🟠 vs 🟡）
  → 取高评级（🟠）
  → 写入本节点 UpdatePlan（如 StoryReviewUpdatePlan）

情况 4：某 reviewer 发现某缺陷、其他全部未发现
  → 列入"存疑项"
  → root agent 自己读产出物复核（不得默认信任任一方）
  → 复核后确认 → 纳入最终报告；复核后否决 → 记录否决理由

情况 5：所有 reviewer 评级一致
  → 接受，无不一致
```

> **🔴 root agent 复核职责（防默认信任）：** 情况 4 中，root agent **必须自己读双方产出物**判断对错，不得"哪边 reviewer 多就信哪边"或"默认信评级高的"。违反 = 汇总失败（对齐 §门禁与规则第 3 条）。

### 8.4.5 降级规则（环境不支持物理 sub-agent 时）

> **为什么需要降级：** 部分运行环境（单 session、无 mavis spawn 能力）无法派物理 sub-agent。此时不能阻断流程，但也不能假装"多 reviewer 已达标"。

**降级执行方式：**

| 情况 | 降级动作 | 强制标注 |
|------|---------|---------|
| 环境不支持物理 sub-agent | 同一 AI 用**不同视角的 prompt** 跑 N 遍（每遍聚焦一个视角 lens） | 报告头部标注 `reviewerMode: "logical-multi-perspective"`（逻辑多视角，非物理独立） |
| token 预算不足 | Tier 3 → Tier 2（减一个 reviewer） | 报告头部标注 `tierDowngraded: true` + 降级原因 |
| 用户显式要求单审 | Tier 2/3 → Tier 1 | 报告头部标注 `userWaiver: true` |

**🔴 降级非等效：**
- 逻辑多视角**仍受同模型同盲区影响**——同一 AI 切换视角，盲区并未消除，只是降低。这是退路，不是"多 reviewer 已达标"的替代品。
- 标注 `reviewerMode: "logical-multi-perspective"` 的报告，在下游节点（如 Story Review 结论被 Task Review 引用时）**必须显式提示"上游为逻辑多视角，交叉验证强度降低"**，让下游知悉风险。
  - **落地位置（2026-06-25 已全链路落地）**：上游节点在"退出状态摘要"输出 `reviewerMode` 字段（Story Review / DR Review 均已落地）；下游 Task Review 读取该字段并在其报告标注"上游逻辑多视角，交叉验证强度降低"。链路闭合：DR Review → Story Review → Task Review。

**禁止的降级：**
- ❌ AI 自行决定"这次不派多 reviewer"（必须有用户豁免或环境/预算硬约束）
- ❌ 降级后不标注（等于隐瞒风险）
- ❌ 把逻辑多视角当成物理多 reviewer 宣称"已交叉验证"
- ❌ **🆕 v3.8.0 自动化模式下用逻辑多视角降级**：`automation.enabled=true` 时必须物理 3 个独立 session reviewer（G-09B 校验 sessionId≠root）；环境不支持物理 sub-agent → **不得降级为 logical-multi-perspective**，必须 `state.phase=paused` 等用户（按 `automation.onConsensusStall`）。自动化模式跳过人工✅，逻辑多视角无法消除同模型同盲区，联审形同虚设

### 8.4.6 与 5 阶段并行挖掘的关系（🔴 正交叠加，不冲突）

> **📍 概念澄清（2026-06-25）：** ae-sdd 现有 Review 节点已有"5 阶段并行挖掘"（A-E 各派一个 sub-agent，按**检查维度**分工）。本节的"多 reviewer"是**另一层**（按**审视视角**分工）。两者正交，可叠加。
>
> **🆕 v3.4.3 术语澄清：** 本节"A-E"指 Story Review 专属 5 阶段（A 一致性 / B AC / C 数据 / D 模板 / E 约束），与 code-review 的 A-F 6 阶段（A 业务 / B 分层 / C DB / D 测试 / E 项目资产 / F 跨文档）语义不同。各 review 节点的阶段定义详见 [`review-loop-skill.md` 各节点专属配置表](review-loop-skill.md)。

| 维度 | 5 阶段并行挖掘 | 多 reviewer 交叉审（本节） |
|------|--------------|------------------------|
| 分工依据 | 检查维度（A 一致性 / B AC / C 数据 / D 模板 / E 约束） | 审视视角（业务 / 架构 / 前端 / 测试） |
| 每个 sub-agent 跑什么 | 跑**一个阶段**的全部检查项 | 跑**全部阶段**但聚焦一个视角 |
| 防什么 | 效率（并行提速） | 逻辑自洽（交叉防漏） |
| 关系 | 与多 reviewer **正交**，可叠加 | 与 5 阶段并行 **正交**，可叠加 |

**叠加示意（Tier 3 Story Review）：**

```
reviewer-设计实现视角 ──┐
                       ├── 各自内部可 5 阶段并行挖掘（A-E）
reviewer-前端契约视角 ──┤    （sub-agent 内部的事）
                       │
reviewer-数据模型视角 ──┘
        │
        ▼
   root agent 跑 §8.4.3 交叉对比算法
        │
        ▼
   生成最终 Story Review 报告
```

> **🔴 叠加规则（2026-06-25 修订 — 解决与节点"不得跳过阶段"硬门禁的冲突）：**
> - **5 阶段并行挖掘 = 单 reviewer 内部机制**：每个 reviewer 各自完整跑本节点全部阶段（满足各节点"不得跳过任何阶段"硬门禁）。
> - **多 reviewer 之间串行**：N 个 reviewer 依次执行，sub-agent 总数按 **reviewer 数**计（≤3），**不是** reviewer×阶段数。
> - **上限核算**：Tier 2/3 的峰值 sub-agent 数 = max(单 reviewer 内部并行阶段数, 通常 ≤5)，reviewer 串行不叠加 → 不超 §2.2 硬上限 5。
> - **视角差异靠 prompt lens 制造**，不靠"切分阶段"——各 reviewer 跑同样阶段但聚焦点不同（如设计实现视角在 A/B/C 投入更深，前端契约视角在 F-Stage 投入更深）。
>
> **❌ 已废弃表述（2026-06-25 删除）：** 早期草稿曾建议"各 reviewer 只跑自己视角相关阶段（如前端契约视角只跑 A+F）"以绕开 5 上限——此建议与各 Review 节点"不得跳过任何阶段"硬门禁冲突，已删除。改用"reviewer 串行 + 单 reviewer 内部并行"模型。

### 8.4.7 多 reviewer 与现有机制的兼容性

| 现有机制 | 与多 reviewer 的关系 | 是否改动 |
|---------|-------------------|---------|
| Code Review A/B/C 模式 | A/B/C = 本节 Tier 1/2/3 的实例化（已落地） | ❌ 不改（作为范本） |
| Code Review 交叉对比算法（code-review-skill §多 Agent） | 算法上提到本节 §8.4.3，code-review 改为引用本节（去重） | ✅ 已完成（2026-06-25）：code-review §多 Agent 评审编排 已改为引用 §8.4.3/§8.4.4，保留 Code Review 专属配置 |
| Story Review 漏报升级规则（准确率<70% 引入交叉验证） | 从"反应式兜底"升级为"Tier 2+ 默认已有" | ✅ 已完成（2026-06-25）：story-review 漏报升级规则改为"Tier 1→升 Tier 2 / Tier 2-3→换 reviewer"，并新增 §第一步 bis 视角切分配置 |
| 各节点 5 阶段并行挖掘 | 与多 reviewer 正交叠加（§8.4.6） | ❌ 不改（保留） |
| 各节点检查清单/门禁条目（TR-1~7 等） | 单 reviewer 也要跑全部，多 reviewer 是叠加的交叉层 | ❌ 不改（保留） |
| test-verifier 独立验证（⑥.10） | 与多 reviewer 互补（一个防主观、一个防伪造） | ❌ 不改（保留） |
| DR Review 节点视角切分配置 | §8.4.2 要求各节点落地 | ✅ 已完成（2026-06-25）：dr-review 新增 §第一步 bis 视角切分（业务价值/架构拆分/跨边界）；3 处旧版"多 reviewer 冲突→升级最高等级"措辞改为引用 §8.4.4；退出流转落地 reviewerMode 字段 |
| Task Review 节点视角切分配置 | §8.4.2 要求各节点落地 | ✅ 已完成（2026-06-25）：task-generate §5bis 新增多 reviewer 视角切分（Story 覆盖/实现可行/风险 Task）；Review 结论产出落地 reviewerMode（本节点）+ 读取上游 Story Review reviewerMode |
| reviewerMode 降级提示下游落地 | §8.4.5 要求下游节点读取并标注 | ✅ 已完成（2026-06-25）：Story Review 退出摘要输出 reviewerMode；DR Review 退出流转输出 reviewerMode；Task Review 结论产出读取上游 reviewerMode 并标注风险。链路闭合（DR→Story→Task） |
| RA Review 节点 | §8.4.2 索引列为占位，但 requirement-analysis 无独立 RA Review 流程 | ❌ 不触发多 reviewer 编排（RA 修订走 requirement-analysis §修订影响）|

### 8.4.8 多 reviewer 执行清单（root agent）

| # | 动作 | 产出 | 门禁 |
|---|------|------|------|
| 1 | 准入通过后判定 Tier（§8.4.1） | `reviewerTier` 字段 | 必有判定 + 写入报告头部 |
| 2 | 查节点 SKILL §多 reviewer 视角切分，确定视角分工 | 视角列表 | 视角并集 = 本节点全部 HARD 项 |
| 3 | 派 N 个 reviewer（按 §4 任务分配卡） | N 个任务卡 | 各卡 input/output/standards 齐 |
| 4 | 各 reviewer 内部执行（可 5 阶段并行挖掘） | N 份 Review 报告 | 各报告含客观证据 |
| 5 | 跑 §8.4.3 交叉对比算法 | 聚合三维表 | 逐行判定有结论 |
| 6 | 跑 §8.4.4 不一致决策树（如有存疑项） | 决策日志 | root agent 复核存疑项 |
| 7 | 生成最终 Review 报告（含 reviewer 元信息） | 最终报告 | `reviewerMode` 字段齐 |
| 8 | 降级时标注（§8.4.5） | 降级标注 | 不得隐瞒降级 |

---

## 8.5 🆕 v3.5.5 默认单 sub-agent 模式（单 Story 也派活）

> **🔴 背景：** v3.5.4 及之前的默认是"单 Agent 串行做所有事"，主会话被迫读全部子 SKILL、读源码、写文档、做 walkthrough、跑测试 → 上下文爆炸。v3.5.5 起主会话职责收口（详见 SKILL.md §🤖 主会话职责边界），**默认单 Story 也派 1 个 sub-agent**，主会话只承担编排层职责。
>
> **🟢 与 §8.4 多 reviewer 的关系：** §8.4 是 Review 节点的多 reviewer 编排（已稳定）；本节 §8.5 是**单 Story 单 sub-agent** 的派活默认行为（v3.5.5 新增）。两者正交，不替代。

### 8.5.1 主会话 vs sub-agent 职责划分

| 类别 | 主会话负责 | 派给 sub-agent |
|---|---|---|
| **SKILL 文档** | 仅读 SKILL.md 编排层内容 | 读子 SKILL 模板按节点执行 |
| **源码** | 不读源码 | 读源码做分层 walkthrough |
| **文档产出** | 不写流程文档 | 写 Story / Task / CodingPlan / TestCase / CodeReview |
| **讲解** | ✅ 主笔（5-7 维度故事由主会话在对话中产出）| 准备讲解素材 |
| **CLI 调用** | ✅ `ae-sdd state/gates/iteration-check/context-pressure` | 跑 `mvn test` / Surefire XML / `scripts/test_authenticity_scan.py` |
| **状态落盘** | ✅ 写 `state.json` / `session.json` | 不直接写 |
| **用户对话** | ✅ 输入分析、✅/⚠️/⏸️ 收口 | 不直接对话用户 |

### 8.5.2 不派活的例外（保留路径）

| 例外 | 原因 | 处理 |
|---|---|---|
| 微任务（类型 4）| 单文件/单枚举值，机械改动 | 主会话直做，跳过 sub-agent 派活 |
| BUG/配置类 | coding-skill BUG 路径 | 主会话直做 |
| 用户明确豁免 | "主会话直做 / 不要派活" | 尊重用户，记录到 `state.json.contextNote`（可选）|
| ⑥.10 test-verifier | v3.4.0 强制独立验证 | 即使其他节点主会话直做，本节点也强制派 |

### 8.5.3 单 sub-agent 派活协议

参照 §4 任务分配卡，v3.5.5 默认参数：

```yaml
agent_role: <单节点角色，如 coder / task-writer>
story_id: STORY-XXX-BE
priority: P0 / P1 / P2

input:
  - Story 文档 / Task 文档 / CodingPlan（按节点）
  - 项目资产 / 约束 / 模板路径

output:
  deliverable: <节点产出物文件路径>
  report: <节点报告文件路径>

standards:
  - 必须按对应子 SKILL 模板执行
  - 必须输出结构化报告（参照 §4.1）

context:
  - 必要的 Story 背景信息
  - 与其他 Story 的关联（如有）
  - 上一步的产出物引用

deadline: <最长执行时间>

report_back:
  channel: mavis communication
  target: <root session id>
  format: <节点报告模板>
```

### 8.5.4 与 §8.4 多 reviewer 的衔接

- 单 Story 默认派 1 个 sub-agent（§8.5）
- Review 节点默认派 1/2/3 个 reviewer（§8.4，按 Tier 判定）
- 两者独立，**总 sub-agent 数 = 单 Story 数 + Review reviewer 数**，需 ≤ §2.2 上限 5

---

## 8.6 🆕 v3.5.5 节点级派活清单（审核点 → sub-agent 映射）

> **🔴 与 §8.5 的关系：** §8.5 是"派活协议通用规则"；本节是"具体节点的派谁"——SKILL.md §整体流程图的每个审核点对应的 sub-agent 角色映射。

### 8.6.1 审核点 → 派活映射表

| 审核点 | 主会话职责 | 派活角色 | 产出物 | 必须输出报告 |
|---|---|---|---|---|
| 1（设计阶段完成）| 讲解 + 收口 | `story-writer` + `testcase-writer` + `testcase-reviewer` | Story 文档 + TestCase 文档 + TestCase Review 报告 | `*-Story-WriterReport.md` + `*-TestCase-WriterReport.md` + `*-TestCase-Review-r{N}.md` |
| 1.5（实现方案预确认）| 讲解 5 维度 + 收口 | `task-writer`（草稿实现方案）| `{STORY-ID}-Task实现方案.md` | `*-Task-WriterReport.md` |
| 2（Task 文档完成）| 讲解 + 逐文件收口 | `task-writer`（写 Task 文档）| Task 文档集 | `*-Task-WriterReport.md` |
| 2.5（CodingPlan 评审）| 讲解 + 收口 | `task-writer`（汇总统一版 CodingPlan）| `{STORY-ID}-CodingPlan.md` | 含 16 章节 + 14 门禁自检表 |
| 4（CodeReview 完成）| 讲解 + walkthrough + 收口 | `coder` + `code-reviewer` | 代码 + CodeReview 报告 | `*-Coding-CoderReport-r{M}.md` + `*-CodeReview-v{N}-r{M}.md` |
| 5（PRD 完成确认）| 讲解 5 维度（PRD 级视角）+ 收口 | `summary-writer` | `.auto-engineering/{PRD-ID}/summary.md` | `*-PRD-SummaryReport.md` |
| ⑥（Test Generate）| 调用声明 + 汇总测试摘要 | `test-runner` | `{STORY-ID}-Report-v{N}-r{M}.md` | 含原始日志 + XML + G-09 扫描 |
| ⑥.10（Test Review）| 独立派 `test-verifier` 跑 | `test-verifier`（v3.4.0 强制）| 新版 TEST_REPORT 复核章节 | 含 session_id + TV-1~TV-10 |

### 8.6.2 单节点派活粒度建议

| 节点 | 推荐 sub-agent 数 | 理由 |
|---|---|---|
| 审核点 1 | 1-3（story-writer + testcase-writer + testcase-reviewer） | Story 与 TestCase 生成可并行；TestCase Review 依赖 TestCase 生成完成；小 Story 可合并 sub-agent |
| 审核点 1.5 / 2 / 2.5 | 1（task-writer）| 顺序依赖强，并行收益小 |
| 审核点 4 | 1-2（coder + code-reviewer）| 强依赖（CodeReview 依赖代码完成）|
| 审核点 5 | 1（summary-writer）| 单 PRD 收尾，无并行需求 |
| ⑥ | 1（test-runner）| 运行测试与出报告，强依赖代码完成 |
| ⑥.10 | 1（test-verifier）| v3.4.0 已固定，强制独立 |

### 8.6.3 节点边界上下文压力软提示（🆕 v3.5.5）

每个审核点用户 ✅ 确认后，主会话**必须**调一次：

```bash
ae-sdd context-pressure --story {STORY-ID}
```

软提示 SOP 详见 SKILL.md §⏱️ 节点级上下文压力软提示。本节只强调：**sub-agent 完成后 → root agent 汇总 → 调用 context-pressure → 用户确认 → 派下一节点 sub-agent**。

> **🔴 红线：** context-pressure 不阻断流程（report-only），critical 时输出推荐动作清单，由用户决定是否进入 PRD 收尾。

---

## 9. 状态跟踪

### 9.1 状态字段（追加到 state.json）

```json
{
  "storyId": "STORY-001-BE",
  "currentPhase": "Phase 2",
  "currentStep": "step-4-coding-r2",
  "agentOrchestration": {
    "subTasks": [
      {
        "subId": "coding-r2-sub-1",
        "agentRole": "coder",
        "agentId": "agent-001",
        "status": "running | done | failed | escalated",
        "startedAt": "2026-06-05T10:00:00",
        "deadline": "2026-06-05T10:30:00",
        "completedAt": null,
        "deliverable": "domain/BossUserDO.java"
      },
      {
        "subId": "coding-r2-sub-2",
        "agentRole": "coder",
        "agentId": "agent-002",
        "status": "running",
        "startedAt": "2026-06-05T10:00:00",
        "deadline": "2026-06-05T10:30:00"
      }
    ],
    "failures": [
      {
        "subId": "coding-r1-sub-3",
        "type": "timeout | format | missing | gate",
        "retryCount": 2,
        "lastAction": "re-assign | degrade | escalate"
      }
    ]
  }
}
```

### 9.2 状态展示

```
【STORY-001-BE Coding r2 多 Agent 状态】
├─ sub-1: coding-r2-sub-1 (coder / agent-001) — ✅ done (10:25)
├─ sub-2: coding-r2-sub-2 (coder / agent-002) — ✅ done (10:30)
├─ sub-3: coding-r2-sub-3 (coder / agent-003) — ⏳ running
└─ sub-4: coding-r2-sub-4 (coder / agent-004) — 🔴 failed → re-assigned → ✅ done (10:45)
```

---

## 10. 与现有 SKILL 的衔接

| 上下游 SKILL | 衔接点 |
|------------|-------|
| `SKILL.md §统一入口` | 用户输入先经 AE-skill 路由到节点 SKILL |
| 节点 SKILL（`coding / code-review / story-generate` 等）| 复杂任务时调用本 SKILL 决定"是否拆子任务" |
| `proposal-skill.md` | sub-agent 故障时升级用户 → 触发 Proposal 走流程 |
| `document-storage-skill.md` | sub-agent 产出物路径按本 SKILL §2 路径模板确定 |

### 10.1 调用方式（节点 SKILL 如何调用本 SKILL）

节点 SKILL 在"决定是否拆子任务"时：

```yaml
# 节点 SKILL 在 §整体流程 中插入
#### 步骤 X：决定是否拆子任务（调用 agent-orchestration-skill.md）
1. 查 agent-orchestration-skill.md §1 拆与不拆决策表
2. 查 §2 Agent 数量决策
3. 查 §3 角色分配
4. 派活（如拆）
5. 状态跟踪（按 §9 格式写 state.json）
6. 故障补救（如发生）
7. 结果汇总
```

---

## 11. 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止按流程节点拆 | 串行无法并行 | §1 拆与不拆决策表 |
| 2 | 禁止 > 5 个 sub-agent | token 爆炸 + 管理失控 | §2.2 硬上限 5 |
| 3 | 禁止同一文件多 sub-agent 改 | 写冲突 | §1.3 不拆的反例 |
| 4 | 禁止无限重试 | 浪费 token | §7 4 级补救 + 3 次上限 |
| 5 | 禁止跳过故障子任务 | 破坏流程完整性 | §7.5 升级用户 |
| 6 | 禁止 root agent 不汇总就退出 | 漏 Agent 输出 | §8 结果汇总必跑 |
| 7 | 禁止 sub-agent 输出格式不统一 | 汇总困难 | §4 任务分配卡统一格式 |
| 8 | 禁止不写 state.json 跟踪 | 失追溯 | §9 状态跟踪必填 |

---

## 12. 执行清单

| # | 动作 | 产出 | 门禁 |
|---|------|--------|------|
| 1 | 查 §1 拆与不拆决策表 | 拆/不拆判定 | 判定合规 |
| 2 | 查 §2 Agent 数量决策 | 数量 | ≤ 5 |
| 3 | 查 §3 角色分配 | 角色列表 | 角色与子任务匹配 |
| 4 | 写 §4 任务分配卡 | YAML 卡 | 字段齐 |
| 5 | 派活（root agent 调度）| sub-agent 启动 | 含 deadline / report_back |
| 6 | 状态跟踪（按 §9 写 state.json）| 状态文件 | agentOrchestration 字段齐 |
| 7 | 故障检测（按 §6 4 大故障源）| 故障日志 | 每 5 分钟检查 1 次 |
| 8 | 故障补救（按 §7 4 级补救 SOP）| 补救记录 | ≤ 3 次重试 |
| 9 | 结果汇总（按 §8 一致性检查）| 汇总报告 | 跨 Agent 一致性 ✅ |
| 9b | **多 reviewer 节点额外**：跑 §8.4.3 交叉对比 + §8.4.4 冲突决策树 | 聚合表 + 决策日志 | 存疑项 root 已复核 |
| 10 | 触发原 SKILL 循环判定 | 评审通过 | 走完流程 |

> **🆕 2026-06-25：** 第 9b 步仅当 Review 节点启用多 reviewer（Tier 2/3）时执行。Tier 1 单 reviewer 跳过 9b。是否启用多 reviewer 由 §8.4.1 Tier 判定决定。

---

## 维护

- **维护人：** 架构组
- **更新频率：** 每次新增"多 Agent 场景"或故障模式时
- **同步对象：**
  - `SKILL.md §统一入口` 引用本 SKILL 作为"任务节点内子任务编排"
  - 各节点 SKILL（`coding / code-review` 等）复杂任务时调用本 SKILL
  - `proposal-skill.md` 在 sub-agent 故障升级时引用
- **关键变化（2026-06-06 新建）：**
  - 🆕 澄清核心误解："任务拆分 ≠ 按流程节点拆（流程是串行的）"
  - 🆕 真正的拆分原则："按同一节点内子任务拆（可并行）"
  - 🆕 5 个 Agent 硬上限（避免 token 爆炸）
  - 🆕 4 级故障补救 SOP（重试 / 重新分配 / 降级 / 升级用户）
  - 🆕 8 个角色库（与 AE-skill 角色 1-8 对齐）
  - 🆕 状态跟踪（state.json 加 agentOrchestration 字段）
- **关键变化（2026-06-25 升级 — 多 reviewer 默认编排）：**
  - 🆕 §8.4 新增"多 reviewer 默认编排框架"（横切规范 SSOT）：Tier 判定 + 视角切分原则 + 交叉对比算法 + 冲突决策树 + 降级规则 + 与 5 阶段并行挖掘的正交关系
  - 🆕 §3.1 角色库 reviewer 数量从硬编码改为"由 §8.4 Tier 判定决定"
  - 🆕 §1.2 拆法表标注与多 reviewer 的正交叠加关系
  - 🆕 §12 执行清单新增 9b 步（多 reviewer 交叉对比）
  - 🔴 设计哲学升级：多 reviewer 从"反应式兜底"（准确率<70% 才升级）改为"按 Tier 默认启用"，对抗 AI 逻辑自洽陷阱
