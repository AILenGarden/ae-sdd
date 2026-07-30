---
name: review-loop
description: |
  Review Loop 公共协议（🆕 v3.4.3）— 所有 review/循环判定节点的公共骨架。
  统一 Review Batch v2：输入指纹、有效批次、失败分类、硬预算和 Plan-first。退出条件是一批 VALID_CLEAN 即通过（cleanTarget 恒为 1）。旧 round/dryCounter 仅作兼容投影。
  各 review SKILL（story-review / dr-review / code-review / task-generate TR / proposal / story-generate）只定义自己的检查项与 Plan 载体，loop 骨架引用本协议。
  🆕 v3.4.3 废弃"每 N 轮暂停问人"——与退出条件矛盾，且把退出权交给人违反 Loop Engineering 自评估原则。
---

# Review Loop — 公共协议（所有 review 节点的 loop 骨架）

> **🔴 核心洞察（v3.4.3）：**
> - 所有 review 节点（RA 挖掘 / Story Review / DR Review / Code Review / Task Review / Proposal 循环 / Story Generate 自检）**共享同一套 loop 骨架**：挖掘缺陷 → 判定 → 出 Plan → 修复 → 再挖掘 → … → 退出
> - 之前各 SKILL 各写各的退出条件/循环上限/暂停规则，导致**全仓阈值不一致**（1 轮/2 轮混用）+ **矛盾并存**（退出条件 vs 每 2 轮暂停）+ **安全漏洞**（task-generate TR 无循环上限）
> - 本协议统一 loop 骨架，各 review SKILL 只写"我这一节的检查项是什么 + Plan 载体用什么"
>
> **🆕 v3.5.8 扩展（2026-06-27）：** 本协议覆盖范围**向上游延伸到 RA**（requirement-analysis-skill）。RA 是全链路事实源头，之前只有"16 道闸一次性终检"而无反复挖掘闭环，导致 AI 跑一遍 16 道闸就交差、8 维度挖掘是否穷尽无收敛判据（实测案例：2026-06-27 自我修订建议书诊断的"RA 多轮挖掘流程未执行"系统性漏洞）。本次把 RA 第七步纳入本协议，与 DR Review 同等机制。

---

## 📋 核心协议（Review Batch v2，所有 review 节点必须遵守）

### 协议 0：Review Batch 是权威对象

Review 状态使用 `reviewSession`（`schemaVersion: 2`）持久化；`reviewLoop.round` / `dryCounter` 只用于兼容旧 state。每个 session 必须绑定：

| 字段 | 约束 |
| --- | --- |
| `inputFingerprint` | Story/Plan/生产代码/测试/关键报告的 canonical manifest SHA-256；输入变更使旧 clean streak 失效。 |
| `rulesetFingerprint` | reviewer 与 gate 规则版本；规则变化不得静默复用旧结论。 |
| `batches[]` | 每次派发尝试一条记录，状态只能是 `VALID_CLEAN` / `VALID_FINDINGS` / `INVALID_INFRA` / `INVALID_PROTOCOL` / `INVALID_INPUT_DRIFT` / `CANCELLED`。 |
| `counters` | 分开统计 `attempts`、`validBatches`、`cleanStreak`、`remediations`、平台/协议失败。 |
| `budgets` | `maxAttempts`、`maxValidBatches`、`maxRemediations`、`maxWallClockMinutes`，任一耗尽进入 `STALLED`，不得转为通过。 |

有效批次必须满足：required reviewer 集完整、session 独立、input/ruleset fingerprint 一致、报告格式有效。平台错误只重试失败角色，不重跑已完成角色；输入漂移必须创建新 fingerprint session。

### 协议 1：退出条件

**一批 `VALID_CLEAN` 即通过。** `cleanTarget` 恒为 1，不随 Tier 或 repair class 升档：

| Tier | 首批无缺陷 | P0/P1 修复后 |
| --- | --- | --- |
| Tier 1 | 1 个 `VALID_CLEAN` | 1 个新 fingerprint `VALID_CLEAN` |
| Tier 2 | 1 个 `VALID_CLEAN` + deterministic gates | 1 个新 fingerprint `VALID_CLEAN` + deterministic gates |
| Tier 3 | 1 个 `VALID_CLEAN` + 增量最终验证 | 1 个新 fingerprint `VALID_CLEAN` + 增量最终验证 |

`VALID_CLEAN` 只在 required reviewer 集完整时增加 `validBatches`/`cleanStreak`；`INVALID_*` 不改变 clean 结论。

风险由**两条正交防线**约束，而不是靠重复一个已经 clean 的批次：

| 防线 | 作用 |
|---|---|
| required reviewer 集 | Tier 决定必须到齐的角色（Tier 1 GENERAL；Tier 2 BE+AR；Tier 3 BE+AR+QA），缺角色即 `INVALID_PROTOCOL`，不计 clean |
| `finalProofRequirement` | Tier 2 收尾必须现场跑 `G-CODEPLAN-SRC`/`G-14`/`G-08` 且全 PASS；Tier 3 必须存在唯一一条 PASS 的增量最终验证 job（覆盖改动 crate/模块及对应测试文件；全量套件仅 release/分发门禁执行），且 digest 与 fingerprint 全部对齐 |

修复后重新挖掘仍必须换 `inputFingerprint`：改了代码就是新一代输入，旧 clean streak 失效。

### 协议 2：循环上限

所有自动循环都有 attempts、valid batches、remediations 和 wall-clock 硬预算。任一预算耗尽进入 `STALLED`，输出阻塞证据并升级用户；预算耗尽不得自动放行 P0/P1。

| 规则 | 说明 |
|---|---|
| 平台失败 | 429/超时/crash：指数退避，只重试失败角色，最多 2 次；已成功角色 verdict 可复用。 |
| 协议失败 | 报告格式错误、角色缺失、session 重复：`INVALID_PROTOCOL`，修复输入后重试。 |
| 用户中断 | `CANCELLED`；恢复时继续同一 batch，不增加 valid count。 |
| 禁止 | 禁止无限循环、把 `STALLED` 判成 PASS、把平台失败计入 clean。 |

### 协议 3：Plan-first（有确认缺陷必先出 Plan）

**任何确认缺陷必须先出 Plan，按 Plan 修复，Plan 外修改无效。**

| 规则 | 说明 |
|---|---|
| Plan 载体 | 统一用 [`proposal-skill.md`](proposal-skill.md)；Story Review / Story Update 已收敛到 Proposal-first，其他历史 UpdatePlan 仅作兼容说明，不作为运行时载体 |
| Plan-first 流程 | 挖掘缺陷 → 判定确认 → **生成 Plan** → 按 Plan 修复 → 重新挖掘（不是直接改）|
| Plan 外修改无效 | 修复时若超出 Plan 范围 → 视为无效更新，必须补 Plan 或回滚 |
| 节点专属 Plan | story-review / story-update 已统一为 Proposal；dr-review 的 DR Review UpdatePlan / code-review 的 CodeReviewUpdatePlan 为历史兼容载体，本协议要求最终都收敛到 Proposal-first |

---

## 🔴 禁止条款（v3.4.3 废弃）

### 禁止 1：禁止"每 N 轮暂停问人"

**v3.4.3 废弃**之前 story-review / dr-review / SKILL.md 主编排层的"每完成 3 轮 A-E 阶段循环自动暂停，询问用户是否继续"规则。

**废弃原因：**
1. **与退出条件矛盾**：退出条件说"无新增即退出"（信任自评估），暂停说"每 3 轮就停下问人"（不信任自评估）——同一文件内自相矛盾
2. **违反 Loop Engineering 自评估原则**：把退出权交给人，AI 无法自主决定"我审干净了可以退出"
3. **不是 common 设定**：只有 story-review/dr-review 有，code-review/task-generate/proposal 都没有，说明它本就不是公共规则

**替代方案：** 退出条件（协议 1）+ 循环上限（协议 2）已足够。AI 自评估达标即退出；预算耗尽仍有 🔴 才升级用户。人不在循环中间介入，只在异常退出（升级用户）或节点结束处的人工审核点介入。

### 禁止 2：禁止无预算循环

所有 review 节点必须同时有 attempts、valid batches、remediations 和 wall-clock 上限。无预算 = AI 可无限重试或把平台错误伪装成质量结论，是安全漏洞。

---

## 📌 各节点专属配置（本协议只管骨架，专属配置由各 SKILL 自定义）

| 节点 | 检查项/阶段定义 | Plan 载体 | 专属硬门禁 | SKILL 位置 |
|---|---|---|---|---|
| 🆕 **RA 挖掘循环**（v3.5.8）| RAModel 12 维 + 8 维度并行挖掘（A-H 阶段）+ 5 问自检；循环对象 = "缺口 + 8 维度挖掘是否穷尽"（不仅缺口维度）| RAGeneratePlan（已有，复用）| RA-G01~RA-G16（已有，复用） | [`requirement-analysis-skill.md` §第七步](../phase1-design/requirement-analysis-skill.md) |
| Story Review | A-E 5 阶段（DR-Story一致性 / AC完整性 / 业务逻辑覆盖 / 数据模型与接口契约 / 模板与约束）+ F-Stage 前端契约 | Proposal | C8 数据视角总览 | [`story-review-skill.md`](../phase1-design/story-review-skill.md) |
| DR Review | A-E 5 阶段（业务价值 / 架构合理性 / 接口契约 / 数据模型与不变量 / Story 拆分）| DR Review UpdatePlan | DR Approved 状态 | [`dr-review-skill.md`](../phase1-design/dr-review-skill.md) |
| Code Review | A-F 6 阶段（业务逻辑 / 分层职责 / DB 逻辑链 / 测试真实性 / 项目资产合规 / 跨文档引用）| CodeReviewUpdatePlan / Proposal | 7 道闸 | [`code-review-skill.md`](../phase3-review/code-review-skill.md) |
| Task Review (TR) | TR-1~TR-7（全局 Task Review 7 项）| 边界规则（Story/DR 层问题先走 Update SKILL）| 无 | [`task-generate-skill.md §5bis`](../phase2-task/task-generate-skill.md) |
| Proposal 循环 | §1~§4 四段式 + 渠道 1-7 | Proposal 本身即 Plan | 无 | [`proposal-skill.md`](proposal-skill.md) |
| Story Generate 自检 | A-G 7 阶段（业务背景 / 主流程异常 / AC / 接口契约 / 数据模型 / 实现任务映射 / 前端契约）| StoryGeneratePlan | 无 | [`story-generate-skill.md`](../phase1-design/story-generate-skill.md) |

> **⚠️ A-E 字母语义不统一是合理的：** 各 review 节点的"A-E"指代不同的检查维度（Story Review 的 A=DR-Story一致性，DR Review 的 A=业务价值，Code Review 用 A-F 6 阶段）。本协议**不强制统一字母语义**，各节点保留自己的阶段定义。引用他节点时需注明"本节 A-E 指 X，与 Y 节点的 A-E 不同"。

---

## 🔗 与其他 SKILL 的关系

| SKILL | 关系 |
|---|---|
| [`ae-sdd-skill.md`](../../SKILL.md) | 主编排层，在 §整体流程 中引用本协议作为 review loop 公共骨架 |
| [`proposal-skill.md`](proposal-skill.md) | Plan 载体（协议 3 的默认实现）|
| [`agent-orchestration-skill.md`](agent-orchestration-skill.md) | 多 reviewer 编排，其循环判定回退本协议 |
| 各 review SKILL | 引用本协议，只写专属配置 |

---

## 📖 实施历史

- **v3.4.3（2026-06-26）**：新建本协议。统一 review loop 骨架、循环预算和 Plan-first，废弃"每 N 轮暂停问人"。修复 task-generate TR 无上限漏洞。
- **v3.5.8（2026-06-27）**：覆盖范围向上游延伸到 RA。requirement-analysis-skill 第七步从"16 道闸一次性终检"重构为引用本协议（反复挖掘 + 风险策略收敛 + 预算耗尽升级用户 + 漏报升级）。补齐"RA 是全链路事实源头却无 review 闭环"的体系性缺口。
- **v3.10.1（2026-07-11）**：Review Batch v2 取代固定 round/dryCounter 作为运行时权威；新增 input/ruleset fingerprint、有效性状态、失败角色定向重试、风险策略和硬预算，旧字段只作兼容投影。
- **v3.4.3 已知缺口（留待下个 PR）**：story-review L1213 vs L1225 的 Plan-first 内部矛盾（2026-06-06 重构未清干净的遗留），本 PR 不处理。
