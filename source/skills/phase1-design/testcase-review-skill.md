---
name: testcase-review
description: 审查 testcase-generate-skill 产出的测试用例，核对 AC 覆盖率、全场景覆盖度、L1-L4 分层完整性，挖掘遗漏用例与冗余用例。当 TestCase 生成后自动触发、或开发者说"审查测试用例"、"检查测试覆盖"、"TestCase Review"时触发。
---

# TestCase Review — 测试用例缺陷挖掘 Skill

## 与监管器 4 步的关系

本文件只负责 **TestCase 系列 Step 3：reviewSkill**。Step 1 调用声明、Step 2 生成、Step 4 人工审核由主流程监管器执行；Loop 次数和退出阈值遵守 `review-loop-skill.md`，本文件不重复定义。

## 目标

系统化审查测试用例文档，确认其与 Story（AC/接口/数据模型/异常路径）完全对齐，且四层覆盖（L1/L2/L3/L4）无遗漏、无冗余。发现缺陷后不直接改测试用例，先记录缺陷报告，再触发 TestCase 重新生成修复。

## 依赖标准

- [`testcase-generate-skill.md`](testcase-generate-skill.md)（产出物来源）
- [`review-loop-skill.md`](../cross-cutting/review-loop-skill.md)（循环退出协议）
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 输出文档 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| TestCase Review 报告 | `save_doc(intent="TESTCASE_REVIEW", storyId, version={r:N})` | 带 r{N} | 新增 |

## 流程

```text
触发
  → 读取 Story（AC/接口/数据模型/异常路径）+ 测试用例文档
  → 按 TC-1~TC-9 检查口径逐项核对
  → 写 Review 报告（含缺陷清单）
  → 若存在缺陷，回退 testcase-generate-skill 重新生成
  → 再次 Review
  → 循环与退出遵守 review-loop-skill.md
```

## 检查口径（TC-1~TC-9）

| # | 检查项 | 通过标准 |
|---|--------|---------|
| TC-1 | AC 覆盖完整性 | Story 每个 AC 都有至少 1 个对应测试用例 |
| TC-2 | 全场景覆盖 | 不只覆盖 AC，主流程每一步都有用例，非仅验收点 |
| TC-3 | L1 单元测试 | 核心方法/边界值/异常入参有对应 L1 用例 |
| TC-4 | L2 接口测试 | 每个接口的正常/异常/边界响应有对应 L2 用例 |
| TC-5 | L3 数据层测试 | 涉及写库操作有 INSERT 后 SELECT 校验用例 |
| TC-6 | L4 集成/端到端测试 | 跨服务调用链路有对应集成用例 |
| TC-7 | 异常路径覆盖 | Story 列出的异常场景（超时/并发/幂等冲突等）均有用例 |
| TC-8 | 用例无冗余 | 无重复覆盖同一断言点的冗余用例 |
| TC-9 | 用例可执行性 | 每条用例的前置条件/步骤/预期结果三要素齐全，可直接转化为测试代码 |

## 核心原则

- TestCase Review 不直接修改测试用例文档。
- 已确认缺陷必须回退 testcase-generate-skill 重新生成（不在 Review 阶段直接补写用例）。
- 存疑项不得自动执行，必须人工确认。
- 循环与退出条件遵守 `review-loop-skill.md`，本文件不重复定义轮次阈值。

## 输出要求

- 必须输出 Review 报告，含 TC-1~TC-9 逐项通过/不通过状态。
- 不通过项必须列出具体缺失的 AC/场景/层级。

## 退出条件

满足 `review-loop-skill.md` 的退出条件（连续 3 轮无新增缺陷）。

## 禁止事项

- 禁止直接修改测试用例主文档。
- 禁止把"用例数量足够"当作覆盖完整的判定依据（必须逐项核对 AC/场景映射）。
- 禁止把误报当缺陷。
