# ae-sdd 流程设计规范

> **版本：** v1.0（2026-07-01）  
> **作者：** 陈聪（口述）  
> **来源：** ae-sdd 核心设计会话  
> **用途：** ae-sdd 主入口 SKILL 及主流程监管器的设计依据

> **⚠️ 已废弃 / 历史文档（2026-07-28 标注）：** 本文是 ae-sdd 早期（Python/hook 时代）的设计记录，正文与当前 Rust 实现严重漂移——phase 链含实现中不存在的 `story-reviewed`/`testcase-reviewed`；TestCase 写成必经环节（实现中为可选）；路由描述为 LLM 裁定（实现为确定性 RouteEngine）；`correctionCounts`/B1-B4/「Review 最多 3 轮」等均与实现不符。**当前权威：`source/SKILL.md`（声明式契约）+ `constraints/`（工程约束 SSOT）+ `crates/`、`bins/` 的 Rust 实现**（FlowRuntime 拥有相位/门禁/转换；ContextProjection/CompactCycle 拥有上下文注入与压缩生命周期）。正文保留仅供历史参考，请勿据本文档实现、评审或排障。

---

## 概述

ae-sdd 是一套端到端自动化工程工作流。AI 作为「主流程监管器」，从需求输入到代码交付全程驻守，按系列推进，不跳步、不偏离。

**核心原则：**
- 流程可以暂离（步出讨论），但不能偏离（跳节点、从错误位置开始、编码时脱离流程）
- 任何编码动作必须先回到流程
- 监管器只编排+校验，不执行具体业务

---

## 一、5 步启动序列

ae-sdd 启动后，**必须按序执行以下 5 步**，禁止跳过任意步。

### Step 1 — 工作区确认

检查当前目录下是否存在 ae-sdd 工程文件（`.ae-sdd/`）。

| 场景 | 行为 |
|------|------|
| `.ae-sdd/` 不存在 | 执行初始化脚本（创建 `state.json` + 启动主流程监管器），完成后进 Step 2 |
| `.ae-sdd/` 已存在 | 读取 `state.json`，根据 `phase` 值判断续接/重启/进 Step 2 |

**续接规则：**

| phase 值 | 行为 |
|----------|------|
| `paused` | 输出暂停播报，三选一：恢复 / 新任务 / 查看状态 |
| in-progress | 输出续接播报，从当前节点继续 |
| `initialized` / `completed` | 进入 Step 2 |

### Step 2 — 项目资产检查

运行 G-00 门禁检查项目资产。

| 场景 | 行为 |
|------|------|
| 资产存在且 7 层索引齐全 | 通过，进 Step 3 |
| 资产缺失或索引不完整 | 加载 `project-assets-update-skill.md §3` 生成资产，完成后进 Step 3 |

### Step 3 — 智能路由

分析用户输入，**两层裁定：任务类型 → 任务规格**。

#### 第一层：任务类型裁定

| 任务类型 | 判定信号 | 路由 |
|---------|---------|------|
| ae-sdd 自更新 | 用户要求修改 SKILL / 优化 ae-sdd 本身 | → `ae-sdd-update-skill.md`（监管器全权交接，state 标记 `ae-sdd-update`，收尾由 update-skill 负责）|
| 编码 | 其余所有编码类需求 | → 进入第二层规格裁定 |

#### 第二层：任务规格裁定（仅编码类）

正常流程完整路径：`RA系列 → DR系列 → Story系列 → TestCase系列 → Task系列 → Coding系列 → Test系列 → 交付`

路由根据「已有产物」决定**从哪个节点进入**：

| 规格 | 判定依据 | 进入节点 |
|------|---------|---------|
| **大任务** | 已有 DR | -> DR 系列 |
| **中任务** | 已有 Story（无 DR）| -> Story 系列 |
| **小任务** | 已有 Story+TestCase | -> CodingPlan 系列 |
| **微任务** | BUG / 改逻辑 / 调整代码（无完整产物链）| -> CodingPlan 系列（无文档）|
| — | 新需求无任何产物且非 BUG | 🔴 **阻断**：`新功能开发必须以 PRD 为起点` |

### Step 4 — 执行编码流程

主流程监管器管理每个系列的执行，详见 §二。

### Step 5 — 收尾交付

所有系列流程完结后：

1. ⑦ter 流程收尾合规自检（5 维度）
2. ⑧ 完成输出，输出交付报告
3. `ae-sdd state write --phase completed`
4. 主流程监管器退出

---

## 二、每系列执行流程（主流程监管器管理）

每个系列（RA / DR / Story / **TestCase** / Task / Coding / Test）执行标准 **4 步**：

```
┌─────────────────────────────────────────────┐
│  Step 1  compact + SKILL 调用声明            │
│  Step 2  generateSkill + agent-orchestration │
│  Step 3  reviewSkill + Loop（最多3轮）        │
│  Step 4  人工审核 → 推进或暂停               │
└─────────────────────────────────────────────┘
```

### Step 1 — 压缩上下文 + 调用声明

```
/compact                          # 防跨系列上下文污染
```

输出 SKILL 调用声明：

```
【主流程监管器 → 调用 SKILL】
调用：{skill}  |  理由：{reason}  |  期望产出：{list}  |  完成后 phase 推进至 {X}
```

### Step 2 — 生成（generateSkill + agent-orchestration）

1. 加载当前系列的 `{series}-generate-skill.md`
2. 调用 `agent-orchestration-skill`，由其按当前任务量分配子 Agent 做 workflow
3. 产物生成完成后执行产物核查（gates check）

### Step 3 — Review Loop（generateSkill + reviewSkill 循环）

1. 加载当前系列的 `{series}-review-skill.md`
2. 调用 `agent-orchestration-skill`，分配子 Agent 执行 Review
3. 汇总错误报告，交给主流程监管器

**Loop 控制（主流程监管器监管）：**

| 条件 | 行为 |
|------|------|
| 有错误 + 矫正次数 < 3 | 回到 Step 2，generateSkill 根据错误报告调整产物 |
| 矫正次数 = 3 | Level 3 暂停，等待用户介入 |
| 连续 3 轮无新增错误 | 退出 Loop，进入 Step 4 人工审核 |

### Step 4 — 人工审核

主流程监管器播报产物摘要，等待用户确认：

| 用户反馈 | 行为 |
|---------|------|
| ✅ 通过 | 推进 phase → compact → 执行下一个系列 |
| ⚠️ 有问题 | 重回 Step 2，重置矫正计数 |
| ❌ 拒绝 | `state.phase = paused`，等待用户后续指令 |

---

## 三、暂离与回归协议（流程偏离防护）

**核心原则：流程可以暂离讨论，但不能偏离——任何编码动作必须先回到流程。**

### 暂离声明（AI 主动输出）

当 AI 步出流程参与讨论 / 分析时，**必须**先声明：

```
【流程暂离 — 仅讨论模式】
当前 phase: {X}  |  暂离原因: {reason}
⚠️ 本模式下不执行任何代码改动，讨论结束后说「回归流程」继续。
```

**暂离期间约束：**
- 禁止写入 src/ 源码（Write / Edit / MultiEdit 到源码路径）
- 禁止运行编译 / 测试命令（Bash 非只读操作）
- ✅ 可以读代码、解释逻辑、回答问题

### 编码意图检测（暂离期间触发）

以下意图词出现时，暂离期间**必须先回归流程**，不得直接执行：

> `改一下` / `加个` / `写代码` / `编码` / `修复` / `实现` / `提交` / `跑一下` / `试试` / `调整一下`

触发输出：

```
【主流程监管器 ❌ 阻断】当前处于讨论模式，编码前须先回归流程。
说「回归流程」执行回归检查，或 ae-sdd state read 确认当前节点。
```

### 回归门（回归时强制执行）

**触发词：** `回归流程` / `继续` / `开始做` / `接着做` / `继续上次`

回归动作（**强制，不可跳过**）：

1. `ae-sdd state read` — 确认当前 phase
2. 输出回归播报：

```
【流程回归 — 主流程监管器接管】
当前 phase: {X}  |  下一步: {next-step}  |  续接 SKILL: {skill}
```

3. 从当前节点续接，**不跳步，不重置**

---

## 四、流程偏移检测与矫正

主流程监管器自动检测流程偏移，AI 无法绕过：

| 漂移类型 | 检测条件 | 矫正级别 |
|---------|---------|---------|
| B1 跳步 | gates check 未过，但 AI 宣布进入下一系列 | L2 |
| B2 停滞 | correctionCounts[phase] ≥ 5 | L2 告警 |
| B3 伪完成 | gates check 不通过（产物核查替代自报）| L1 → L2 |
| B4 旁路 | ≥ 5 轮后无进展 | L2 → L3 |

**矫正级别：**

| 级别 | 行为 |
|------|------|
| L1 | 静默注入提示（AI 内部调整）|
| L2 | 输出 `【主流程监管器 🔴 矫正】` |
| L3 | `state.phase = paused`，等待用户介入 |

恢复：`ae-sdd state write --resume`

---

## 五、状态机子链

每个规格对应独立的 phase 序列：

| 规格 | Phase 序列 | 适用场景 |
|------|-----------|---------|
| **大(11)** | initialized -> dr-generated -> story-generated -> story-reviewed -> testcase-generated -> testcase-reviewed -> coding-process -> coding -> test-running -> code-reviewed -> completed | 已有 DR，走全流程 |
| **中(10)** | initialized -> story-generated -> story-reviewed -> testcase-generated -> testcase-reviewed -> coding-process -> coding -> test-running -> code-reviewed -> completed | 已有 Story，跳 DR |
| **小(6)** | initialized -> coding-process -> coding -> test-running -> code-reviewed -> completed | 已有 Story+TestCase，直出 CodingPlan |
| **微(6)** | initialized -> coding-process -> coding -> test-running -> code-reviewed -> completed | BUG/调整，无文档直出 CodingPlan |

---

## 六、播报格式规范

### 续接播报

```
【流程已恢复 — 主流程监管器续接】
项目 Key：{projectKey}  |  Story ID：{STORY-ID}  |  规模：{大/中/小/微}
当前阶段：{phase}  |  已完成：{列表}  |  下一步：{SKILL名}，原因：{reason}
```

### SKILL 调用声明

```
【主流程监管器 → 调用 SKILL】
调用：{skill}  |  理由：{reason}  |  期望产出：{list}  |  完成后 phase 推进至 {X}
```

### 矫正播报（L2）

```
【主流程监管器 🔴 矫正】
漂移类型：{B1/B2/B3/B4}  |  当前 phase：{X}  |  矫正次数：{N}
矫正动作：{action}
```

---

*本文档为设计规范源文档，SKILL.md 中的相关内容以本文档为设计依据。*
