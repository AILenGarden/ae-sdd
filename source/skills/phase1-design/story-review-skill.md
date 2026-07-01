---
name: story-review
description: 根据 DR + PRD + 产品原型 + Story 模板审查 Story，记录缺陷、存疑项和误报，先写 Supplement，再触发 Proposal，禁止生成旧版计划载体。当开发者说"审查 Story"、"检查 Story"、"优化 Story"、"Story 缺陷挖掘"、"Story Review"时触发。
---

# Story Review — Story 缺陷挖掘 Skill

## 目标

系统化审查 Story，确认其是否与 DR、PRD、原型、模板与项目约束一致。发现确认缺陷后，不直接改 Story，而是先形成 Proposal，再由 Story Update 执行。

## 依赖标准

- [Story Review 检查标准](../../standards/story/story-review-checklist.md)
- [Story 前端接口契约标准（①bis）](../../standards/story/story-frontend-contract-standard.md)
- [`proposal-skill.md`](../cross-cutting/proposal-skill.md)
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)
- [`review-loop-skill.md`](../cross-cutting/review-loop-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 输出文档 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| Story Review 报告 | `save_doc(intent="STORY_REVIEW", storyId, version={r:N})` | 带 r{N} | 新增 |
| Story Supplement | `save_doc(intent="STORY_SUPPLEMENT", storyId)` | 不带版本号 | 原地累加 |
| Proposal | `save_doc(intent="PROPOSAL", storyId, version={N})` | 按 N 编号 | 新增 |
| 跨轮 Review 对比表 | `save_doc(intent="REVIEW_COMPARE", storyId, version="v1-to-v2")` | 带 v1-to-v2 | 新增 |

## 流程

```text
触发
  → 读取 DR / PRD / 原型 / Story / 模板 / 资产 / Supplement
  → 按 Story Review 检查标准执行 A-E + F
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

## A-E / F 检查口径

详细检查口径见 `standards/story/story-review-checklist.md`。

简化约束如下：

1. DR-Story 一致性必须可定位。
2. AC 必须完整且可测试。
3. 业务流程、异常流程和状态流转必须完整。
4. 数据模型、接口契约、字段链路必须闭环。
5. 模板与项目约束必须落地。
6. 涉及前端交互时，必须满足 ①bis 前端接口契约标准。

## 输出要求

- 必须输出 Review 报告。
- 必须回写 Supplement。
- 确认缺陷必须生成 Proposal。
- 不得输出旧版计划载体。

## 退出条件

满足 `review-loop-skill.md` 的退出条件，且 C8 数据视角已完成、Supplement 与 Proposal 状态已回写时退出。

## 禁止事项

- 禁止直接修改 Story 主文档。
- 禁止把误报当缺陷。
- 禁止在 Review 内直接生成修复计划。
- 禁止跳过 Proposal 直接触发 Story Update。
