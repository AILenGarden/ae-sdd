# ae-sdd PRD 级状态机 + 流程级 compact 方案 v2

**版本:** v2.0
**日期:** 2026-06-25
**基线:** v1 方案 A' + reviewer 报告 (R-1~R-16)
**作者:** root agent (Harness)
**Reviewer:** general agent (mvs_5914995d3d1542e5aa37575f79e20f21)
**Review 报告:** `D:\Item\ae-sdd\docs\plans\2026-06-25-ae-sdd-prd-review-report.md`

---

## 0. 决策摘要

按 reviewer 推荐组合落地：

| # | 决策 | 选择 | 理由 |
|---|---|---|---|
| D-1 | PRD 完成判定闸数 | **B: 4 层 AND + 跨 Story 闸** | R-2 重构 |
| D-2 | state.json 新增字段 | **B: 5 核心 + 3 runtime 字段** | R-5 |
| D-3 | Codex runtime | **A: PoC-first** | 30 分钟成本可接受 |
| D-4 | Story 级 state.json 过渡 | **B: 三阶段渐进迁移** | R-9 兼容 |
| D-5 | 同步清单扩展 | **B: 扩 L280 加 §4.1** | R-10 |
| D-6 | Harness compact 路径 | **B: `harness session rotate --handoff-file`** | 已存在 |

---

## 1. 解决 4 项 🔴 阻断

### R-1：状态机归属 — 落档 `ae-sdd-skill.md`

**改动文件：** `D:\Item\ae-sdd\source\SKILL.md`
**插入位置：** 第 L1111 行之后，§整体流程 章节之前
**新增章节内容：**

```markdown
### § 流程状态跟踪与再启动（PRD 级）— 🆕 v3.3.0

> 状态机归属：本节由 `ae-sdd-skill.md` 单点持有，PRD 级状态机定义在此；
> 子 SKILL（phase1-design / phase2-coding / phase3-review）通过指针引用本节，
> **不**独立发明 PRD 级状态字段。

#### 1.1 PRD 级状态文件路径

| 文件 | 路径 | 写入方 | 读取方 |
|------|------|--------|--------|
| `state.json` | `.auto-engineering/{PRD-ID}/state.json` | Story 完成 hook、PRD 收尾 CLI | 所有 phase SKILL、CLI |
| `state.md` | `.auto-engineering/{PRD-ID}/state.md` | `ae-sdd state prd-complete`（一次性） | 用户、handoff 包 |
| `summary.md` | `.auto-engineering/{PRD-ID}/summary.md` | `harness session rotate --handoff-file` | 下一个 session / 下一个 PRD |

#### 1.2 PRD ID 命名规范

格式：`PRD-<业务域>-<序号>`（3 段，kebab-case）

- 业务域：CS / IM / USER / LIFE（与 `dr-review-skill.md:184` DR ID 业务域对齐）
- 序号：3 位数字，从 001 起
- 示例：`PRD-CS-001`、`PRD-IM-002`、`PRD-USER-001`

#### 1.3 PRD 级 `state.json` schema

参见 R-5 第 5 节（`{state.json schema}` 完整定义）。

#### 1.4 流程脱离场景扩展（5 场景）

参见 R-7 第 7 节。**第 5 场景：PRD 完成 / 进入下一个 PRD**。
```

---

### R-2：PRD 完成判定 — 4 层 AND + 跨 Story 闸

**改动文件：** `D:\Item\ae-sdd\source\SKILL.md`（新增章节，紧接 R-1 之后）

**新增章节内容：**

```markdown
### § PRD 完成判定 SOP（4 层 AND + 跨 Story 闸）— 🆕 v3.3.0

#### 1.5 🔍 人工审核点 5：PRD 完成确认（新增）

与现有 4 个 Story 级人工审核点（`SKILL.md:1167`）同级，编号 5。

**触发时机：** 4 层 AND 全过 + 用户说「PRD 收尾了 / 进入下一个 PRD」

**AI 主动讲解模板（基于 `SKILL.md:1300-1343` 扩展）：**

1. PRD 业务全貌（PRD 文档 + DR 摘要）
2. 各 Story 完成情况（聚合自 state.json）
3. **跨 Story 关键决策**（从 `crossStoryDeps` + `crossStoryResidualRisks` 提取）
4. **sizeBudget 实际 vs 估算**（新增维度）
5. 残留风险清单（owner + dueDate）

**记录字段：**
```json
{
  "prdReview": {
    "confirmedAt": "2026-06-25T...",
    "confirmedBy": "<user-id>",
    "storytoldAt": "2026-06-25T...",
    "openQuestions": []
  }
}
```

#### 1.6 PRD 完成判定闸（4 层 AND）

```
G-PRD-1 (Story 全部完成):
  ∀ STORY-ID ∈ prdState.storyIds:
    story.codeReviewReport 存在
    ∧ story.sevenBisPassed == true
    ∧ story.userConfirmedAt 非空

G-PRD-2 (Story ⑦bis 全通过):
  ∀ STORY-ID: story.sevenBisMatrix 无 🔴 断链
  ∧ ∀ STORY-ID: story.sevenBisMatrix 出闸条件满足

G-PRD-3 (跨 Story 残留风险已闭环):
  crossStoryDeps[].verifiedAt 全部非空
  ∧ crossStoryResidualRisks[].mitigationPlan 全部非空
  ∧ ∀ risk.severity == "🟢" 或 risk.dueDate > now

G-PRD-4 (PRD 级人工审核通过):
  prdReview.confirmedAt 非空
  ∧ prdReview.confirmedBy 存在
```

**CLI 入口：** `ae-sdd state prd-check-complete --prd {PRD-ID}`（只校验，不改状态）
```

---

### R-3：Codex runtime PoC（30 分钟，可独立并行）

**不阻塞** M1/M2 实施，与 v3.3.0 发布解耦。

**PoC 任务（用户或 reviewer 执行）：**

```bash
# 1. 查 Codex 官方文档 hook API（10 分钟）
codex --help 2>&1 | Select-String -Pattern "hook|notify|event|trigger"
Get-ChildItem -Path "$env:USERPROFILE\.codex" -Filter "*hook*" -ErrorAction SilentlyContinue

# 2. 查 Codex session export / dump（10 分钟）
codex session export --help 2>&1
codex session dump --help 2>&1

# 3. 翻 docs.codex.com / GitHub issue（10 分钟）
# 搜 "hook protocol" / "PreToolUse equivalent" / "Stop hook"
```

**PoC 结论落档：** `D:\Item\ae-sdd\docs\plans\2026-06-25-codex-poc-result.md`
**决策矩阵：** 参见 review 报告 §9.3（hook → 3 runtime / export → 2.5 runtime / 都没有 → 2 runtime + 1 manual）

---

### R-4：state.json 路径规范 + PRD ID 命名

**改动文件：** `D:\Item\ae-sdd\source\skills\cross-cutting\document-storage-skill.md`

**在 §2.5 流程状态文件路径表（第 494 行附近）追加：**

```markdown
| 流程状态文件（PRD 级）| 不带版本号 | `.auto-engineering/{PRD-ID}/state.json` | 状态实时变化 |
| 流程状态文件（PRD 级，handoff）| 不带版本号 | `.auto-engineering/{PRD-ID}/summary.md` | `harness session rotate` 时生成 |
| 流程状态文件（PRD 级，人类读）| 不带版本号 | `.auto-engineering/{PRD-ID}/state.md` | `ae-sdd state prd-complete` 时一次性生成 |
```

**PRD ID 命名规范：** 已在 R-1 §1.2 定义；同步在 `ae-sdd-conventions.md §3 路径速查` 加一行：

```markdown
| PRD ID 命名 | `PRD-<业务域>-<序号>` | 与 DR ID 业务域对齐 | `dr-review-skill.md:184` |
```

---

## 2. 解决 7 项 ⚠️ 必改

### R-5：`state.json` 扩展 schema（落档 `document-storage-skill.md §2.5.1`）

**完整 schema：**

```json
{
  "$schema": "https://ae-sdd.dev/schema/prd-state-v1.0.0.json",
  "schemaVersion": "1.0.0",
  "prdId": "PRD-CS-001",
  "prdTitle": "客服系统 v1",
  "prdDocPath": "ae-sdd-doc/PRD/PRD-CS-001.md",
  "drId": "DR-CS-001",
  "storyIds": [
    {
      "storyId": "STORY-002-BE",
      "state": "completed",
      "taskIds": ["TASK-002-01", "TASK-002-02"],
      "codingPlanIds": ["CP-002-01", "CP-002-02"],
      "codeReviewReport": ".auto-engineering/STORY-002-BE/CR-r1.md",
      "sevenBisPassed": true,
      "userConfirmedAt": "2026-06-25T10:30:00Z",
      "completedAt": "2026-06-25T10:30:00Z"
    }
  ],

  "crossStoryDeps": [
    {
      "fromStory": "STORY-002-BE",
      "toStory": "STORY-007-FE",
      "depType": "api",
      "critical": true,
      "verifiedAt": null,
      "verifiedBy": null
    }
  ],

  "crossStoryResidualRisks": [
    {
      "riskId": "RISK-PRD-CS-001-001",
      "description": "...",
      "owner": "...",
      "severity": "🟠",
      "dueDate": "2026-07-15",
      "mitigationPlan": "..."
    }
  ],

  "sizeBudget": {
    "estimated": { "storyCount": 5, "taskCount": 18, "hours": 240 },
    "actual": { "storyCount": 6, "taskCount": 22, "hours": 310 },
    "variance": { "storyCountPct": 20, "taskCountPct": 22, "hoursPct": 29 }
  },

  "prdReview": {
    "confirmedAt": null,
    "confirmedBy": null,
    "storytoldAt": null,
    "openQuestions": []
  },

  "memoryLifecycle": {
    "enterHistory": [{ "at": "2026-06-01T...", "phase": "ra" }],
    "writeHistory": [{ "at": "2026-06-15T...", "phase": "design", "kind": "decision" }],
    "exitHistory": [{ "at": "2026-06-25T...", "phase": "review" }]
  },

  "runtimeHooks": {
    "harness": { "compactCmd": "harness session rotate", "args": ["--handoff-file", "{summary.md}"] },
    "claude-code": { "hookType": "UserPromptSubmit", "injectCmd": "..." },
    "codex": { "compactCmd": null, "status": "unsupported", "fallback": "user-manual" }
  },

  "gateRegistry": {
    "G-PRD-1": "pending",
    "G-PRD-2": "pending",
    "G-PRD-3": "pending",
    "G-PRD-4": "pending"
  },

  "prdStatus": "in_progress | prd_complete_pending_user | awaiting_compact | compacted | prd_aborted",
  "lastUpdated": "2026-06-25T10:30:00Z",
  "compactHistory": []
}
```

**`prdStatus` 枚举扩展：**
- `in_progress` — 进行中
- `prd_complete_pending_user` — 4 层 AND 全过，等用户确认收尾
- `awaiting_compact` — 用户已确认，等 compact 钩子触发
- `compacted` — compact 完成，可进入下一个 PRD
- `prd_aborted` — 异常终止（保留现场，不删 state.json）

**memory lifecycle 强制门禁（v3.2.3+）：** `ae-sdd state write --phase <next>` 必须先查 `memoryLifecycle.enterHistory` 有对应 phase 的 `enter` 记录 + `writeHistory` 有 `write` 记录。

---

### R-6：CLI 命名 — `ae-sdd state prd-complete` 单层

**改动文件：** `D:\Item\ae-sdd\source\SKILL.md:1996-2006` 子命令列表追加

```markdown
| `state prd-check-complete` | 校验 4 层 AND，输出未达成项，不改状态 | `ae-sdd state prd-check-complete --prd {PRD-ID}` |
| `state prd-complete` | 校验通过后执行 compact，更新 prdStatus | `ae-sdd state prd-complete --prd {PRD-ID} --runtime {harness|claude-code|codex}` |
| `state prd-archive` | 归档 compactHistory 到 state.archive.json | `ae-sdd state prd-archive --prd {PRD-ID} --keep-last 5` |
| `runtime compact` | runtime-specific compact 适配层 | `ae-sdd runtime compact --runtime {harness|claude-code|codex}` |
```

---

### R-7：流程脱离 SOP 第 5 场景

**改动文件：** `D:\Item\ae-sdd\source\SKILL.md:1053-1099` 流程脱离判定表追加

```markdown
| **场景 5：PRD 完成 / 进入下一个 PRD** | 用户说「PRD 收尾了 / 进入下一个 PRD」 | 读 `.auto-engineering/{PRD-ID}/state.json`，校验 prdStatus 状态，触发 PRD 级 compact |
```

**再启动判定规则表（`SKILL.md:1103-1111`）追加一行：**

```markdown
| PRD 完成 / 进入下一个 PRD | 读 `.auto-engineering/{PRD-ID}/state.json`，校验 prdStatus=compacted，写 next-prd 指针 | `.auto-engineering/{PRD-ID}/state.json` + `.auto-engineering/PRD-NEXT/state.json` 模板预生成 |
```

---

### R-8：`HARNESS.md` 新增 HS-7 / HS-8

**改动文件：** `D:\Item\ae-sdd\source\HARNESS.md:77-85`

```markdown
| **HS-7** | PreToolUse hook 拦截 | 未通过 4 层 AND 闸就触发 `ae-sdd state prd-complete` | 🔴 物理阻断 | `source/SKILL.md §1.6` |
| **HS-8** | Stop hook 检查 | PRD 级 compact 失败时未保留旧 PRD state.json | 🔴 阻断 + 报警 | `source/SKILL.md §1.3` |
```

**三层 hook 协议（`HARNESS.md:101-128`）补充 PRD 级路径：**

```markdown
| UserPromptSubmit（PRD 完成场景）| 注入 `state.md` 路径 + prdStatus | 复用现有 UserPromptSubmit 协议 |
```

---

### R-9：Harness 适配层重生成

**改动文件：** 无（命令驱动）

**执行命令（实施时跑）：**

```bash
cd D:\Item\ae-sdd
pwsh tools/convert-ae-sdd-to-harness.ps1
git diff harness/.adapter.lock  # 验证 commit hash 已更新
```

**新增条目（`source/SKILL.md §1.1~1.6`）：** 见 R-1/R-2，自动被 adapter 抓取。

**前置条件：** R-1/R-2 母版已 commit + R-5 schema 文档已落档。

---

### R-10：CHANGELOG + update-graph.json + README 同步清单

**改动 1：新建 CHANGELOG 文件**

`D:\Item\ae-sdd\source\CHANGELOG\2026-06-25-v3.3.0-prd-level-state-and-compact.md`

（命名规范按 `ae-sdd-update-skill.md:296`）

**改动 2：更新 `D:\Item\ae-sdd\source\standards\update-graph.json`**

新增 `prd-level-state` 块（参见 review 报告 §9.4）。

**改动 3：更新 `README.md:5` 版本号行**

```markdown
> **版本：** 2026-06-25（最新变更：v3.3.0 PRD 级状态机 + 流程级 compact）
```

**改动 4：更新 `ae-sdd-update-skill.md:280` 同步清单**

新增 §4.1 同步清单扩展（参见 review 报告 §9.4 完整表）。

---

### R-11：`state.md` 模板（与 `state.json` 职责分离）

**新建文件：** `D:\Item\ae-sdd\source\templates\design\prd-summary-template.md`

**职责边界：**
- `state.json` — 机器读，结构化，含 gate 状态
- `state.md` — 人类读，叙述性，**不重复 state.json 字段**，只讲"为什么这样设计 + 跨 Story 关键决策 + 残留风险"
- `summary.md` — handoff 包，rotate 时生成，介于两者之间

**state.md 模板内容：**

```markdown
# PRD {prdId} 完成总结

> 本文档由 `ae-sdd state prd-complete` 一次性生成。结构化字段见 `state.json`，
> 移交包见 `summary.md`，本文件只讲"为什么"。

## 1. PRD 业务全貌
（3-5 段叙事，讲清这个 PRD 解决了什么业务问题、为什么这样切片）

## 2. 跨 Story 关键决策
（按时间顺序，记录每个跨 Story 决策的 context / options / rationale / impact）

## 3. sizeBudget 实际 vs 估算
（表格 + 偏差归因）

## 4. 残留风险与后续行动
（owner + dueDate + mitigation 进度）

## 5. 下一步建议
（给下一个 PRD / 下一个 session 的 actionable hints）
```

---

## 3. Runtime 适配落地（D-6 = B）

### Harness（已存在 rotate 路径）

**实现：** `ae-sdd runtime compact --runtime harness` 内部调用：

```bash
harness session rotate --handoff-file .auto-engineering/{PRD-ID}/summary.md
```

**前置：** `summary.md` 必须由 `ae-sdd state prd-complete` 先生成。

### Claude Code（改造 UserPromptSubmit 协议）

**实现：** PRD 完成时 `state prd-complete` 触发 bash 写 `state.md` + `summary.md`；
下次 prompt 通过 `prompt-inject` hook 自动注入 PRD 完成提示 + 文件路径。

**复用现有协议：** `HARNESS.md:101-128` UserPromptSubmit 协议不动，只追加 PRD 级 payload。

### Codex（PoC-first，详见 R-3）

PoC 结论决定最终路径。

---

## 4. 实施计划（4 里程碑）

### M1 — 阻断解决（R-1 + R-2 + R-4）

| 任务 | 文件 | 预计时间 |
|------|------|----------|
| R-1 新增 SKILL.md §1.1~1.4 | `source/SKILL.md` | 1.5 h |
| R-2 新增 SKILL.md §1.5~1.6 | `source/SKILL.md` | 1.5 h |
| R-4 document-storage-skill.md §2.5 追加 | `source/skills/cross-cutting/document-storage-skill.md` | 0.5 h |
| R-4 ae-sdd-conventions.md §3 追加 | `source/ae-sdd-conventions.md` | 0.5 h |
| **小计** | | **4 h** |

### M2 — 必改落地（R-5 + R-6 + R-7 + R-8 + R-9 + R-10 + R-11）

| 任务 | 文件 | 预计时间 |
|------|------|----------|
| R-5 state.json schema 落档 | `source/skills/cross-cutting/document-storage-skill.md §2.5.1` | 1 h |
| R-6 CLI 子命令列表追加 | `source/SKILL.md:1996-2006` | 0.5 h |
| R-7 流程脱离第 5 场景 + 再启动判定 | `source/SKILL.md:1053-1111` | 1 h |
| R-8 HARNESS HS-7/HS-8 + UserPromptSubmit PRD payload | `source/HARNESS.md` | 1 h |
| R-9 harness 重生成命令（实施时跑） | `tools/convert-ae-sdd-to-harness.ps1` | 0.5 h |
| R-10 CHANGELOG + update-graph.json + README:5 + update-skill.md §4.1 | 4 个文件 | 1.5 h |
| R-11 prd-summary-template.md | `source/templates/design/prd-summary-template.md` | 0.5 h |
| **小计** | | **6 h** |

### M3 — Codex PoC + 决策（R-3）

| 任务 | 负责 | 预计时间 |
|------|------|----------|
| Codex PoC 跑完 | 用户 / reviewer | 30 min |
| PoC 结论落档 `docs/plans/2026-06-25-codex-poc-result.md` | 跑 PoC 那位 | 30 min |
| 根据 PoC 决定 D-3 最终选 A/B/C | 用户拍板 | 5 min |
| **小计** | | **1 h（与 M1/M2 并行）** |

### M4 — 验证

```bash
# 1. ae-sdd update-check 全绿
cd D:\Item\ae-sdd
pwsh tools/ae-sdd.ps1 update-check
# 预期：UC-01/UC-05/UC-08 全 pass

# 2. dev-sync.sh 跑通
bash tools/dev-sync.sh
# 预期：母版 → Claude install 同步成功

# 3. harness 重生成
pwsh tools/convert-ae-sdd-to-harness.ps1
git diff harness/.adapter.lock  # commit hash 已更新

# 4. ae-sdd health 9 项
pwsh tools/ae-sdd.ps1 health
# 预期：9/9 pass，含新增的 PRD 级路径检查
```

---

## 5. 总览 checklist（实施时勾选）

```
M1 — 阻断解决
[ ] R-1: source/SKILL.md §1.1~1.4 新增
[ ] R-2: source/SKILL.md §1.5~1.6 新增
[ ] R-4: source/skills/cross-cutting/document-storage-skill.md §2.5 追加
[ ] R-4: source/ae-sdd-conventions.md §3 追加

M2 — 必改落地
[ ] R-5: state.json schema 落档 §2.5.1
[ ] R-6: SKILL.md:1996-2006 追加 4 个新 CLI
[ ] R-7: SKILL.md:1053-1111 追加第 5 场景 + 再启动行
[ ] R-8: HARNESS.md:77-85 追加 HS-7/HS-8 + UserPromptSubmit payload
[ ] R-9: harness 重生成命令（实施后跑）
[ ] R-10: CHANGELOG/2026-06-25-v3.3.0-*.md 新建
[ ] R-10: source/standards/update-graph.json 新增 prd-level-state 块
[ ] R-10: README.md:5 版本号更新
[ ] R-10: source/skills/orchestration/ae-sdd-update-skill.md §4.1 追加
[ ] R-11: source/templates/design/prd-summary-template.md 新建

M3 — Codex PoC
[ ] PoC 跑完，结论落档 docs/plans/2026-06-25-codex-poc-result.md
[ ] D-3 拍板（A/B/C）

M4 — 验证
[ ] ae-sdd update-check 全绿
[ ] dev-sync.sh 跑通
[ ] harness 重生成 commit hash 更新
[ ] ae-sdd health 9 项全 pass
```

---

## 6. 决策可逆性 + 风险登记

| 决策 | 可逆性 | 风险 | 缓解 |
|------|--------|------|------|
| D-1 4 层 AND | 中（改 G-PRD-* 字段语义）| 新审核点 5 增加用户操作 1 次 | UI 提示 |
| D-2 8 新字段 | 高（schema 演进 + 迁移脚本）| 旧 state.json 缺字段报错 | 字段全 optional + 默认值 |
| D-3 PoC-first | 高（不动 ae-sdd 母版）| PoC 失败 = 临时降级 | Codex 标 "manual"，FAQ 文档化 |
| D-4 三阶段 | 高（默认 v3.3 阶段 1）| Story 级 state.json 字段漂移 | 双向锚定 `prdId` |
| D-5 §4.1 子节 | 高（追加不修改原 L280）| 同步清单变长 | 表格化 |
| D-6 rotate | 高（已存在的命令）| summary.md 必须先生成 | state prd-complete 强制前置 |

---

## 7. 关联文档

- v1 方案 A' 草案：（对话历史）
- Review 报告：`D:\Item\ae-sdd\docs\plans\2026-06-25-ae-sdd-prd-review-report.md`（34KB / 663 行）
- Codex PoC 结果（待）：`D:\Item\ae-sdd\docs\plans\2026-06-25-codex-poc-result.md`
- ae-sdd 母版主入口：`D:\Item\ae-sdd\source\SKILL.md`
- Harness 协议：`D:\Item\ae-sdd\source\HARNESS.md`
- 路径规范：`D:\Item\ae-sdd\source\skills\cross-cutting\document-storage-skill.md`
- 同步机制：`D:\Item\ae-sdd\source\skills\orchestration\ae-sdd-update-skill.md`
- 已有 v3.2.6 multi-reviewer CHANGELOG：`D:\Item\ae-sdd\source\CHANGELOG\2026-06-25-v3.2.6-multi-reviewer-default-framework.md`（参考命名 + 格式）

---

**下一步：** 用户确认 v2 方案 → 按 M1→M2→M3→M4 顺序执行 → 实施完成再做一轮 review 验证
