---
name: story-update
description: 根据 Proposal、Story 补充说明文档和 Story 模板更新 Story 主文档。当 Story Review、Coding 或其他渠道发现 Story 缺陷时触发，或开发者说"更新 Story"、"同步补充说明到 Story"时触发。
---

# Story Update — Story 文档更新 Skill

> **🆕 v3.9.3 重大变更：**
> 1. §第零步：准入清单统一指向 `story-input-checklist.md` SSOT（13 项自检表）
> 2. §执行规则新增第 6 条："Update 修改字段时必须保持来源追溯标注不被破坏"
> 3. §禁止事项新增 4 条：禁止破坏来源追溯 / 禁止修改字段而不同步来源 / 禁止跳过 §三步 bis 来源追溯

## 目标

根据 Proposal 中记录的修复建议更新 Story 主文档，并同步回写 Supplement、模板优化建议或 DR 反馈。

## 依赖标准

- [Story 输入清单 SSOT（v3.9.3 新增）](../../standards/story/story-input-checklist.md) **← 单一权威源**
- [Story Review 检查标准](../../standards/story/story-review-checklist.md)（v3.9.3 已扩展 §A.bis / §D.bis / §D.ter）
- [Story 前端接口契约标准（①bis）](../../standards/story/story-frontend-contract-standard.md)
- [Story 生成标准](../../standards/story/story-generation-standard.md)（v3.9.3 已扩展 §2.5 映射表 + 10 道闸）
- [`proposal-skill.md`](../cross-cutting/proposal-skill.md)
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 文档类型 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| Story 主文档 | `ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md` | 不带版本号（原地更新）|
| Story Supplement | `ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |
| Proposal | `ae-sdd doc save --intent PROPOSAL --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |

## 第零步：准入检查（🆕 v3.9.3 新增 SSOT 化）

**输入清单统一指向 [`story-input-checklist.md`](../../standards/story/story-input-checklist.md) 单一权威源（SSOT）。**

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
[ ] C1 依赖 Story 已加载并比对字段（**Update 时尤其重要**：需确认修改不影响依赖 Story）
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

未过 → **BLOCK，禁止进入 Update**。

> 🔴 **v3.9.3 强化理由：** Update 修改字段时可能影响依赖 Story 字段对齐（§D.ter），必须重新加载并校验。

## 流程

```text
触发
  → 【v3.9.3 新增】第零步：加载 13 项输入（SSOT prose 自检 + G-STORY-CTX 门禁）
  → 读取 Proposal + Supplement
  → 读取 Story 模板
  → 校验 Proposal 可执行性
  → 按 Proposal 更新 Story
  → 【v3.9.3 新增】重新执行 §三步 bis 来源追溯与验证（确保修改后来源标注不被破坏）
  → 【v3.9.3 新增】重新执行 §A.bis / §D.ter 字段对齐（如修改了接口字段）
  → 判断是否影响 Task / DR / 模板
  → 回写 Supplement / Proposal 状态
  → 必要时触发 Task Generate 或 DR Update
```

## 核心原则

- Story Update 只执行 Proposal 覆盖的修复。
- Story Update 不重新分析问题根因。
- Story Update 不生成 Proposal。
- 如果发现新的业务语义变更，必须暂停并让上游补新 Proposal。
- **🔴 v3.9.3 新增：** Update 修改字段时必须保持来源追溯标注不被破坏（§三步 bis 重新校验）。
- **🔴 v3.9.3 新增：** Update 修改接口字段必须重新执行 §A.bis DR-Story 字段级一致性比对。

## 执行规则

1. 先读 Proposal，再读 Supplement。
2. 只修改 Proposal 覆盖的章节。
3. 字段链路、接口、数据模型、错误码一旦涉及，必须同步闭环。
4. 任何计划外修改都视为无效。
5. 修改后必须标记缺陷状态，并回写更新结果。
6. **🔴 v3.9.3 新增：** Update 修改字段时必须保持来源追溯标注不被破坏：
   - 字段类型/名称变更 → 同步更新该字段的 `📌 来源：` 标注
   - 字段来源变更 → 在 Supplement 中说明并补充 Proposal 引用
   - 新增字段 → 必须填写完整四维 + 章节级来源标注，禁止留空
   - 删除字段 → 在 Supplement 中保留删除记录（含原来源标注），便于追溯
7. **🔴 v3.9.3 新增：** Update 修改接口字段必须重新执行：
   - §三步 bis 字段来源追溯（确保四维完整）
   - §A.bis DR-Story 字段级一致性（与 DR 重新比对）
   - §D.ter 依赖 Story 字段对齐（与依赖 Story 重新比对）

## 触发下游

| 变更类型 | 是否触发下游 |
|---------|-------------|
| Task 列表、任务说明、接口字段、数据模型、字段链路、错误码变化 | 触发 Task Generate |
| DR 规则或设计缺陷被确认 | 触发 DR Update |
| 仅 AC / 验收记录变化 | 不触发 |
| 仅补充说明变化 | 不触发 |
| **🔴 v3.9.3 新增：** 字段来源标注变化 | 不触发下游，但在 Supplement 中记录 |
| **🔴 v3.9.3 新增：** 影响依赖 Story 字段对齐 | 触发对应依赖 Story 的 Update |

## 禁止事项

- 禁止读取或依赖旧版 Story Review 计划载体
- 禁止按计划外内容改 Story
- 禁止跳过 Proposal 直接修改 Story
- 禁止把 Supplement 当作修复计划
- **🔴 v3.9.3 新增：** 禁止在 Update 时破坏来源追溯标注（违反 → 必须重做 §三步 bis）
- **🔴 v3.9.3 新增：** 禁止修改字段而不更新该字段的来源标注
- **🔴 v3.9.3 新增：** 禁止跳过第零步 SSOT 自检与 G-STORY-CTX 门禁
- **🔴 v3.9.3 新增：** 禁止跳过 Update 后的 §三步 bis 与 §A.bis / §D.ter 重校验