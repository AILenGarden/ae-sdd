---
name: agent-orchestration
description: Agent 编排 SKILL — 任务节点**内**的子任务拆分 + 多 Agent 并行 + 负载均衡 + 故障检测 + 故障补救。**澄清（2026-06-06）：** 任务拆分不是按流程节点（流程串行无法并行），而是按同一节点内的子任务（可并行）。🆕 新建，补全 AE 体系的多 Agent 协作能力。
---

# Agent Orchestration — 任务节点内子任务编排 Skill

> **🔴 核心定位（2026-06-06 修订）：**
> - **不是**按流程节点拆（Phase 1 ① → Phase 1 ② → ... 是串行的，无法并行）
> - **是**按**同一节点内**的子任务拆（如 Coding 一个 Task 时，可拆成"写 Domain 类"/"写 Application 类"/"写 Infrastructure 类"/"写 Tests"等多个子任务并行跑）
>
> **与现有 SKILL 的关系：**
> - `ae-sdd-skill.md` = 流程编排 + 智能路由（决定走哪个节点 SKILL）
> - **`agent-orchestration-skill.md`（本文件）** = 任务节点**内**的子任务编排（决定同节点内如何拆/分 Agent/补救）
> - 节点 SKILL（`story-generate / coding / code-review` 等）= 各自节点的执行
>
> **🔴 触发场景：** 任何节点 SKILL 在执行复杂任务时，可调用本 SKILL 决定"是否拆子任务 / 拆多少 / 分几个 Agent / 故障如何补救"。

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
| `story-reviewer` | Phase 1 ② | Story 评审（业务 / 前端契约 双 reviewer）|
| `testcase-writer` | Phase 1 ③ | 测试用例生成 |
| `task-writer` | Phase 2 ④ | Task 生成（含任务级 CodePlan）|
| `plan-writer` | Phase 2 ④ter | 🆕 2026-06-06 合并入 task-writer |
| `coder` | Phase 2 ⑤ | Coding |
| `code-reviewer` | Phase 3 ⑦ | Code Review（业务 / 架构 双 reviewer）|
| `test-verifier` | Phase 3 ⑥.10 | 测试真实性验证 🔴 强制 |

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
  deliverable: {产出物文件路径}  # 如 design/story/be/{STORY-ID}.md
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
| **input 必填具体文件路径** | 不写"看 Story"，写"design/story/be/STORY-001-BE.md" |
| **output 必填具体产出路径** | 不写"出报告"，写"`documentStorage.resolve_path(intent="PROPOSAL", storyId="STORY-001-BE", version=1, title="示例")`" 🆕 2026-06-17 修复 P1-3 |
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
| `ae-sdd-skill.md §统一入口` | 用户输入先经 AE-skill 路由到节点 SKILL |
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
| 10 | 触发原 SKILL 循环判定 | 评审通过 | 走完流程 |

---

## 维护

- **维护人：** 架构组
- **更新频率：** 每次新增"多 Agent 场景"或故障模式时
- **同步对象：**
  - `ae-sdd-skill.md §统一入口` 引用本 SKILL 作为"任务节点内子任务编排"
  - 各节点 SKILL（`coding / code-review` 等）复杂任务时调用本 SKILL
  - `proposal-skill.md` 在 sub-agent 故障升级时引用
- **关键变化（2026-06-06 新建）：**
  - 🆕 澄清核心误解："任务拆分 ≠ 按流程节点拆（流程是串行的）"
  - 🆕 真正的拆分原则："按同一节点内子任务拆（可并行）"
  - 🆕 5 个 Agent 硬上限（避免 token 爆炸）
  - 🆕 4 级故障补救 SOP（重试 / 重新分配 / 降级 / 升级用户）
  - 🆕 8 个角色库（与 AE-skill 角色 1-8 对齐）
  - 🆕 状态跟踪（state.json 加 agentOrchestration 字段）
