---
name: ae-sdd
version: 3.9.0
description: |
  端到端自动化工程主入口（v3.9.0）。从 DR/PRD 出发，经 RA→DR→Story→TestCase→Task→Coding→Test，直到全部通过。
  支持大/中/小/微四条子链（按已有产物就近入链）、流程状态跟踪、中断恢复、主流程监管器（产物核查+偏移检测+暂离回归协议）。
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

## 🎛️ 主流程监管器执行协议

每系列标准 4 步。**监管器只编排+校验，不执行具体业务。**

| 步骤 | 动作 |
|------|------|
| 1 | `/compact`（系列入口，防跨系列上下文污染）；输出 SKILL 调用声明；写 events 日志 `ae-sdd state write --event skill-launched` |
| 2 | 加载 `{series}-generate-skill.md` → 调 `agent-orchestration-skill`（按任务量分配子 Agent workflow）→ 产物核查 |
| 3 | 加载 `{series}-review-skill.md` → 调 `agent-orchestration-skill`（分配子 Agent）→ 汇总错误报告给监管器；**Loop**：有错+矫正<3 → 回步骤2；矫正=3 → Level 3 暂停等用户；连续3轮无新错 → 步骤4 |
| 4 | **默认模式**：人工审核：播报产物摘要；等用户 ✅/⚠️/❌ → ✅推进phase→下一系列；⚠️→重回步骤2+重置计数；❌→paused<br>**🆕 v3.8.0 自动化模式**：联审共识 — 强制 Tier 3 派 3 个独立 session reviewer（视角正交，见 `agent-orchestration-skill §8.4`）→ 跑 §8.4.3 交叉对比 → `ae-sdd state register-review-consensus` 写 `reviewConsensus[point]` → G-09B + G-REVIEW-LOOP + G-AUTO-CONSENSUS 全过即自动推进 phase；3 轮矫正未决 → L3 `state.phase=paused`（按 `automation.onConsensusStall`） |

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
| **微任务/BUG/配置类豁免** | 走 task-generate-skill 从 Task 系列入（含轻量 CodingPlan）|

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

联审 3 轮矫正未决 → 按 `onConsensusStall`：
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
| 联审不通过仍推进 phase | G-AUTO-CONSENSUS 阻断；3 轮未决 → paused |

---

## 🔴 输出核心原则（最高优先级）

| 原则 | 要求 |
|------|------|
| 基于事实 | 所有输出必须有明确来源（DR/PRD/资产/用户告知/代码读取）|
| 禁止猜测 | 不确定 → 标 `{待确认}` 并主动询问；禁止推断后输出 |
| 禁止杜撰 | 不得编造不存在于输入材料中的规则/字段/类名/配置项 |
| 🆕 禁止文档承载 changelog | 设计/架构/模板/标准类文档只写**当前生效内容**；历史变更走 `source/CHANGELOG/{YYYY-MM-DD}-{主题}.md`，文档内仅用一句引用（如「详见 CHANGELOG/...」）指向。混写会破坏主题连续性、加剧检索劣化、让 git blame 失真——属于坏习惯，不是细节。 |

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

**调用顺序：** ① 自更新识别 → ② 任务类型（编码） → ③ 规格裁定 → ④ G-RA 门禁（大任务时）

> 🆕 v3.9.0 **路由自动 state 匹配**：路由时 `classify.match_state()` 自动分析需求特征（提取 PRD/DR/Story ID + 判定 Bug/改 Story）→ 扫描现有嵌套 state → 命中则 relocate/absorb，未命中则 create_nested。匹配优先级：
> 1. R4 Bug/微任务不改 Story → `create_flat`
> 2. R5 Story 命中现有 state → `relocate` + 重置子状态（若下游已动）
> 3. R2 DR 归入已存在的 PRD state → `absorb_into_prd`
> 4. R2 Story 归入已存在的 DR state → `absorb_into_dr`
> 5. R7 无匹配 → `create_nested`（以当前主体为顶层，R6 顶层命名）

### 路由表（编码类）

| 规格 | 已有产物 / 场景 | 入口系列 | G-RA |
|------|----------------|---------|------|
| **大** | 已有 PRD | `requirement-analysis-skill.md`（RA 系列）| 不需要（入口本身）|
| **中** | 已有 DR | `dr-generate-skill.md`（DR 系列）| **必过** |
| **小** | 已有 Story | `story-generate-skill.md`（Story 系列）| **必过** |
| **微** | BUG / 改逻辑 / 调整代码 | `task-generate-skill.md`（Task 系列）| 豁免 |
| — | 新需求无任何产物且非BUG | 🔴 阻断 | — |

**非编码类路由：**

| 输入/场景 | 路由 |
|----------|------|
| 修改SKILL / 优化ae-sdd | `ae-sdd-update-skill.md`（监管器全权交接）|
| 安装/升级ae-sdd | `ae-sdd-install-skill.md` |
| 修改/BUG/生产故障 proposal | `proposal-skill.md` |
| 放文档哪里 / 命名 | `document-storage-skill.md` + G-DOC-STORAGE |
| 从X继续 / 重入 | 读 state.json 判重入点再路由 |

### 4类规格判定

| 规格 | 判定依据 | 入口节点 |
|------|---------|---------|
| **大** | 已有 PRD | RA 系列 |
| **中** | 已有 DR（无PRD或PRD已完成）| DR 系列 |
| **小** | 已有 Story（无DR）| Story 系列 |
| **微** | BUG / 改逻辑 / 调整代码（无完整产物链）| Task 系列 |

### 状态机子链（实际 state.json phase 值）

| scale | phase 序列 | 适用 |
|-------|-----------|------|
| 大(14) | initialized→ra-generated→dr-generated→story-generated→story-reviewed→testcase-generated→testcase-reviewed→task-generated→task-reviewed→coding-process→coding→test-running→code-reviewed→completed | 有PRD，走全流程 |
| 中(13) | initialized→dr-generated→story-generated→story-reviewed→testcase-generated→testcase-reviewed→task-generated→task-reviewed→coding-process→coding→test-running→code-reviewed→completed | 有DR，跳RA |
| 小(12) | initialized→story-generated→story-reviewed→testcase-generated→testcase-reviewed→task-generated→task-reviewed→coding-process→coding→test-running→code-reviewed→completed | 有Story，跳RA+DR |
| 微(7) | initialized→task-generated→task-reviewed→coding-process→coding→test-running→completed | BUG/调整，跳RA+DR+Story+TestCase |

---

## 📖 人工审核主动讲解规范

**双支柱（缺一不可）：** ① 先讲故事（叙述性，讲清背景/意图）+ ② 对话内直接呈现（结构化，用户无需开文件即可审核）

| 审核节点 | 讲解主体 | 模板位置 |
|---------|---------|---------|
| Story Review | 业务设计故事 | `story-review-skill.md §📖` |
| Task Generate | 实现拆解故事 | `task-generate-skill.md §📖` |
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
  ③bis TestCase Review（testcase-review-skill，TC-1~TC-9循环，3轮无新增退出）
  ③ter 业务逻辑汇总
  🔍 审核点1：设计完成确认 + context-pressure

Phase 2 实现阶段
  🔍 审核点1.5：实现方案预确认 + context-pressure
  ④ Task生成（task-generate-skill）+ 全局Task Review（TR-1~TR-7）
  ④bis CodingProcess（coding-skill能力库）：CodeAnalysis→CodingPlan→Execute
        state.phase=coding-process；G-CODEPLAN-SRC + G-14 通过后才能 Execute
  🔍 审核点2：Task文档逐文件核对 + context-pressure
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

**state.json** 路径：`.auto-engineering/{WORKITEM-ID}/state.json`

关键字段：`scale` / `entryNode` / `phase` / `currentWorkItem` / `currentStory` / `completedSteps` / `correctionCounts`

**再启动判定**：

| 用户意图 | AI动作 |
|---------|--------|
| 继续/接着做 | 读 state.json，恢复当前步骤 |
| 从某步骤继续 | 从指定步骤恢复（不可倒退已完成关键门禁）|
| 放弃重新开始 | 确认后删 state.json，从 Phase 1 ① 重启 |
| 切换WorkItem/Story | 保留当前 state.json，切换 `.auto-engineering/{WORKITEM-ID}/state.json` |
| 偏离流程 | 简短回应，不更新 state |
| PRD收尾 | 跑 prd-check-complete → 4层AND全过 → 审核点5 → prd-complete |
| 🆕 v3.9.0 改已管理 Story | `ae-sdd state relocate --story <ID>` 重定位到所属 state + 只重置该 Story 子状态到 story-generated（R5） |

**paused 恢复**：输出原因 + pausedFromPhase，三选一：恢复/新任务/查看状态

---

## Phase 1 关键节点

### ①bis 前端视角接口审视
6维度（契约完整性/调用流程/状态展示/错误码/边界场景/联调支持）→ 详细清单见 `story-review-skill.md §📋 ①bis`

AE编排层门禁：① 完成后必做 ①bis；② Story含"接口契约"章节（v3.9.5 起合并，含 ①bis 6 维度）；③ Story Review含F-Stage

### ② Story Review
循环：挖掘→判定→Proposal→按Proposal修复→再挖掘→退出（连续3轮无新增）

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
| Task Generate | `task-generate-skill.md` | Task生成+全局Review |
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
| 2b | ①bis 前端视角接口审视 | 6维度通过；Story含"接口契约"章节（含①bis 6维度） |
| 3 | Story Review（含F-Stage）| 循环退出（3轮无新增）|
| 4 | 生成测试用例（testcase-generate-skill）| 文件已生成+合规校验通过 |
| 4a | TestCase Review（testcase-review-skill，TC-1~TC-9）| 循环退出（3轮无新增）|
| 4-📖 | AI主动讲解Story故事 | 5维度讲清才进审核点1 |
| 4b | 审核点1（设计阶段完成确认）| 用户✅；context-pressure |
| 4c | 审核点1.5（实现方案预确认）| 用户✅；context-pressure |
| 5 | 生成Task（task-generate-skill）| 全部Task文件已生成 |
| 5a | 全局Task Review（TR-1~TR-7）| 3轮无新增才退出 |
| 5b-📖 | AI主动讲解Task故事 | 5维度讲清才进审核点2 |
| 5b | 审核点2（Task逐文件核对）| 每文件单独✅；context-pressure |
| 5c | ④bis CodingProcess（CodeAnalysis→CodingPlan）| G-CODEPLAN-SRC+G-14通过 |
| 5b.5 | 审核点2.5（CodingPlan评审）| 14条门禁全过+用户✅；context-pressure |
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
