---
name: story-update
description: 根据 Proposal、Story 补充说明文档和 Story 模板更新 Story 主文档。当 Story Review、Coding 或其他渠道发现 Story 缺陷时触发，或开发者说"更新 Story"、"同步补充说明到 Story"时触发。
---

# Story Update — Story 文档更新 Skill

## 目标

根据 Proposal 中记录的修复建议更新 Story 主文档，并同步回写 Supplement、模板优化建议或 DR 反馈。

## 依赖标准

- [Story Review 检查标准](../../standards/story/story-review-checklist.md)
- [Story 前端接口契约标准（①bis）](../../standards/story/story-frontend-contract-standard.md)
- [`proposal-skill.md`](../cross-cutting/proposal-skill.md)
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 文档类型 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| Story 主文档 | `ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md` | 不带版本号（原地更新）|
| Story Supplement | `ae-sdd doc save --intent STORY_SUPPLEMENT --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |
| Proposal | `ae-sdd doc save --intent PROPOSAL --work-item {W} --story-id {S} --content-file 草稿.md` | 不带版本号 | 原地更新 |

## 流程

```text
触发
  → 读取 Proposal + Supplement
  → 读取 Story 模板
  → 校验 Proposal 可执行性
  → 按 Proposal 更新 Story
  → 判断是否影响 Task / DR / 模板
  → 回写 Supplement / Proposal 状态
  → 必要时触发 Task Generate 或 DR Update
```

## 核心原则

- Story Update 只执行 Proposal 覆盖的修复。
- Story Update 不重新分析问题根因。
- Story Update 不生成 Proposal。
- 如果发现新的业务语义变更，必须暂停并让上游补新 Proposal。

## 执行规则

1. 先读 Proposal，再读 Supplement。
2. 只修改 Proposal 覆盖的章节。
3. 字段链路、接口、数据模型、错误码一旦涉及，必须同步闭环。
4. 任何计划外修改都视为无效。
5. 修改后必须标记缺陷状态，并回写更新结果。

## 触发下游

| 变更类型 | 是否触发下游 |
|---------|-------------|
| Task 列表、任务说明、接口字段、数据模型、字段链路、错误码变化 | 触发 Task Generate |
| DR 规则或设计缺陷被确认 | 触发 DR Update |
| 仅 AC / 验收记录变化 | 不触发 |
| 仅补充说明变化 | 不触发 |

## 禁止事项

- 禁止读取或依赖旧版 Story Review 计划载体
- 禁止按计划外内容改 Story
- 禁止跳过 Proposal 直接修改 Story
- 禁止把 Supplement 当作修复计划
