# ae-sdd 重组为"规则层 + 工具层 + 主入口"三段式架构 — Plan

> **起草日期**：2026-06-18
> **目标版本**：ae-sdd v3.0
> **起草人**：Mavis（基于 2026-06-18 批判盘点 + 用户两轮提问）
> **状态**：待评审

---

## 0. TL;DR

把 ae-sdd 从"哲学完备、工程半成品"重组为：

| 层 | 路径 | 角色 | 形态 |
|----|------|------|------|
| **主入口** | `SKILL.md` | 触发即加载 | ≤ 300 行，路由 + 工具速查 |
| **规则层** | `rules/` | AI 推理依据 | 散文 SKILL.md + 声明式 `rules.yaml` |
| **工具层** | `tools/` | 机器可执行 | 8 个 CLI 子命令 + JSON Schema |

**关键约束：单一事实源 (SSOT)。** 规则改了 → 工具必须可同步重生成。

---

## 1. 现状盘点（Plan 起点）

### 1.1 关键发现（2026-06-18 调研结果）

| 发现 | 证据 |
|------|------|
| **SKILL.md 主入口已存在但名存实亡** | 仓库根 `SKILL.md` = 1843 行（110KB），与 `skills/orchestration/ae-sdd-skill.md` 字节级相同 |
| **主入口违反单一职责** | `SKILL.md` 含 6 个独立知识域（路由 / 4 类需求 / 9 步流程 / 状态机 / 角色库 / CodingModel 11 维）|
| **18 个 SKILL 文件散在 6 个 phase 目录** | phase1-design(8) / phase2-coding(2) / phase2-task(1) / phase3-review(1) / cross-cutting(4) / orchestration(2) |
| **scripts/ 实际 6 个文件**（不是之前估的 4 个）| `sync-to-plugin.sh` `dev-sync.sh` `install.ps1` `install.sh` `migrate-docs.mjs` `test_authenticity_scan.py` |
| **test_authenticity_scan.py 是非平凡工具** | 14.5KB，对应 `coding-skill.md §测试真实性` 的 8 类禁止 |
| **docs/ 目录存在但空** | 没有 plans/ 子目录可参考 |

### 1.2 现状目录树

```
D:\Item\ae-sdd\
├── SKILL.md              ← 1843 行（与 ae-sdd-skill.md 重复，主入口未独立）
├── README.md
├── CHANGELOG/
├── docs/                 ← 空
├── skills/
│   ├── orchestration/    (ae-sdd-skill, ae-sdd-update-skill)
│   ├── phase1-design/    (dr-generate, dr-review, dr-update, requirement-analysis, story-generate, story-review, story-update, testcase-generate)
│   ├── phase2-coding/    (coding-skill, coding-report-skill)
│   ├── phase2-task/      (task-generate-skill)
│   ├── phase3-review/    (code-review-skill)
│   └── cross-cutting/    (agent-orchestration, document-storage, project-assets-update, proposal)
├── scripts/              (6 文件)
├── standards/            (9 约束)
├── templates/
├── assets/               (项目特定数据)
├── plugins/ae-sdd/       (副本)
├── .claude-plugin/
├── .git/
├── .idea/                ← IDE 配置
└── logs/
```

### 1.3 三大根因

1. **规则与工具未分区** → 工具跟不上规则膨胀（18 SKILL ↔ 6 scripts）
2. **规则是散文非声明式** → AI 解读歧义（"5 维"在 3 个 SKILL 指 3 件不同事）
3. **状态机/门禁靠 AI 推理** → 没有可执行的确定性工具

---

## 2. 目标架构

### 2.1 三段式

```
┌─────────────────────────────────────────────────────────────┐
│  SKILL.md  (主入口)                                         │
│  - YAML frontmatter (name / description / triggers)         │
│  - §0 是什么 / 不是什么                                     │
│  - §1 三条路径（Quick / Standard / Cross-Cutting）          │
│  - §2 工具 API 速查                                         │
│  - §3 子 SKILL 索引（仅链接，不含内容）                     │
│  - §4 状态机约定（指向 rules/orchestration/）                │
│  - §5 文档与资产约定                                        │
│  - §6 标准与约束                                            │
│  - §7 何时不用                                              │
│  - §8 维护规则（修改 rules.yaml 后必跑 sync-tools）         │
│  ≤ 300 行                                                   │
└─────────────────────────────────────────────────────────────┘
            │ 加载
            ▼
┌──────────────────────────┐  ┌──────────────────────────────┐
│  rules/  (规则层)         │  │  tools/  (工具层)             │
│  - 散文 SKILL.md          │  │  - CLI: ae-sdd <cmd>         │
│  - 声明式 rules.yaml      │◄─┤  - lib/*.mjs 实现            │
│  - 6 phase × 概念单元     │  │  - schemas/*.json            │
│  - SSOT 入口              │──►  - sync-tools 从 yaml 生成   │
└──────────────────────────┘  └──────────────────────────────┘
            │                              │
            └───── ae-sdd sync-tools ─────┘
                   (单一事实源保证)
```

### 2.2 三条路径入口

| 路径 | 触发词 | 工具入口 | 流程深度 |
|------|--------|---------|---------|
| ⚡ **Quick Mode** | "改个枚举值" / "改个常量" / "重命名" | `ae-sdd quick <task>` | 跳 DR/Story/Task |
| 🔵 **Standard Mode** | "做 XX 功能" / Story 重入 | `ae-sdd run <story-id>` | 完整 9 步 |
| 🟣 **Cross-Cutting** | "重构 XX 域" / "改 2 个 bounded context" | `ae-sdd proposal <scope>` | 走 proposal-skill |

---

## 3. 分区设计

### 3.1 目标目录树

```
D:\Item\ae-sdd\
├── SKILL.md                    ← 🆕 重写为薄入口（≤ 300 行）
├── README.md                   ← 改为"项目概览 + 快速开始"
├── CHANGELOG/
├── docs/
│   ├── plans/                  ← 🆕 Plan 文档归档
│   └── adr/                    ← 🆕 架构决策记录（可选）
├── rules/                      ← 🆕 重组自 skills/
│   ├── orchestration/
│   │   ├── SKILL.md            (原 ae-sdd-skill.md 全文迁入)
│   │   ├── ae-sdd-update-SKILL.md
│   │   └── rules.yaml          ← 🆕 状态机/路由/分类声明
│   ├── phase1-design/
│   │   ├── dr-generate-SKILL.md
│   │   ├── dr-review-SKILL.md
│   │   ├── dr-update-SKILL.md
│   │   ├── requirement-analysis-SKILL.md
│   │   ├── story-generate-SKILL.md
│   │   ├── story-review-SKILL.md
│   │   ├── story-update-SKILL.md
│   │   ├── testcase-generate-SKILL.md
│   │   └── rules.yaml          ← 🆕 7 区模板 / 评审 rubric 声明
│   ├── phase2-task/
│   │   ├── task-generate-SKILL.md
│   │   └── rules.yaml          ← 🆕 5 维规模 / 11 维决策声明
│   ├── phase2-coding/
│   │   ├── coding-SKILL.md
│   │   ├── coding-report-SKILL.md
│   │   └── rules.yaml          ← 🆕 14 门禁 / 测试真实性 8 类禁止声明
│   ├── phase3-review/
│   │   ├── code-review-SKILL.md
│   │   └── rules.yaml          ← 🆕 TR-1~TR-7 声明
│   └── cross-cutting/
│       ├── agent-orchestration-SKILL.md
│       ├── document-storage-SKILL.md
│       ├── project-assets-update-SKILL.md
│       ├── proposal-SKILL.md
│       └── rules.yaml          ← 🆕 文档存放 / 资产路径声明
├── tools/                      ← 🆕 重组自 scripts/，扩充
│   ├── bin/
│   │   ├── ae-sdd              ← 🆕 主 CLI（Node.js ESM）
│   │   ├── ae-sdd-dev          ← 🆕 开发期 CLI（dev-sync）
│   │   └── ae-sdd-install      ← 🆕 安装期 CLI（install.ps1/sh 的 Node 版）
│   ├── lib/
│   │   ├── state.mjs           ← 🆕 状态机
│   │   ├── classify.mjs        ← 🆕 4 类需求识别
│   │   ├── route.mjs           ← 🆕 路由
│   │   ├── gates.mjs           ← 🆕 14 门禁扫描
│   │   ├── review.mjs          ← 🆕 TR-1~TR-7 扫描
│   │   ├── health.mjs          ← 🆕 健康度 9 项
│   │   ├── diff.mjs            ← 🆕 版本对比
│   │   ├── init.mjs            ← 🆕 项目脚手架
│   │   ├── quick.mjs           ← 🆕 Quick Mode 入口
│   │   ├── sync.mjs            ← 🆕 规则→工具同步
│   │   ├── project-assets-scan.mjs  ← 移植自 project-assets-update-skill
│   │   ├── test-authenticity.mjs    ← 移植自 test_authenticity_scan.py
│   │   └── migrate-docs.mjs         ← 迁移自 scripts/
│   ├── schemas/
│   │   ├── state.schema.json   ← 状态机 JSON Schema
│   │   ├── rules.schema.json   ← rules.yaml JSON Schema
│   │   ├── classify.schema.json
│   │   └── gates.schema.json
│   ├── tests/                  ← 🆕 Node 内置 test runner
│   │   ├── state.test.mjs
│   │   ├── classify.test.mjs
│   │   ├── gates.test.mjs
│   │   ├── health.test.mjs
│   │   └── sync.test.mjs
│   └── README.md               ← 🆕 工具开发指南
├── standards/                  (保留，9 约束)
├── templates/                  (保留)
├── assets/                     (保留)
├── plugins/ae-sdd/             (保留，sync-to-plugin.sh 同步副本)
├── .claude-plugin/             (保留)
├── .git/
├── .idea/                      ← 建议 .gitignore 强化
├── logs/
├── .gitignore
├── package.json                ← 🆕 标记 tools/bin 为 bin 入口
└── sync-to-plugin.sh           (保留，更新路径)
```

### 3.2 迁移策略

| 旧路径 | 新路径 | 动作 |
|--------|--------|------|
| `skills/**/*.md` | `rules/**/*.md` | git mv |
| `scripts/sync-to-plugin.sh` | `sync-to-plugin.sh` | 保留在根，更新引用 |
| `scripts/dev-sync.sh` | `tools/bin/ae-sdd-dev` | 改写为 Node CLI |
| `scripts/install.{ps1,sh}` | `tools/bin/ae-sdd-install` | 改写为 Node CLI（跨平台）|
| `scripts/migrate-docs.mjs` | `tools/lib/migrate-docs.mjs` | git mv |
| `scripts/test_authenticity_scan.py` | `tools/lib/test-authenticity.mjs` | **移植到 Node**（统一栈）|

**旧 `skills/` 和 `scripts/` 目录保留为 `skills.legacy/` 和 `scripts.legacy/` 30 天**——回滚兜底。

---

## 4. SKILL.md 主入口设计

### 4.1 YAML Frontmatter（必须）

```yaml
---
name: ae-sdd
description: |
  端到端自动化工程方法论与工具集。从需求分析到代码上线的全流程约束，
  包含 DR/Story/Task/Coding/Review 9 步 SOP + 14 条门禁 + 状态机。
  当开发者说"启动自动化工程"、"从 DR 开始实现"、"端到端实现"、
  "继续流程"、"继续上次"、"/ae-sdd" 时触发。
version: 3.0.0
triggers:
  - "启动自动化工程"
  - "从 DR 开始实现"
  - "端到端实现"
  - "继续流程"
  - "继续上次"
  - "/ae-sdd"
allowed_tools:
  - "ae-sdd"  # 主 CLI
---
```

### 4.2 章节结构（≤ 300 行）

```markdown
# ae-sdd — 自动化工程主入口

## 0. 是什么 / 不是什么
- **是**：方法论 + 工具集 + 流程门禁 + 状态机
- **不是**：单独的 SKILL（是一个 SKILL 系统）
- **不适用**：单行 bug fix / 纯文档编辑 / 一次性脚本

## 1. 三条主路径（按规模选）
| 路径 | 触发 | 入口命令 | 流程深度 |
|------|------|---------|---------|
| ⚡ Quick | < 5 行配置 | `ae-sdd quick <task>` | 跳 DR/Story/Task |
| 🔵 Standard | 单 Story | `ae-sdd run <story-id>` | 完整 9 步 |
| 🟣 Cross-Cutting | ≥ 2 域 | `ae-sdd proposal <scope>` | proposal 先行 |

## 2. 工具 API 速查
[8 个核心 CLI 子命令的 1 行说明 + 用法]

## 3. 子 SKILL 索引
[按 phase 列出所有 rules/ 下的 SKILL 链接 + 一句话功能]

## 4. 状态机约定
[指向 rules/orchestration/SKILL.md §状态机]

## 5. 文档与资产约定
[指向 rules/cross-cutting/document-storage-SKILL.md]

## 6. 标准与约束
[指向 standards/ 9 约束]

## 7. 何时不用 ae-sdd
- 单行 bug fix
- 纯文档撰写
- 一次性脚本
- 调研类问题

## 8. 维护规则
- 修改 `rules/*/rules.yaml` 后必须跑 `ae-sdd sync-tools`
- 提交前跑 `ae-sdd health`
- 升级 SKILL 走 `ae-sdd-update-SKILL.md`
```

### 4.3 与原 `ae-sdd-skill.md` 的边界

| 内容 | 在哪 |
|------|------|
| YAML frontmatter | **SKILL.md**（唯一）|
| 4 类需求判定（详细） | `rules/orchestration/SKILL.md`（保留）|
| 9 步流程（详细） | `rules/orchestration/SKILL.md`（保留）|
| 状态机详解 | `rules/orchestration/SKILL.md`（保留）|
| 角色库 8 角色 | `rules/orchestration/SKILL.md`（保留）|
| CodingModel 11 维 | `rules/phase2-task/rules.yaml`（声明化）|
| 14 门禁 | `rules/phase2-coding/rules.yaml`（声明化）|
| 工具速查 | **SKILL.md**（新增）|
| 三条路径入口 | **SKILL.md**（新增）|
| 维护规则 | **SKILL.md**（新增）|

**SKILL.md 只承载"是什么、怎么进、有什么工具"；ae-sdd-skill.md 承载"怎么走每一步"。**

---

## 5. 工具层设计（8 个核心 CLI）

### 5.1 主 CLI：`ae-sdd`

```bash
ae-sdd <command> [options]

Commands:
  init <project-dir> [project-key]
      初始化项目脚手架（.auto-engineering/ + state.json + 资产引用）

  classify <change-description>
      识别 4 类需求：Type A (微) / B (单 Story) / C (跨 Story) / D (跨域)
      输出 JSON: { type, skip_phases, rationale }

  route <type> [--from-state <state>]
      返回下一步: { phase, skill_path, gates, role }

  state <subcommand> --state <file>
      状态机操作
        next-step    返回当前 state 的下一步
        validate     检查 state.json 合法性
        show         可视化当前状态
        diff         对比两个 state
        lock         锁定版本（user_decision_at 锚定）

  gates check <target> [--gates <gate-list>]
      扫描 14 门禁对 target（Story/Task/Coding）通过情况
      返回: [{ gate_id, name, pass, evidence, blocker }]

  review <story-id> [--round <n>]
      跑 TR-1~TR-7 全局 Task Review
      返回: [{ review_id, name, pass, evidence, blocker }]

  health [--json]
      跑健康度 9 项检查
      返回: [{ check_id, name, pass, severity, message }]

  diff <v1> <v2> [--story <id>]
      对比 v1/v2 文档/状态/资产差异
      返回: unified diff 或结构化变更清单

  sync-tools [--dry-run] [--verify-only]
      从 rules/*.yaml 重生成 tools/lib/*.mjs 骨架
      --verify-only: 只检查不修改

  quick <task-description>
      ⚡ Quick Mode 入口，跳过 DR/Story/Task，直接出 CodingPlan

  run <story-id>
      🔵 Standard Mode 入口，从 state.json 续接或从 DR 重新开始

  proposal <scope>
      🟣 Cross-Cutting Mode 入口，先生成 Proposal 再走标准流程

  version
      输出 ae-sdd 版本（来自 package.json）
```

### 5.2 优先级矩阵

| 优先级 | 命令 | 阻塞其他 | 工时估 | 实施期 |
|--------|------|---------|--------|--------|
| 🔴 P0 | `state` | 是 | 1.5d | Phase 3-1 |
| 🔴 P0 | `gates check` | 是 | 1.0d | Phase 3-1 |
| 🔴 P0 | `classify` | 否 | 0.5d | Phase 3-1 |
| 🟡 P1 | `route` | 否 | 0.5d | Phase 3-2 |
| 🟡 P1 | `review` | 否 | 0.5d | Phase 3-2 |
| 🟡 P1 | `health` | 否 | 0.5d | Phase 3-2（移植 scripts/health-check.mjs）|
| 🟢 P2 | `diff` | 否 | 1.0d | Phase 3-3 |
| 🟢 P2 | `init` | 否 | 0.5d | Phase 3-3 |
| 🟢 P2 | `quick` | 否 | 0.5d | Phase 3-3 |
| 🟢 P2 | `run` | 否 | 0.3d | Phase 3-3 |
| 🟢 P2 | `proposal` | 否 | 0.3d | Phase 3-3 |
| 🟢 P2 | `sync-tools` | 是 | 1.5d | Phase 4 |
| 🟢 P2 | `version` | 否 | 0.1d | Phase 3-3 |

**P0 三件套（state + gates + classify）= 3 天，单独可发布。**

### 5.3 工具实现约束

| 约束 | 说明 |
|------|------|
| **语言** | Node.js ESM（与 migrate-docs.mjs 一致）|
| **测试** | `node --test`（内置，零依赖）|
| **schema 校验** | `ajv`（轻量 JSON Schema 验证）|
| **CLI 框架** | 自研（不引 commander / yargs，保持轻量）|
| **错误处理** | 退出码：0=成功，1=业务错误，2=参数错误，3=工具错误 |
| **日志** | stderr 写日志，stdout 写数据（pipeline 友好）|
| **跨平台** | 不依赖 bash，所有路径用 `path.join` |

### 5.4 与 `scripts/` 现有工具的关系

| 现有工具 | 处理方式 |
|---------|---------|
| `sync-to-plugin.sh` | 保留在根，更新路径引用 |
| `dev-sync.sh` | 改写为 `tools/bin/ae-sdd-dev` |
| `install.{ps1,sh}` | 合并为 `tools/bin/ae-sdd-install`（Node 跨平台）|
| `migrate-docs.mjs` | 迁移到 `tools/lib/migrate-docs.mjs` |
| `test_authenticity_scan.py` | **移植到 Node** → `tools/lib/test-authenticity.mjs` |
| `health-check.mjs`（待做）| 实现为 `tools/bin/ae-sdd health` |

---

## 6. 规则-工具同步机制

### 6.1 核心问题

**当前状态**：规则散文（SKILL.md）↔ 工具脚本（scripts/）各自独立。规则改了，工具不一定改。**这是 ae-sdd 漂移的最大温床。**

### 6.2 方案：声明式 + 代码生成（SSOT）

**关键设计：每个 phase 配套一个 `rules.yaml`，作为该 phase 的声明式事实源。**

#### 示例：`rules/phase2-coding/rules.yaml`

```yaml
version: "1.0"
phase: phase2-coding

# 14 条 CodingPlan 门禁
gates:
  - id: G-01
    name: 现有能力复用扫描
    condition: "所有实现点已扫描项目资产/依赖/历史 Task/公共组件/平台能力"
    severity: blocker
    check_script: "tools/lib/gates.mjs#checkReuseScan"
  - id: G-02
    name: 业内成熟方案参考
    condition: "非平凡实现点已列业内/团队既有方案并说明取舍"
    severity: blocker
    check_script: "tools/lib/gates.mjs#checkIndustryRef"
  # ... 12 more

# 测试真实性 8 类禁止
test_authenticity:
  prohibitions:
    - id: TA-01
      name: 禁止 Mock 替身返回 hardcode
      pattern: 'return\s+["\']\w+["\']'
      # 通过 test-authenticity.mjs 扫描
    - id: TA-02
      name: 禁止跳过异常分支
      pattern: 'catch.*\{\s*\}'
    # ... 6 more

# 角色
roles:
  - id: coding-agent
    name: Coding Agent
    owns: [code-writing, test-writing, self-review]
  - id: code-reviewer
    name: Code Reviewer
    owns: [independent-verdict]
```

#### 示例：`rules/orchestration/rules.yaml`

```yaml
version: "1.0"
phase: orchestration

# 4 类需求
classify:
  types:
    - id: A
      name: 微任务
      criteria: "单文件 / < 5 行 / 仅配置"
      skip_phases: [dr, dr-review, story, story-review, task]
      transaction: "Plan-{服务缩写}-{任务简述}"
    - id: B
      name: 单 Story
      criteria: "1 个 Story 范围"
      skip_phases: []
      transaction: "{STORY-ID}"
    - id: C
      name: 跨 Story
      criteria: "≥ 2 个 Story 联动"
      skip_phases: []
      require_proposal: true
      transaction: "{STORY-ID}-multi"
    - id: D
      name: 跨域
      criteria: "≥ 2 个 bounded context"
      skip_phases: []
      require_proposal: true
      require_cross_review: true
      transaction: "{STORY-ID}-cross-domain"

# 9 步流程（状态机）
state_machine:
  phases:
    - id: requirement-analysis
      skill: phase1-design/requirement-analysis-SKILL.md
    - id: dr-generate
      skill: phase1-design/dr-generate-SKILL.md
    - id: dr-review
      skill: phase1-design/dr-review-SKILL.md
    - id: story-generate
      skill: phase1-design/story-generate-SKILL.md
    - id: story-review
      skill: phase1-design/story-review-SKILL.md
    - id: testcase-generate
      skill: phase1-design/testcase-generate-SKILL.md
    - id: task-generate
      skill: phase2-task/task-generate-SKILL.md
    - id: coding
      skill: phase2-coding/coding-SKILL.md
    - id: coding-report
      skill: phase2-coding/coding-report-SKILL.md
    - id: code-review
      skill: phase3-review/code-review-SKILL.md

  transitions:
    - from: requirement-analysis
      to: dr-generate
      condition: "scale == large"
    - from: requirement-analysis
      to: story-generate
      condition: "scale == medium"
    - from: requirement-analysis
      to: coding
      condition: "scale == micro"
    # ... 8 more
```

### 6.3 sync-tools 工作机制

```
[开发者修改 rules/{phase}/rules.yaml]
         │
         ▼
[运行 ae-sdd sync-tools]
         │
         ├─→ 1. 加载所有 rules/{phase}/rules.yaml
         │
         ├─→ 2. 与 tools/lib/* 现有实现对比
         │     │
         │     ├─ 新增 gate/role/transition → 生成 stub（函数体 TODO）
         │     ├─ 删除/修改 → 标记 stale（不自动删，留人工确认）
         │     └─ 无变化 → 跳过
         │
         ├─→ 3. 对 stub 部分：
         │     ├─ 写入 tools/lib/{cmd}.mjs 的对应函数
         │     ├─ 标记 TODO + 链接到对应 SKILL.md 段落
         │     └─ 不覆盖任何已有的人工实现
         │
         ├─→ 4. 同步更新 SKILL.md §2 工具速查（如果新增命令）
         │
         ├─→ 5. 跑 tools/tests/* 全套测试
         │     └─ 失败 → 回滚 + 报错
         │
         └─→ 6. 输出 sync 报告
               {
                 added: 3,
                 modified: 1,
                 stale: 0,
                 todo_人工补全: 3,
                 tests_passed: 12/12
               }
```

### 6.4 sync-tools 的设计原则

| 原则 | 说明 |
|------|------|
| **不自动覆盖** | 工具核心逻辑有人工判断时，sync 只生成 stub + TODO |
| **可回滚** | 同步前自动 `git add -A && git stash`，失败可恢复 |
| **可测试** | 同步后自动跑 `tools/tests/`，失败即报错回滚 |
| **可校验** | `ae-sdd sync-tools --verify-only` 只检查不修改（CI 用）|
| **双源校验** | SKILL.md 散文与 rules.yaml 必须一致（每周 health 检查）|

### 6.5 "专门写一个规则实例化 SKILL" 的回答

**用户问**：是不是要专门写一个规则实例化 SKILL，用来同步修改工具实例？

**答**：**写，但不是 SKILL 形态——是工具形态。**

理由：
- SKILL = 教学式（告诉 AI 怎么想、怎么走）
- 工具 = 命令式（运行→产出，零推理）

**"规则实例化"本质是代码生成（code generation），不是教学。** 应该用 `ae-sdd sync-tools` CLI，而不是一个新 SKILL。

但 sync-tools 也**可以配一个 SKILL 文档**（`rules/cross-cutting/sync-tooling-SKILL.md`），教 AI：
- 何时运行 sync-tools
- 修改 rules.yaml 后必跑
- 怎么读 sync 报告
- 怎么处理 TODO 标记

**所以形式是"CLI 工具 + 配套 SKILL 文档"双轨。** 这跟 harness 里的"agent + tool"组合一致。

---

## 7. 实施步骤

### Phase 1: 分区与重组（1.0 天）

- [ ] 创建 `rules/` `tools/` `docs/plans/` `docs/adr/` 目录
- [ ] `git mv skills/* rules/` （保留 `skills.legacy/` 30 天）
- [ ] `git mv scripts/{migrate-docs.mjs, test_authenticity_scan.py} tools/lib/`
- [ ] `sync-to-plugin.sh` 更新新路径
- [ ] 所有 SKILL.md 内部相对路径更新（`../skills/` → `../rules/` 等）
- [ ] `.gitignore` 强化（`.idea/` `__pycache__/` `logs/_*.py`）

### Phase 2: SKILL.md 主入口重写（0.5 天）

- [ ] 重写 `SKILL.md` 为 ≤ 300 行薄入口（frontmatter + 8 章节）
- [ ] 把原 1843 行内容**全量迁入** `rules/orchestration/SKILL.md`
- [ ] README.md 改为"项目概览 + 快速开始"
- [ ] `plugins/ae-sdd/SKILL.md` 同步（sync-to-plugin.sh 跑一遍）

### Phase 3: 工具层落地（4.5 天）

#### Phase 3-1: P0 三件套（3.0 天）
- [ ] `tools/bin/ae-sdd` 主 CLI 骨架
- [ ] `tools/lib/state.mjs` + `tools/tests/state.test.mjs`
- [ ] `tools/lib/gates.mjs` + `tools/tests/gates.test.mjs`
- [ ] `tools/lib/classify.mjs` + `tools/tests/classify.test.mjs`
- [ ] `tools/schemas/state.schema.json` `gates.schema.json` `classify.schema.json`
- [ ] 在 `D:\Item\life` 上跑通（实际项目验证）

#### Phase 3-2: P1 三件套（1.0 天）
- [ ] `tools/lib/route.mjs`
- [ ] `tools/lib/review.mjs`
- [ ] `tools/lib/health.mjs`（合并 `scripts/health-check.mjs` 待做）
- [ ] 各自测试

#### Phase 3-3: P2 收尾（0.5 天）
- [ ] `diff` `init` `quick` `run` `proposal` `version` 入口
- [ ] `tools/bin/ae-sdd-dev`（移植 dev-sync.sh）
- [ ] `tools/bin/ae-sdd-install`（合并 install.{ps1,sh}）
- [ ] `tools/README.md`

### Phase 4: 同步机制（1.5 天）

- [ ] 设计 `rules.schema.json`（JSON Schema 校验 rules.yaml 合法性）
- [ ] 实现 `tools/lib/sync.mjs`（生成 stub + 不覆盖 + 跑测试）
- [ ] 6 个 `rules/{phase}/rules.yaml` 写齐
- [ ] 跑 `ae-sdd sync-tools --dry-run` 验证
- [ ] 跑 `ae-sdd sync-tools` 实际生成
- [ ] 写 `rules/cross-cutting/sync-tooling-SKILL.md`（教 AI 何时用 sync-tools）

### Phase 5: 迁移与收尾（0.5 天）

- [ ] CHANGELOG 写 v3.0 重组条目
- [ ] 跑 `ae-sdd health` 全检
- [ ] `plugins/ae-sdd/` 同步副本验证
- [ ] 30 天后清理 `skills.legacy/` `scripts.legacy/`

**总工时：约 8 天**（含 P0 三件套先行发布、中间验证）

### 推荐节奏

| 周 | 内容 | 里程碑 |
|----|------|--------|
| W1 Day 1-2 | Phase 1 + Phase 2 | 分区 + 薄入口完成 |
| W1 Day 3-5 | Phase 3-1 | P0 CLI（state/gates/classify）发布 |
| W2 Day 1-2 | Phase 3-2 + Phase 3-3 | 全部 8 CLI 完成 |
| W2 Day 3-4 | Phase 4 | 同步机制 + rules.yaml 齐备 |
| W2 Day 5 | Phase 5 | CHANGELOG + 健康度 + 归档 |

---

## 8. 风险与回滚

| 风险 | 影响范围 | 缓解措施 | 回滚方案 |
|------|---------|---------|---------|
| 重组期间 SKILL 引用断链 | 所有依赖 ae-sdd 的项目 | 保留 `skills.legacy/` `scripts.legacy/` 30 天 | git revert 到 v2.x tag |
| rules.yaml schema 设计不当 | 后续返工 | Phase 4 先用一个 phase 试跑，迭代 1-2 版 | schema 单独版本化（`schemaVersion`）|
| sync-tools 自动生成质量差 | 工具行为不符预期 | 默认 `--dry-run` + 不覆盖人工实现 | sync 报告里有 `git stash` 引用可恢复 |
| 8 个 CLI 短期塞太多 | 工期失控 | 严格按 P0→P1→P2 优先级，Phase 3-1 完成后插一次评估 | P2 可推迟到 v3.1 |
| Node 移植 Python 工具引入 bug | `test_authenticity_scan.py` 行为差异 | 跑同一组样本对比输出 | 保留 .py 在 `tools/lib/_legacy/` 30 天 |
| 薄入口信息丢失 | 主入口漏链 | 重写时用 8 章节 checklist 全量覆盖 | 原 1843 行还在 `rules/orchestration/SKILL.md` |

---

## 9. 验收标准

### 9.1 结构验收

- [ ] 仓库根 `SKILL.md` 行数 ≤ 300，YAML 合法
- [ ] `rules/` 含 6 phase 子目录，共 18 SKILL.md
- [ ] `tools/bin/ae-sdd` 可执行，`--help` 输出 8+ 子命令
- [ ] `tools/lib/` 含 11+ .mjs 实现
- [ ] `tools/tests/` 含 5+ 测试文件
- [ ] `tools/schemas/` 含 4+ JSON Schema
- [ ] `sync-to-plugin.sh` 跑通，副本与母版一致

### 9.2 功能验收

- [ ] `ae-sdd state next-step --state test/fixtures/sample-state.json` 返回正确下一步
- [ ] `ae-sdd classify "改个枚举值"` 返回 Type A
- [ ] `ae-sdd classify "做个用户管理功能"` 返回 Type B
- [ ] `ae-sdd gates check --target test/fixtures/sample-coding.md` 返回 14 项结果
- [ ] `ae-sdd health` 9 项检查全跑通（无 P0 失败）
- [ ] `ae-sdd sync-tools --dry-run` 不修改任何文件
- [ ] `ae-sdd sync-tools` 修改 rules.yaml 后能识别变化并生成 stub
- [ ] `ae-sdd quick "改个枚举值"` 输出 CodingPlan

### 9.3 实际项目验证

- [ ] 在 `D:\Item\life` 上跑通：`ae-sdd state next-step` 识别 STORY-020 v2-r2
- [ ] 在 `D:\Item\life` 上跑通：`ae-sdd gates check` 扫描一个真实 Coding
- [ ] `D:\Item\icec-cloud-boss` 项目不被破坏（如果有依赖）

### 9.4 文档验收

- [ ] CHANGELOG 有 v3.0 重组条目
- [ ] README.md 改为概览+快速开始
- [ ] `tools/README.md` 写工具开发指南
- [ ] `rules/cross-cutting/sync-tooling-SKILL.md` 写完
- [ ] `docs/plans/2026-06-18-ae-sdd-v3-restructure-plan.md` 本文档归档

---

## 10. 不在本期范围（v3.0 边界外）

- ❌ ae-sdd 母版本身建 `.harness/`（留到 v3.1 评估）
- ❌ 跨 AI 工具适配（adapter-claude-code.ts / adapter-cursor.ts）
- ❌ Quick Mode 完整 SOP（先开 CLI 入口，规则后续补）
- ❌ "5 维"概念归一化（先在 SKILL.md 加限定词，深度重构后续）
- ❌ CodingModel 11 维 声明化（先做 14 门禁和 4 类需求）
- ❌ 国际化 / 多语言（i18n 留待评估）

---

## 11. 关键决策点（请用户确认）

1. **路径命名**：`rules/` 还是 `knowledge/` 还是保留 `skills/`？
2. **CLI 语言**：Node.js ESM（统一）还是保留 Python 工具？
3. **主入口章节数**：8 章节是否合理？是否要加 §9 故障排查？
4. **规则 SSOT 形态**：YAML 声明式 是否合适？或偏好 JSON / TOML？
5. **第一步做啥**：P0 三件套先行（state/gates/classify） 还是 先做规则分区 + SKILL.md 重写？

---

**等待用户决策后开始 Phase 1。**
