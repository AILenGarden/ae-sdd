---
name: testcase-review
description: 审查 testcase-generate-skill 产出的测试用例，按 TC-1~TC-10 核对 AC、已准入风险、选择决策、最低充分层级、停止条件和预算例外，既防漏测高风险，也阻止无价值边界扩张。
---

# TestCase Review — 测试用例缺陷挖掘 Skill

## 与监管器 4 步的关系

本文件只负责 **TestCase 系列 Step 3：reviewSkill**。Step 1 调用声明、Step 2 生成、Step 4 人工审核由主流程监管器执行；Loop 次数和退出阈值遵守 `review-loop-skill.md`，本文件不重复定义。

## 目标

系统化审查测试用例文档，确认其与 Story 的 AC/契约/风险对齐，并形成有证据、无重复、可停止的最小充分组合。发现漏测或无界扩张后不直接改测试用例，先记录缺陷报告，再触发 TestCase 重新生成修复。

## 依赖标准

- [`testcase-generate-skill.md`](testcase-generate-skill.md)（产出物来源）
- [`review-loop-skill.md`](../cross-cutting/review-loop-skill.md)（循环退出协议）
- [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)

## 文档存放前置调用

写入前必须先调用 `document-storage-skill.md`，不手写路径。

| 输出文档 | API 调用 | 命名规则 | 动作 |
|---------|---------|---------|------|
| TestCase Review 报告 | `ae-sdd doc save --intent TESTCASE_REVIEW --work-item {W} --story-id {S?} --content-file 草稿.md` | 带 r{N} | 新增 |

## 第零步：TestCase Review 准入检查

必须读齐：

- Story 主文档（AC/接口/数据/异常路径，作为审查基准）
- TestCase 文档（审查对象，含缺陷假设清单）
- 项目资产（测试工具与项目约定）
- 项目约束（HTTP/DB/Mock/断言红线）

任一缺失，禁止进入 Review。

**🔴 机械门禁（v3.9.1，对齐 RA/Coding 三合一）：** prose 清单之外，必须跑：

```bash
ae-sdd gates check --only G-TESTCASE-CTX
```

`G-TESTCASE-CTX` 校验 `constraints/assets/Story` 三类上下文已读齐（注册表 `CONTEXT_GATE_REGISTRY`，复用 `document-storage-skill` 的 `get_constraints/get_assets` API + `paths.find_doc`）。未过 → **BLOCK，禁止进入 Review**。注：本门禁覆盖上下文加载维度；TC-1~TC-10 检查口径仍是 report-only。

**读取入口（v3.9.1 显式化）：**
- Story：`ae-sdd doc resolve --intent STORY --story-id {S}`
- TestCase：`ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?}`
- 项目资产/约束：`document-storage-skill.get_assets/get_constraints(projectKey)`

## 流程

```text
触发
  → 读取 Story（AC/接口/数据模型/异常路径）+ 测试用例文档（含 🆕 缺陷假设清单）
  → 按 TC-1~TC-10 检查口径逐项核对
  → 写 Review 报告（含缺陷清单）
  → 若存在缺陷，回退 testcase-generate-skill 重新生成
  → 再次 Review
  → 循环与退出遵守 review-loop-skill.md
```

## 检查口径（TC-1~TC-10）

| # | 检查项 | 通过标准 |
|---|--------|---------|
| TC-1 | AC 覆盖完整性 | 每个 AC 至少被一个用例覆盖；允许一条用例覆盖多个 AC |
| TC-2 | 风险登记完整性 | 候选有证据来源、风险等级、行为分区、独立失败机制和选择决策 |
| TC-3 | L1 最低充分性 | 纯逻辑风险优先在 L1；等价边界和异常输入不重复 |
| TC-4 | L2 接口风险 | 仅 HTTP/序列化/异常映射的独立风险有真实 HTTP 用例 |
| TC-5 | L3 数据风险 | 仅 DB 约束、事务和 SQL 的独立风险有真实 DB 用例 |
| TC-6 | L4 链路风险 | 仅跨服务/多组件的独立风险有端到端用例 |
| TC-7 | 高影响风险保护 | 安全、权限、金额、数据丢失、事务、并发、幂等和不可逆状态适用时未被预算排除 |
| TC-8 | 用例无冗余 | 同一行为分区 + 独立失败机制 + 层级证据相同即合并；仅触发条件不同不足以证明独立价值 |
| TC-9 | 用例可执行性 | 每条用例的前置条件/步骤/预期结果三要素齐全，可直接转化为测试代码 |
| **TC-10** | **测试组合价值与有界性** | 见下方专项校验 |

### TC-10 测试组合价值与有界性专项校验

1. **选择决策**：每个候选是否明确 `keep/merge/exclude/defer`，`merge` 是否指向已有覆盖？
2. **独立价值**：每个用例是否增加新的独立失败机制、控制流、契约、协议、断言或层级证据？否则必须合并或排除。
3. **局部上限**：同一 validator/错误分支是否仅一个代表；字段组合是否无笛卡尔积；状态是否按 guard/副作用分区；跨层重复是否有独立机制？
4. **停止条件**：AC、`keep` 风险、改动分支和历史回归是否已映射，剩余候选是否不再增加新价值？
5. **预算例外**：超出局部数量上限时，新增价值、执行成本、维护成本、不可合并原因和确认人是否齐全？

**不通过处理：** 按 review-loop-skill.md 记录缺陷，回退 testcase-generate-skill 修正有限风险登记、选择决策或预算例外。禁止通过补低价值用例数量修复。

## 核心原则

- TestCase Review 不直接修改测试用例文档。
- 已确认缺陷必须回退 testcase-generate-skill 重新生成（不在 Review 阶段直接补写用例）。
- 存疑项不得自动执行，必须人工确认。
- 循环与退出条件遵守 `review-loop-skill.md`，本文件不重复定义轮次阈值。

## 输出要求

- 必须输出 Review 报告，含 TC-1~TC-10 逐项通过/不通过状态。
- 不通过项必须列出缺失的 AC/风险，或无独立价值的候选、重复层级和缺失例外字段。
- TC-10 不通过时，必须指出违反的选择决策、局部上限或停止条件，不得用用例数量代替证据。

## 与 generate 有界选择的衔接

> generate 负责建立登记并做首次选择，review 独立检查选择是否漏掉高风险或保留了无增量价值候选。

| 把关点 | 位置 | 角色 |
|--------|------|------|
| generate §TC-G11 | 生成时自检 | 第一道：登记、准入、合并、停止、例外 |
| review §TC-10 | 生成后独立审查 | 第二道：反查漏测高风险与无界扩张 |

**回退机制：** review §TC-10 不通过 → 回退 generate 修正登记和最小充分组合，不在 review 阶段直接补写用例。

## 退出条件

满足 `review-loop-skill.md` 的退出条件（连续 3 轮无新增缺陷）。

## 禁止事项

- 禁止直接修改测试用例主文档。
- 禁止把"用例数量足够"或“数量越多越完整”当作通过依据。
- 禁止把误报当缺陷。
- 禁止因触发条件不同就保留相同行为分区、失败机制和断言的重复用例。
- 禁止跳过 TC-10 有界性校验；停止条件满足后继续扩张必须有预算例外。
