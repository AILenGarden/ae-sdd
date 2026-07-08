# PRD {PRD-ID} 完成总结（state.md 模板）

> **🔴 文档定位（v3.3.0 起）：** 本文档由 `ae-sdd state prd-complete` 一次性生成，路径 `.auto-engineering/{PRD-ID}/state.md`。
>
> **职责边界（与 state.json / summary.md 分离）：**
> - `state.json` — **机器读**，结构化，含 gate 状态、Story 聚合、跨 Story 依赖
> - `state.md`（本文件）— **人类读**，叙述性，**不重复 state.json 字段**，只讲"为什么"
> - `summary.md` — **handoff 包**，`mavis session rotate --handoff-file` 时生成，给下个 session / 下个 PRD 用

---

## 1. PRD 业务全貌 `必填`

> 3-5 段叙事，讲清这个 PRD 解决了什么业务问题、为什么这样切片。

{描述当前业务现状、痛点、机会；本次 PRD 的核心目标；为什么用 Story A/B/C 这种切法（而不是 Story X/Y/Z）}

---

## 2. 跨 Story 关键决策 `必填`

> 按时间顺序记录。每个决策 4 段：context（背景）/ options（候选方案）/ rationale（最终选择 + 理由）/ impact（影响范围 + 后续 Story 如何对接）。

### 2.1 {决策标题，如"是否在 STORY-002 中同步做 STORY-007 的接口预埋"} `必填`

- **Context:** {当时遇到什么问题、什么用户输入触发}
- **Options:**
  - A. {方案 A}
  - B. {方案 B}
  - C. {方案 C}
- **Rationale:** {为什么选 X，cite 业务/技术/工期约束}
- **Impact:** {影响 STORY-xxx 的哪些 Task；后续 Story 如何对接；是否有 TODO 残留}

### 2.2 {决策标题} `必填`

- **Context:** ...
- **Options:** ...
- **Rationale:** ...
- **Impact:** ...

---

## 3. sizeBudget 实际 vs 估算 `必填`

> 表格 + 偏差归因。所有数据来自 `state.json.sizeBudget`，本节只解释**为什么偏差**。

| 维度 | 估算 | 实际 | 偏差% | 偏差归因 |
| --- | --- | --- | --- | --- |
| Story 数 | {N} | {M} | {P%} | {如：新增 F-Stage 前端契约 Review，导致 STORY-001-BE 拆为 2 个} |
| Task 数 | {N} | {M} | {P%} | ... |
| 工时 | {N}h | {M}h | {P%} | ... |

**关键偏差决策：**
- {偏差 > 20% 的维度必须说明：要不要调整下一 PRD 的估算公式 / 是否是偶发性偏差}

---

## 4. 残留风险与后续行动 `必填`

> 来自 `state.json.crossStoryResidualRisks`。本节只讲 owner + dueDate + 缓解进度，结构化字段见 state.json。

| Risk ID | 描述 | Owner | Due | 缓解进度 |
| --- | --- | --- | --- | --- |
| RISK-PRD-{X}-001 | {描述} | {owner} | {due} | {🟢 已闭环 / 🟠 缓解中 / 🔴 阻塞} |
| RISK-PRD-{X}-002 | ... | ... | ... | ... |

**🔴 阻塞项单独说明：**
- {Risk ID}：{为什么阻塞 / 需要谁介入 / 预期何时解}

---

## 5. 下一步建议 `必填`

> 给下一个 PRD / 下一个 session 的 actionable hints。

### 5.1 给下一个 session `必填`
- {读 `.auto-engineering/{PRD-ID}/summary.md` 接手}
- {关注 G-PRD-* 闸的阻塞项}

### 5.2 给下一个 PRD `必填`
- {建议先复用本 PRD 的 Story 切分模式 / CodingPlan 模板}
- {建议把残留风险 owner 转交下个 PRD 的 owner}

### 5.3 给 ae-sdd 母版的反馈 `选填`
- {本次 PRD 实施中发现哪些母版规则不够用 / 需要修订}
- {建议改进的章节指针（`ae-sdd-skill.md §X` / `document-storage-skill.md §Y`）}

---

> **生成元信息**
> - 生成时间：{ISO timestamp}
> - 生成 CLI：`ae-sdd state prd-complete --prd {PRD-ID} --runtime {runtime-name}`
> - 关联 PRD 文档：`ae-sdd-doc/PRD/{PRD-ID}.md`
> - 关联 state.json：`.auto-engineering/{PRD-ID}/state.json`
> - 关联 summary.md：`.auto-engineering/{PRD-ID}/summary.md`（compact 时生成）