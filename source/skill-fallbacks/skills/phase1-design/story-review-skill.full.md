---
name: story-review
description: 根据 DR + PRD + 产品原型 + Story 模板审查 Story，记录缺陷、存疑项和误报，先写 Supplement，再触发 Proposal，禁止生成旧版计划载体。当开发者说"审查 Story"、"检查 Story"、"优化 Story"、"Story 缺陷挖掘"、"Story Review"时触发。
---

# Story Review — Story 缺陷挖掘 Skill

> **🆕 v3.9.3 重大变更：**
> 1. §第零步：准入清单统一指向 `story-input-checklist.md` SSOT，13 项自检表 + CLI 门禁双重保障
> 2. §A-E 检查维度新增 §A.bis（DR-Story 字段级一致性）、§D.bis（来源追溯验证）、§D.ter（依赖 Story 字段对齐）
> 3. §退出条件 补充"来源追溯报告已生成 + 字段对齐报告已生成 + 13 项输入自检 + CLI 门禁全过"
> 4. §核心原则 增加"🔴 来源缺失必进 Proposal / 字段不一致必进 Proposal"

## 目标

系统化审查 Story，确认其是否与 DR、PRD、原型、模板与项目约束一致。发现确认缺陷后，不直接改 Story，而是先形成 Proposal，再由 Story Update 执行。

## 依赖标准

- [Story 输入清单 SSOT（v3.9.3 新增）](../../standards/story/story-input-checklist.md) **← 单一权威源**
- [Story Review 检查标准](../../standards/story/story-review-checklist.md)（v3.9.3 已扩展 §A.bis / §D.bis / §D.ter）
- [Story 前端接口契约标准（①bis）](../../standards/story/story-frontend-contract-standard.md)
- [`proposal-skill.md`](../cross-cutting/proposal-skill.md)
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)
- [`review-loop-skill.md`](../cross-cutting/review-loop-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 输出文档 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| Story Review 报告 | `ae-sdd doc save --intent STORY_REVIEW --work-item {W} --story-id {S} --version "r1" --content-file 草稿.md` | 带 r{N} | 新增 |
| Story Supplement | `ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |
| Proposal | `ae-sdd doc save --intent PROPOSAL --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |
| 跨轮 Review 对比表 | `ae-sdd doc save --intent REVIEW_COMPARE --work-item {W} --story-id {S} --version "v1-to-v2" --content-file 草稿.md` | 带 v{N}-to-v{M} | 新增 |
| 🔴 **来源追溯报告（v3.9.3 新增）** | `ae-sdd doc save --intent STORY_SOURCE_TRACE --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |
| 🔴 **依赖 Story 字段对齐报告（v3.9.3 新增）** | `ae-sdd doc save --intent STORY_FIELD_ALIGN --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |

## 第零步：Story Review 准入检查（🆕 v3.9.3 强化为 SSOT 化）

**输入清单统一指向 [`story-input-checklist.md`](../../standards/story/story-input-checklist.md) 单一权威源（SSOT），共 13 项输入。**

### 0.1 加载 SSOT
读 `source/standards/story/story-input-checklist.md` 全文，按 §3 输入加载 SOP 4 步执行。

### 0.2 13 项 prose 自检表（必须全部 ✅）

```
[ ] A1 DR 文档已读取
[ ] A2 PRD 文档已读取
[ ] A3 产品原型已读取（或标注"无原型"）
[ ] A4 历史 RA 已读取（如有）
[ ] B1 项目资产已读取
[ ] B2 项目约束已读取
[ ] B3 前端约束已读取（如涉及前端）
[ ] B4 依赖能力清单已扫描
[ ] C1 依赖 Story 已加载并比对字段
[ ] C2 阻塞下游 Story 已识别（如有）
[ ] D1 Story 模板已加载
[ ] D2 Story 生成标准已加载
[ ] D3 Story 审核标准 + 前端契约标准已加载
[ ] D4 测试策略已加载
```

### 0.3 🔴 机械门禁

```bash
ae-sdd gates check --only G-STORY-CTX
```

`G-STORY-CTX` 校验 `constraints/assets/DR/PRD/dependsStory/sourceTrace` 6 类上下文已读齐（v3.9.3 新增 dependsStory + sourceTrace）。未过 → **BLOCK，禁止进入 Review**。

### 0.4 13 项自检通过后，按 `story-review-checklist.md` §准入检查 10 项确认 Story 本身状态

任一缺失，禁止进入 Review。

### 0.5 读取入口（v3.9.3 强化）

- DR / PRD / Story / Supplement：`ae-sdd doc resolve --intent X --story-id {S}`
- 项目资产：`document-storage-skill.get_assets(projectKey)` + `ae-sdd assets read story-review --project <projectKey>`
- 项目约束：`document-storage-skill.get_constraints(projectKey)` → `constraints/*.md`
- **🔴 v3.9.3 新增：** 依赖 Story：扫描 Story 元信息"复用其他 Story"表 → 逐个 `ae-sdd doc resolve --intent STORY --story-id {ID}`
- **🔴 v3.9.3 新增：** 来源追溯报告：`ae-sdd doc resolve --intent STORY_SOURCE_TRACE --story-id {S}`（如已存在）

## 流程

```text
触发
  → 【v3.9.3 强化】第零步：加载 13 项输入（SSOT prose 自检 + G-STORY-CTX 门禁）
  → 【v3.9.3 强化】按 story-review-checklist A-E + A.bis + D.bis + D.ter 检查
  → 【v3.9.3 新增】写来源追溯报告（§D.bis 校验）
  → 【v3.9.3 新增】写依赖 Story 字段对齐报告（§D.ter 校验）
  → 写 Supplement 和 Review 报告
  → 若存在 confirmed 缺陷，生成 Proposal
  → 触发 Story Update 按 Proposal 修复
  → 再次 Review
  → 循环与退出遵守 review-loop-skill.md
```

## 核心原则

- Story Review 不直接修改 Story 主文档。
- Story Review 不生成旧版计划载体。
- 已确认缺陷必须进入 Proposal。
- 存疑项不得自动执行，必须人工确认。
- 误报只记录，不进入修复流。
- 循环与退出条件遵守 `review-loop-skill.md`，本文件不重复定义轮次阈值。
- **🔴 v3.9.3 新增：** 来源缺失（§D.bis 不通过）必须进 Proposal，不得标 deferred。
- **🔴 v3.9.3 新增：** 字段不一致（§A.bis / §D.ter 不通过）必须进 Proposal，不得标 deferred。

## A-E / F 检查口径（🆕 v3.9.3 扩展为 8 个维度）

**完整检查项见 `standards/story/story-review-checklist.md`（v3.9.3 已扩展为 A + A.bis + B + C + D + D.bis + D.ter + E + F 共 8 维）。**

简化约束如下：

1. **§A DR-Story 一致性** —— 业务规则、边界场景、验收标准、字段定义、接口契约与 DR 一致
2. **🔴 §A.bis（v3.9.3 新增）DR-Story 字段级一致性** —— 字段名/类型/必填/校验规则逐字段比对
3. **§B AC 完整性** —— AC 覆盖核心目标、可独立验证、可映射到输入/输出/状态变化、无空泛表述
4. **§C 业务逻辑覆盖** —— 主流程、异常流程、分支、状态流转完整 + C8 数据视角
5. **§D 数据模型与接口** —— 字段链路闭环、数据模型与接口契约一致、DB/外部依赖/DTO/VO/DO/PO 对应
6. **🔴 §D.bis（v3.9.3 新增）来源追溯验证** —— 每个 AC/字段/异常/数据模型字段都有章节级来源标注
7. **🔴 §D.ter（v3.9.3 新增）依赖 Story 字段对齐** —— 复用项字段与依赖 Story 实际定义一致
8. **§E 模板与约束** —— 必填章节齐全、模板格式一致、项目约束落地、引用可定位、偏离声明明确
9. **§F 前端接口契约** -- ①bis 6 维度全填（指向 `## 接口契约-REST` 章节）

## 输出要求

- 必须输出 Review 报告。
- 必须回写 Supplement。
- 确认缺陷必须生成 Proposal。
- 不得输出旧版计划载体。
- **🔴 v3.9.3 新增：** §D.bis 来源追溯校验结果必须写入 `STORY_SOURCE_TRACE` 文档。
- **🔴 v3.9.3 新增：** §D.ter 依赖 Story 字段对齐结果必须写入 `STORY_FIELD_ALIGN` 文档。

## 退出条件

满足 `review-loop-skill.md` 的退出条件，且满足以下 6 项：

1. C8 数据视角已完成
2. Supplement 与 Proposal 状态已回写
3. **🔴 v3.9.3 新增：** 来源追溯报告已生成（§D.bis 全部勾选）
4. **🔴 v3.9.3 新增：** 依赖 Story 字段对齐报告已生成（§D.ter 全部勾选）
5. **🔴 v3.9.3 新增：** 13 项输入 prose 自检 + CLI 门禁全过
6. **🔴 v3.9.3 新增：** 8 维检查（A + A.bis + B + C + D + D.bis + D.ter + E）全部有结论（confirmed/deferred/misreport/info）

## 禁止事项

- 禁止直接修改 Story 主文档。
- 禁止把误报当缺陷。
- 禁止在 Review 内直接生成修复计划。
- 禁止跳过 Proposal 直接触发 Story Update。
- **🔴 v3.9.3 新增：** 禁止将 §D.bis 来源缺失标为 deferred（必须进 Proposal）。
- **🔴 v3.9.3 新增：** 禁止将 §A.bis / §D.ter 字段不一致标为 deferred（必须进 Proposal）。
- **🔴 v3.9.3 新增：** 禁止未跑 G-STORY-CTX 门禁就宣布 Review 开始。