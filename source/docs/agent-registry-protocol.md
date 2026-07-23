# Agent Registry Protocol — ae-sdd 子系列 Agent 注册分发协议

> **📍 定位（2026-07-23 新建）：** 本文件是 ae-sdd 子系列 Agent **预创建模板、分发注册、目标注册中心适配**的 SSOT（唯一真相源）。`agent-orchestration-skill.full.md` §3.1 定义了 5 个子系列 Agent（`ra-agent` / `dr-agent` / `story-agent` / `coding-agent` / `test-agent`），本文件定义"如何把它们作为角色模板预创建、分发时注册到各 AI coding agent 的注册中心"。
>
> **与现有 SKILL 的关系：**
> - `agent-orchestration-skill.full.md` §3.1 = 角色库 SSOT（子系列 Agent 与内含角色定义）
> - `agent-orchestration-skill.full.md` §4 = 任务分配卡 YAML 模板（运行时派活用，填内含角色名）
> - `agent-orchestration-skill.full.md` §8.7.1 = 委托契约（主流程 → 子流程Agent）
> - **本文件 = 分发层协议**：把子系列 Agent 作为模板预创建，分发时注册到 ClaudeCode / Codex / ZCode 的 agents 注册中心
>
> **本轮边界：** 只定义协议规范，不实际生成第三方 agents 目录文件（`.claude/agents/*.md` 等）。实际落盘留待后续分发任务。

---

## 1. 核心概念

### 1.1 三层 Agent 模型（对齐 §8.5 / §8.7）

| 层 | 名称 | 职责 | 生命周期 |
|---|------|------|---------|
| L1 | 主流程会话（root） | 编排、收口、用户对话、状态落盘 | 整个 ae-sdd 流程 |
| L2 | 子流程Agent（series） | 接管 1 个子系列（RA/DR/Story/Coding/Test）| 单个 series 生命周期 |
| L3 | sub-subAgent（task/reviewer） | 执行单节点 generate/review | 单节点任务 |

### 1.2 子系列 Agent = L2 层的可分发模板

本协议定义的"子系列 Agent"是 **L2 层的角色模板**，不是常驻 session。分发时：

1. 主流程会话从注册中心按子系列名（如 `coding-agent`）取模板
2. 按模板实例化一个子流程Agent（分配 session_id）
3. 子流程Agent 内部再派 L3 sub-subAgent 执行具体节点

### 1.3 与任务分配卡的区别

| 维度 | 任务分配卡（§4）| 子系列 Agent 模板（本协议）|
|------|---------------|----------------------|
| 用途 | 运行时派 1 个 L3 sub-subAgent 执行单节点 | 预创建 L2 子流程Agent 模板，分发时实例化 |
| `agent_role` 字段 | 填内含角色名（如 `coder`）| 不填此字段（模板用 `agent_name`）|
| 粒度 | 单节点单任务 | 整个子系列 |
| 载体 | 运行时 state.json `agentOrchestration` | 注册中心 agents 目录文件 |

---

## 2. 角色模板 Schema（YAML）

### 2.1 顶层结构

```yaml
# 子系列 Agent 模板
agent_name: <子系列 Agent 名>  # ra-agent / dr-agent / story-agent / coding-agent / test-agent
series_type: <§8.7.1 series_type>  # ra / dr / story / testcase / coding / test
description: <一句话职责描述>
roles:  # 内含角色（§3.1.2），向下兼容
  - <内含角色名 1>  # 如 coder
  - <内含角色名 2>  # 如 code-reviewer
default_input:  # 默认输入（实例化时可覆盖）
  - <路径或动态定位表达式>
default_standards:  # 默认必须满足的标准
  - <标准 1>
  - <标准 2>
default_deadline: <ISO8601 时长>  # 如 PT30M（30 分钟）
default_report_back:  # 默认回传配置
  channel: harness communication
  target: <root session id 占位，实例化时填>
  format: <报告模板路径>
parent_skills:  # 该子系列执行时需读的节点 SKILL
  - <SKILL 路径 1>
  - <SKILL 路径 2>
```

### 2.2 5 个子系列 Agent 模板（预定义）

#### ra-agent

```yaml
agent_name: ra-agent
series_type: ra
description: 需求分析系列 Agent — 承接 RA 起草与修订影响分析
roles:
  - ra-writer
default_input:
  - ae-sdd doc resolve --intent RA  # 路径由 document-storage-skill 动态定位
  - 项目资产（get_constraints / get_thinking_engine）
default_standards:
  - RA 5 维评分输出（大/中/小/微）
  - 边界与异常场景覆盖 ≥ 90%
  - 模糊需求必须主动提问，禁止主观臆断
default_deadline: PT30M
default_report_back:
  channel: harness communication
  target: <root session id>
  format: requirement-analysis-skill.md §RA 报告模板
parent_skills:
  - source/skill-fallbacks/skills/cross-cutting/requirement-analysis-skill.full.md
```

#### dr-agent

```yaml
agent_name: dr-agent
series_type: dr
description: 设计决策系列 Agent — DR 起草与 DR Review
roles:
  - dr-writer
  - dr-reviewer  # 评审数量由 §8.4 Tier 判定
default_input:
  - ae-sdd doc resolve --intent RA  # 上游 RA 产物
  - ae-sdd doc resolve --intent DR
default_standards:
  - DR 与 RA 一致性（业务价值对齐）
  - 架构拆分合理性
  - 跨 Story 边界清晰（Tier 3 才强制）
default_deadline: PT40M
default_report_back:
  channel: harness communication
  target: <root session id>
  format: dr-review-skill.md §DR 报告模板
parent_skills:
  - source/skill-fallbacks/skills/cross-cutting/dr-review-skill.full.md
```

#### story-agent

```yaml
agent_name: story-agent
series_type: story
description: Story 故事系列 Agent — Story/TestCase 生成与评审
roles:
  - story-writer
  - story-reviewer  # 评审数量由 §8.4 Tier 判定
  - testcase-writer
  - testcase-reviewer  # 评审数量由 §8.4 Tier 判定
default_input:
  - ae-sdd doc resolve --intent DR  # 上游 DR 产物
  - ae-sdd doc resolve --intent STORY
  - 项目资产 / 约束 / 模板路径
default_standards:
  - Story 7 阶段挖掘全覆盖
  - TestCase TC-1~TC-9 全过
  - 接口契约 / 字段 / AC / 数据模型齐
default_deadline: PT45M
default_report_back:
  channel: harness communication
  target: <root session id>
  format: story-generate-skill.md §Story 报告模板
parent_skills:
  - source/skill-fallbacks/skills/cross-cutting/story-generate-skill.full.md
  - source/skill-fallbacks/skills/cross-cutting/story-review-skill.full.md
  - source/skill-fallbacks/skills/cross-cutting/testcase-generate-skill.full.md
```

#### coding-agent

```yaml
agent_name: coding-agent
series_type: coding
description: Coding 实现系列 Agent — Task 生成 / 编码 / Code Review
roles:
  - task-writer
  - coder
  - code-reviewer  # 评审数量由 §8.4 Tier 判定（A/B/C 模式 = Tier 1/2/3）
default_input:
  - ae-sdd doc resolve --intent STORY  # 上游 Story 产物
  - ae-sdd doc resolve --intent CODINGPLAN
  - 项目资产 / 约束 / 模板路径
default_standards:
  - CodingPlan 16 章节 + 14 门禁自检
  - Code Review 7 道闸全过
  - 阻断型问题必须 0
default_deadline: PT60M
default_report_back:
  channel: harness communication
  target: <root session id>
  format: coding-skill.md §Coding 报告模板
parent_skills:
  - source/skill-fallbacks/skills/cross-cutting/task-generate-skill.full.md
  - source/skill-fallbacks/skills/cross-cutting/coding-skill.full.md
  - source/skill-fallbacks/skills/cross-cutting/code-review-skill.full.md
```

#### test-agent

```yaml
agent_name: test-agent
series_type: test
description: Test 系列 Agent — 测试运行与测试真实性验证
roles:
  - test-runner
  - test-verifier  # 🔴 强制独立 sub-session，不得与 test-runner 同 session
default_input:
  - ae-sdd doc resolve --intent CODINGPLAN  # 上游代码与测试计划
  - 测试报告路径（test-runner 产出）
default_standards:
  - L1-L4 测试层级全覆盖
  - G-09 测试真实性扫描通过
  - test-verifier TV-1~TV-10 全过
default_deadline: PT45M
default_report_back:
  channel: harness communication
  target: <root session id>
  format: test-generate-skill.md §Test 报告模板
parent_skills:
  - source/skill-fallbacks/skills/cross-cutting/test-generate-skill.full.md
```

---

## 3. 分发 SOP（注册到目标注册中心）

### 3.1 分发流程

```
主流程会话决定派某子系列（如 Coding）
    ↓
1. 查本协议 §2.2 取该子系列 Agent 模板（如 coding-agent）
    ↓
2. 按目标 AI coding agent 的注册中心适配规则（§4）生成对应文件
    ↓
3. 写入目标注册中心目录（如 .claude/agents/coding-agent.md）
    ↓
4. 主流程会话通过该 agent 的 spawn 机制实例化子流程Agent
    ↓
5. 子流程Agent 内部派 L3 sub-subAgent（填 §4 任务分配卡，agent_role 填内含角色名）
```

### 3.2 分发约束

| 约束 | 说明 |
|------|------|
| **模板为只读** | §2.2 预定义模板不可在分发时篡改 roles/standards；实例化时只覆盖 input/deadline/target |
| **内含角色名不变** | 分发文件里的角色名必须与 §3.1.2 一致（向下兼容） |
| **test-verifier 强制独立** | test-agent 分发时，test-verifier 必须用独立 session_id（对齐 §8.5.2 例外 + §8.4 物理独立性要求） |
| **不自动落盘第三方** | 本轮只定义协议；实际写 `.claude/agents/` 等目录属后续分发任务，需单独走 ae-sdd 流程 |

---

## 4. 目标注册中心适配规则

### 4.1 ClaudeCode（`.claude/agents/`）

| 项 | 规则 |
|----|------|
| 目录 | `.claude/agents/<agent_name>.md`（如 `.claude/agents/coding-agent.md`）|
| 文件格式 | Markdown + YAML frontmatter |
| frontmatter schema | `name` / `description` / `tools`（可选，工具白名单）|
| 正文 | 从本协议 §2.2 模板映射：`description` → frontmatter.description；`roles` + `default_standards` → 正文角色与标准小节 |
| 命名 | `agent_name` 原样用作文件名（如 `coding-agent`）|

**frontmatter 示例（coding-agent）：**

```yaml
---
name: coding-agent
description: Coding 实现系列 Agent — Task 生成 / 编码 / Code Review
tools:
  - Read
  - Edit
  - Write
  - Bash
  - Grep
  - Glob
---
```

### 4.2 Codex（agents 配置）

| 项 | 规则 |
|----|------|
| 载体 | Codex agent 配置（参照 Codex agents 机制，等价于 AGENTS.md 或 `.codex/agents/`）|
| 格式 | Markdown + frontmatter（字段同 ClaudeCode，Codex 复用 agents 协议）|
| 正文 | 同 ClaudeCode 映射 |
| 命名 | `agent_name` 原样 |

### 4.3 ZCode（agent 机制）

| 项 | 规则 |
|----|------|
| 载体 | ZCode agent 配置（复用 `.zcode/` 或 skills 目录机制）|
| 格式 | Markdown + frontmatter（字段同 ClaudeCode，ZCode 兼容该协议）|
| 正文 | 同 ClaudeCode 映射 |
| 命名 | `agent_name` 原样 |

> **🔴 适配通用原则：** 三家 AI coding agent 的 agents 注册中心均支持「Markdown + YAML frontmatter（name/description）」基础协议。ae-sdd 分发时统一生成该格式，frontmatter 字段三家一致；差异只在目录路径与可选工具白名单。若某家不支持 `tools` 字段，省略即可（默认继承宿主全部工具）。

---

## 5. 与 §8.7 子流程Agent 的衔接

| §8.7.1 委托契约字段 | 本协议对应 |
|--------------------|----------|
| `agent_id: spa-{uuid}` | 分发时实例化生成 |
| `series_type` | = 模板的 `series_type`（§2.2）|
| `entity_id` | = Story/DR/RA ID |
| `input` | = 模板 `default_input` + 实例化覆盖 |
| `output.deliverables` | 子流程Agent 内部 sub-subAgent 产出 |
| `session_id` | 分发时分配；test-verifier 强制独立 |

> CLI: `ae-sdd subprocess spawn --series <series_type> --entity-id <ID>`（§8.7.1 既有命令，本协议复用，不新增 CLI）。

---

## 6. 禁止事项

| # | 禁止 | 危害 | 正确做法 |
|---|------|------|---------|
| 1 | 禁止在分发时篡改 `roles` / `default_standards` | 破坏 SSOT 一致性 | 只覆盖 input/deadline/target |
| 2 | 禁止用子系列 Agent 名填 §4 任务分配卡 `agent_role` | 混淆 L2/L3 层级 | `agent_role` 填内含角色名（如 `coder`）|
| 3 | 禁止 test-verifier 与 test-runner 同 session | 破坏 §8.4 物理独立性 + ⑥.10 强制 | test-verifier 独立 session_id |
| 4 | 禁止本轮直接写第三方 agents 目录 | 越界（本轮只出协议） | 走后续分发任务流程 |

---

## 维护

- **维护人：** 架构组
- **更新频率：** 每次新增子系列 Agent 或适配新注册中心时
- **同步对象：**
  - `agent-orchestration-skill.full.md` §3.1（角色库 SSOT，本协议引用其子系列 Agent 定义）
  - `agent-orchestration-skill.full.md` §4（任务分配卡，与本协议 §1.3 区分）
  - `agent-orchestration-skill.full.md` §8.7.1（委托契约，本协议 §5 衔接）
- **关键变化（2026-07-23 新建）：**
  - 🆕 5 个子系列 Agent 预定义模板（§2.2）
  - 🆕 角色模板 YAML schema（§2.1）
  - 🆕 分发 SOP（§3）
  - 🆕 3 目标注册中心适配规则（§4：ClaudeCode / Codex / ZCode）
