---
name: story-generate
description: Story 生成 SKILL - Phase 1 ① 节点的环节内具体规则。从 DR 生成完整 Story 文档，按标准输出 StoryGeneratePlan，并触发 Story Review。当用户说"生成 Story"、"写 Story"、"从 DR 生成 Story"、"Story 起草"时触发。
---

# Story Generate - 从 DR 生成 Story Skill

## 目标

从 DR、PRD、产品原型和项目资产生成完整 Story，要求内容可执行、可评审、与 DR 一致。

## 依赖标准

- [Story 生成标准](../../standards/story/story-generation-standard.md)
- [Story 前端接口契约标准（①bis）](../../standards/story/story-frontend-contract-standard.md)
- [StoryGeneratePlan 模板](../../templates/design/story-generate-plan-template.md)
- [Story 生成 Agent 任务分配卡](../../templates/design/story-writer-prompt-template.md)
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)
- [`review-loop-skill.md`](../cross-cutting/review-loop-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 输出文档 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| Story 主文档 | `ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md` | 不带版本号（原地更新）|
| Story Supplement | `ae-sdd doc finalize --intent STORY_SUPPLEMENT --story-id {S}` | 📝 手写+finalize |
| Story-WriterReport | `ae-sdd doc finalize --intent STORY_WRITER_REPORT --story-id {S}` | 📝 手写+finalize |
| Review 对比表 | `ae-sdd doc finalize --intent REVIEW_COMPARE --story-id {S}` | 📝 手写+finalize |
| StoryGeneratePlan | `ae-sdd doc finalize --intent STORY_GENERATE_PLAN --story-id {S}` | 📝 手写+finalize |

## 整体流程

```text
触发
  → 读取 DR / PRD / 产品原型 / 项目资产 / Story 模板 / 约束
  → 完成实现方案决策基线
  → 按 7 阶段生成 Story
  → 做合理性自检
  → 写入 Story
  → 生成 StoryGeneratePlan
  → 触发 Story Review
  → 根据 Review 反馈修正
  → 按 review-loop-skill.md 处理修正循环与退出
```

## 第零步：准入检查

必须读齐：

- DR
- PRD
- 产品原型
- 项目资产
- Story 模板
- 测试策略模板

任一缺失，禁止进入生成。

## 第一步：读取输入

必须提取：

- DR 业务背景、业务规则、验收标准、接口契约、数据模型、异常场景、领域模型
- PRD 业务背景、业务规则、用户场景
- 产品原型 UI 流程、状态、边界场景
- 项目资产分层、DDD 落点、命名约定、数据库规范、契约入口

## 第一步 bis：实现方案决策基线

所有非平凡实现点都必须先判断：

1. 是否复用现有能力
2. 是否参考成熟方案
3. 是否需要新建能力
4. 候选方案的五维质量如何
5. 最终能力归属在哪里

任何未完成基线判断的实现点，禁止进入接口、伪代码、Task 和第三方服务设计。

## 第二步：Story 章节生成

按 `story-generation-standard.md` 的七阶段标准生成：

- A 业务背景与核心价值
- B 主流程与异常流程
- C AC 验收标准
- D 接口契约
- E 数据模型
- F 实现任务映射
- G ①bis 前端接口契约

## 第三步：合理性自检

按 `story-generation-standard.md` §4 执行自检门禁。

## 第四步：写入 Story 文档

按 `story-template.md` 写入 Story 正文。

## 第四步 bis：生成 StoryGeneratePlan

按 `story-generate-plan-template.md` 写入 Plan。

## 第五步：触发 Story Review

写入完成后，触发 `story-review-skill.md`。

## 第六步：循环判定

Story Review 反馈后进入修正循环，规则遵守 `review-loop-skill.md`。

## 第七步：闸门校验

Story 生成必须通过 `story-generation-standard.md` §4 的 8 道闸。

## 禁止事项

- 禁止未读齐输入就开始写 Story
- 禁止跳过方案决策直接写接口、伪代码和 Task
- 禁止把示例当正文
- 禁止跳过 ①bis 六维度
- 禁止未过闸门就写入最终文档
