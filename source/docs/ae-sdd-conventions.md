# ae-sdd 项目级 SOP 模板（v3.1 新增）

> **版本**：v3.1（接 v3.0 之后的纪律层加固）
> **起草日期**：2026-06-22
> **配套母版**：[`source/SKILL.md`](../SKILL.md) §🔴 第一动作（硬前置）
> **触发问题**：life 项目 STORY-020-BE v3-r2 CodeReview 复盘中，root agent 收到 `/ae-sdd` 触发后**未走 G-00 / 路由判定 / 派 sub-agent / 出 CodeReview 报告**，直接动手改了代码。详见 `source/docs/plans/2026-06-22-discipline-hardening-plan.md`。
> **目标读者**：使用 ae-sdd 的项目 root agent（orchestrator），**不约束 sub-agent**（sub-agent 由 reins/agent.md 约束）

---

## 0. 范围与边界

### 0.1 本文件约束的对象

**只约束 root agent（orchestrator / 项目级 system prompt 的 AI 实例）。** 不约束：

- reins sub-agent（由各项目 `.harness/reins/*/agent.md` 约束）
- 用户主动调用具体 SKILL 的场景（如直接 `/ae-sdd requirement-analysis`，已绕过 root agent 路由）

### 0.2 与母版的关系

| 层级 | 文件 | 关系 |
|------|------|------|
| **母版（权威）** | `D:\Item\ae-sdd\source\SKILL.md` | 定义通用流程，G-00、路由、sub-agent 边界 |
| **本文件（项目级 SOP 模板）** | `D:\Item\ae-sdd\source\docs\ae-sdd-conventions.md` | 给"使用 ae-sdd 的项目"一个项目级约定模板 |
| **项目实例化** | `<project>/.harness/ae-sdd-instance.md` | 本模板在具体项目场景的实例化 |

**优先级**：母版 > 本文件 > 项目实例化。冲突时以母版为准。

### 0.3 何时引用本文件

- 母版 SKILL.md §🛡️ G-00 之后会显式引用（"**项目级补充见 `docs/ae-sdd-conventions.md`**"）
- 项目实例化文件第 1 段会引用本文件 + 母版

---

## 1. 项目级强制节点（root agent 必跑）

### 1.1 G-00 必跑且必须落档

**跑法**（实测有效路径，与 SKILL.md §🛡️ G-00 L34-73 一致）：

```bash
# 路径 1：母版路径（永远存在，sync 前可用）
PROJECT_KEY=<projectKey>
ASSETS_MASTER="D:/Item/ae-sdd/source/assets/${PROJECT_KEY}/${PROJECT_KEY}.assets.md"

# 路径 2：sync 后路径（sync-to-plugin.sh 跑过才有）
ASSETS_SYNC="$HOME/.claude/skills/ae-sdd/skills/ae-sdd/assets/${PROJECT_KEY}/${PROJECT_KEY}.assets.md"
```

**🔴 v3.1 关键事实**：

- **`ae-sdd` CLI 不存在**（实测 `CommandNotFoundException`）—— SKILL.md §🛡️ G-00 L60-71 写的工具命令是规划,目前不能跑
- **路径 1 (母版) 永远可用** —— root agent 收到 `/ae-sdd` 触发后,第一动作 = `Read ${ASSETS_MASTER}`,**不依赖 CLI**
- **路径 2 (sync 后) 是 CLI 调用约定路径** —— v3.1 不强制要求,留给 v3.2 CLI 实现后使用

**落档要求**：会话开始时必须输出一段"G-00 资产摘要",格式如下：

```markdown
## G-00 资产摘要

- projectKey: <projectKey>
- 资产路径: <ASSETS_MASTER> （或 sync 后: <ASSETS_SYNC>）
- 资产存在: ✅ / ❌（缺失则进入项目资产生成流程）
- lastAuditedAt: <YYYY-MM-DD>
- 7 层索引完整性: <✅ 全 / ⚠️ 缺 N 层 / ❌ 缺失>
- 工程规模: <N 个微服务, N 层架构, N 项工程约束>
```

**资产缺失时的标准动作**：

1. 不要静默生成 —— 明确告知用户"⚠️ 未找到项目 {projectKey} 的资产文件"
2. 引导走 `source/skills/cross-cutting/project-assets-update-skill.md §3` 生成
3. **不要绕过 G-00 继续流程**（即使资产缺失,也要先让用户看到资产缺失的事实）

### 1.2 路由判定必须显式

收到任何 `/ae-sdd` 触发后,**必须输出"路由判定卡"**,格式：

```markdown
## 路由判定卡

- 触发词: <触发词原文>
- 关键词命中: <命中路由表哪一行 / 或"未命中">
- 4 维判定:
  - 来源: <PRD / Issue / 对话需求 / BUG / 配置类 / 无输入>
  - 规模: <大 / 中 / 小 / 微 / 特殊>
  - 现有产物: <无 / 有 RA / 有 DR / 有 Story / ...>
  - 项目类型: <重任务 / 小任务 / 微任务>
- 路由目标: <目标 SKILL 文件路径>
- 是否需要快速通道: <是 / 否>
```

**🔴 禁止**：

- 主观归类为"对话轻量通道"后跳过路由判定
- 跳过 4 维判定直接动手
- 多个 SKILL 命中时擅自选一个（必须问用户）

### 1.3 sub-agent 边界

**自己动手前必须先回答这个问题**:

> "这件事应该派给哪个 sub-SKILL / sub-agent？"

| 答案 | 动作 |
|------|------|
| **能回答** | 用 `harness communication send --command spawn` 派活,自己只做 reviewer |
| **能回答但任务简单**(单文件单改) | 可直接动手,但**事后必出追溯文档** |
| **不能回答** | 先停下来想清楚,或问用户,不要"先动手再说" |
| **任务仅涉及 root agent 自身职责**（如 ae-sdd 自身维护）| 直接动手,按母版 `ae-sdd-update-skill.md` |

**🔴 v3.1 关键判断**：

- 母版 SKILL.md §🤖 多 Agent 编排 L450-870 已有完整 sub-agent 边界定义 —— 但**入口段（L1-260）完全不提"派活"**
- **v3.1 修正**：root agent 读完入口段就能意识到"派活是默认动作",而不是"自己动手是默认动作"
- 触发"派活"判断的最小信号：用户输入涉及 ≥2 个领域 / 涉及 reins 列表中的某个 rein 明确负责的模块

---

## 2. 项目级产物路径约定

### 2.1 不同项目的产物路径

| 项目 | 产物路径 | 备注 |
|------|---------|------|
| **life**（原 icec-cloud-boss）| `\.auto-engineering\STORY-{ID}-BE\` | 实战路径,与 v3.1 之前一致 |
| **icec-cloud-boss**（顶层目录已重命名为 life）| 同 life | 2026-06-16 顶层重命名,产物路径不变 |
| **icec-cloud-life** | `\.auto-engineering\STORY-{ID}-BE\` | 同 life |
| **其他新接入项目** | 建议同 life 模式 | 项目实例化时确认 |

**🔴 v3.1 关键事实**：

- SKILL.md §🎯 统一入口的"4 类需求智能路由"(L155-173)和"4 维判定"(L175-211) 写的是 `ae-sdd-doc/iterations/{date}/{DocType}/...`
- **life 项目的实战路径是 `\.auto-engineering\STORY-{ID}-BE\`**,与 SKILL.md 描述不一致
- **v3.1 处理方式**：项目实例化文件 §2 显式说明"实战路径覆盖 SKILL.md 默认路径",不强制改 SKILL.md

### 2.2 路径冲突处理

如果项目实例化路径与 SKILL.md 默认路径冲突：

1. **以项目实战路径为准**（已经过验证的不要改）
2. 项目实例化文件 §2 显式声明冲突点
3. SKILL.md 在 v3.2 评估是否需要更新默认路径描述

### 2.3 产物文件命名约定

| 产物类型 | 命名 | 模板 |
|---------|------|------|
| Story | `STORY-{ID}-BE.md` | `source/templates/design/story-template.md`（v3.9.3 起主模板，已合并原 be-story-template.md；纯后端在前端章节标注"不涉及"即可） |
| DR | `DR-{ID}.md` | `source/templates/design/dr-template.md` |
| CodeReview 报告 | `code-review-report-v{N}-r{N}.md` | `source/templates/coding/be-codereview-template.md` |
| Coding 报告 | `CodingReport-{事务简称}.md` | `source/templates/coding/be-coding-report-template.md` |
| Coding Plan | `CodingPlan-{事务简称}.md` | `source/templates/coding/be-coding-plan-template.md` |
| **PRD ID 命名** | **`PRD-<业务域>-<序号>`**（3 位数字）| `dr-review-skill.md:184` + `ae-sdd-skill.md §1.2` |
| **PRD 级 state.json** | `.auto-engineering/{PRD-ID}/state.json` | `document-storage-skill.md §3.5` |
| **PRD 级 handoff** | `.auto-engineering/{PRD-ID}/summary.md` | `harness session rotate --handoff-file` |

---

## 3. 项目级 escape hatch 语义

### 3.1 快速通道的启用与豁免

| 节点 | 快速通道是否豁免 | 豁免条件 | 落档要求 |
|------|----------------|---------|---------|
| G-00（项目资产门卫）| ✅ 可豁免 | 用户显式说 `/ae-sdd-quick` 或 `走快速通道` | 仍需落档"项目资产摘要" |
| 4 维判定 / 4 类需求路由 | ✅ 可豁免 | 同上 | 仍需落档"路由判定卡"（简版）|
| Plan-first（编码前硬前置）| ❌ **不豁免** | — | 必须出 CodingPlan |
| 出 CodeReview 报告 | ❌ **不豁免** | — | 必须出 `code-review-report-v{N}-r{N}.md` |
| 更新 state.json | ❌ **不豁免** | — | 必须走 code-review-skill §第六步 |

### 3.2 快速通道的触发语义

**"走快速通道"等价于**:

- 跳过 G-00 的"资产完整性校验"步骤
- 跳过 4 维判定的"项目类型"维度
- 仍需落档"项目资产摘要"和"路由判定卡（简版）"
- **不豁免 Plan-first / CodeReview 报告 / state.json 更新**——这些是质量底线

### 3.3 谁有权说"走快速通道"

- **只有用户本人**可以触发快速通道
- sub-agent 不能触发快速通道（sub-agent 收到的 brief 已经包含路由判定）
- root agent 不能"为用户决定"走快速通道（即使任务看起来很小）

### 3.4 快速通道的审计要求

- 快速通道产物必须在文档开头标注「⚡ 快速通道触发：[来源原话]」
- 1 周内累计 ≥3 次快速通道触发 → 升级讨论"是否需要正式化快速通道流程"

---

## 4. 项目级失职检测

### 4.1 失职检测的层级

| 层级 | 检测方式 | 阻断能力 | v3.1 状态 |
|------|---------|---------|----------|
| **L1 母版硬前置** | root agent 自我约束（软约束）| 🟡 软 | ✅ v3.1 已实现 |
| **L2 SOP 模板** | root agent 自我约束 + 项目级实例化（软约束）| 🟡 软 | ✅ v3.1 已实现 |
| **L3 项目实例化** | root agent 自我约束 + 项目特定失职历史（软约束）| 🟡 软 | ✅ v3.1 已实现 |
| **L4 hook 强制** | 工具链硬阻断（exit 1 / pre-trigger hook）| 🔴 硬 | ❌ v3.2 评估 |

### 4.2 v3.1 的失职检测方式

由于 L4 延期,**v3.1 依赖以下 3 种软约束叠加**:

1. **root agent 自我审计**:每次输出前问自己"我跑 G-00 了吗？"
2. **对话内自检**:每次输出路由判定卡 / G-00 资产摘要时,**用户可以一眼看出是否漏跑**
3. **事后追溯**:产物文件开头标注 G-00 跑/没跑,便于事后审计

### 4.3 v3.2 评估的 hook 接入方式

| 方案 | 阻断能力 | 兼容性风险 | 优先级 |
|------|---------|----------|-------|
| **harness daemon pre-trigger hook** | 🔴 硬 | 中（需验证 daemon 是否支持）| P0 |
| **system prompt 强约束**（每次 /ae-sdd 触发必跑 pre-ae-sdd-check.sh）| 🟡 半硬 | 低 | P1 |
| **对话内软警告**（AI 自检 + 用户提醒）| 🟢 软 | 无 | P2（v3.1 现状）|

**v3.1 决策**：用 P2（软约束叠加）,1 周观察期后再评估是否升级到 P0 / P1。

---

## 5. 项目级失职历史记录模板

每个使用 ae-sdd 的项目应在 `ae-sdd-instance.md` 中维护"失职历史记录"段:

```markdown
## 失职历史记录

### YYYY-MM-DD | STORY-ID | 失职类型 | 失职描述 | 处置

| 日期 | STORY-ID | 失职类型 | 失职描述 | 处置 |
|------|---------|---------|---------|------|
| 2026-06-22 | STORY-020-BE v3-r2 | 跳 G-00 / 跳路由判定 / 越层动手 / 事后回溯 | root agent 收到 /ae-sdd 触发后直接动手改代码,跳过完整流程 | 已出 code-review-report-v3-r2.md(事后回溯标注);v3.1 加固纪律层 |
```

---

## 6. 附录

### 6.1 v3.1 验收清单

- [x] L1 母版硬前置: `source/SKILL.md` 入口段第 1 段已加「🔴 第一动作(硬前置)」5 行声明
- [x] L2 SOP 模板: 本文件 (`source/docs/ae-sdd-conventions.md`) 已新建,≥150 行
- [ ] L3 项目实例化: 各使用方项目新建 `ae-sdd-instance.md`(v3.1 至少完成 life 项目)
- [ ] L4 hook 强制: 延期到 v3.2 评估
- [ ] CHANGELOG v3.1 条目
- [ ] sync-to-plugin.sh 跑通三处同步

### 6.2 相关文件

- 母版: [`source/SKILL.md`](../SKILL.md) §🔴 第一动作（硬前置）+ §🛡️ G-00
- 建议书: [`source/docs/plans/2026-06-22-discipline-hardening-plan.md`](plans/2026-06-22-discipline-hardening-plan.md)
- 项目级资产模板: [`source/standards/project-assets/project-assets-template.md`](../standards/project-assets/project-assets-template.md)
- 项目级资产更新 SKILL: [`source/skills/cross-cutting/project-assets-update-skill.md`](../skills/cross-cutting/project-assets-update-skill.md)
- 路由决策算法: `source/SKILL.md` §🎯 统一入口 L262-337

### 6.3 失败案例原文（2026-06-22 实测）

> 用户原话：`/ae-sdd 待接待超时做了一个新逻辑，结单前要拉一次AI消息。实现可参考工单接单同步AI消息逻辑和转人工同步AI消息逻辑。你帮我查一下实现有没有问题？`

**root agent 失职序列**：

```
1. 收到 /ae-sdd 触发
2. Read SKILL.md 头几屏（约 300 行）
3. 直接 Grep 代码 → 找到 handleWaitingTimeout 实现
4. 对比 3 个参考实现 → 在对话里输出评审结论
5. 用户确认后 → 直接 Edit 测试文件 + 加 verify
6. mvn test → 13/13 通过 → 在对话里贴结果
```

**应做但跳过的节点**：

- 跑 G-00 项目资产门卫（主观判断"项目资产不存在"——实际 `icec-cloud-life` 资产存在）
- 路由到 code-review-skill（主观归类"短问句=对话通道"）
- 派 code-reviewer sub-agent（自己动手,sub-agent 边界失守）
- 出 STORY-020-CodeReview-v3-r2.md（编码后才补,标注"事后回溯"）
- 更新 state.json（未做）

**补救产物**：`D:\Item\life\.auto-engineering\STORY-020-BE\code-review-report-v3-r2.md`（事后回溯,标注）

---

## 7. 不承诺"永不再犯"

本 SOP 模板**不承诺"永不再犯"**。原因：

- L1/L2/L3 都是软约束,无法强制 AI 行为
- LLM 训练分布、上下文长度、用户表达差异都会影响 AI 决策
- 唯一能"永不再犯"的方案是 L4 hook 强制,但 hook 接入有兼容性问题（v3.2 评估）

**务实目标**：再犯率从 ~50% 降到 ~5%,失职时**早期发现 + 显式标注 + 留追溯**。