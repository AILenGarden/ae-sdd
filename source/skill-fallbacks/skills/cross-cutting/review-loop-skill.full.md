---
name: review-loop
description: |
  Review Loop 公共协议（🆕 v3.4.3）— 所有 review/循环判定节点的公共骨架。
  统一三条核心规则：① 退出条件（连续 2 轮无新增确认缺陷）② 循环上限（2 轮仍有 🔴 → 升级用户）③ Plan-first（有确认缺陷必先出 Plan）。
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

## 📋 核心协议（三条，所有 review 节点必须遵守）

### 协议 1：退出条件

**连续 2 轮无新增确认缺陷**才退出循环。

| 规则 | 说明 |
|---|---|
| 计数器规则 | 本轮发现新确认缺陷 → 计数器归零；本轮无新增 → 计数器 +1 |
| 退出阈值 | 计数器累计到 **2**（即连续 2 轮无新增）→ 满足退出条件 |
| 缺陷定义 | "确认缺陷"= 经判定为 🔴 阻断型 / 🟠 严重型 / 🟡 一般型的缺陷（🟢 建议型不计入计数器归零，但需记录）|
| 节点专属硬门禁 | 各 review 节点可在退出条件上叠加**专属硬门禁**（如 story-review 的 C8 数据视角总览），未满足硬门禁不得退出 |

> **为什么是 2 轮？** 1 轮太容易审漏（AI 偶然一次没发现问题就退出）；2 轮在审漏风险与流程效率间取得平衡，给 AI 一次自纠错机会后再确认一轮。v3.10.1 从 3 轮降为 2 轮--实测 3 轮在 ClaudeCode/Codex 单 Story 实现 8 小时场景下过度冗余，2 轮已足够收敛缺陷且显著缩短流程。

### 协议 2：循环上限

**2 轮循环上限**。2 轮后仍有 🔴 阻断型缺陷未解决 → **升级用户决策**，不无限循环。

| 规则 | 说明 |
|---|---|
| 上限值 | **2** 轮（与退出阈值对齐）|
| 触发升级 | 第 3 轮结束仍有 🔴 阻断型缺陷 → 升级用户 |
| 升级动作 | 暂停循环，向用户输出"2 轮循环仍有 🔴：{缺陷清单}，请决策（人工介入 / 调整需求 / 放弃）" |
| 禁止 | 禁止无限循环（无上限的 review 是安全漏洞，v3.4.3 修复 task-generate TR 的此问题）|

> **退出条件 vs 循环上限的关系：**
> - 退出条件（连续 2 轮无新增）= **正常退出**，AI 自评估达标
> - 循环上限（2 轮仍有 🔴）= **异常退出**，AI 自评估不达标，交人决策
> - 两者不矛盾：正常路径走退出条件，异常路径走循环上限

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
1. **与退出条件矛盾**：退出条件说"连续 3 轮无新增才退出"（信任自评估），暂停说"每 3 轮就停下问人"（不信任自评估）——同一文件内自相矛盾
2. **违反 Loop Engineering 自评估原则**：把退出权交给人，AI 无法自主决定"我审干净了可以退出"
3. **不是 common 设定**：只有 story-review/dr-review 有，code-review/task-generate/proposal 都没有，说明它本就不是公共规则

**替代方案：** 退出条件（协议 1）+ 循环上限（协议 2）已足够。AI 自评估达标即退出；3 轮仍有 🔴 才升级用户。人不在循环中间介入，只在异常退出（升级用户）或节点结束处的人工审核点介入。

### 禁止 2：禁止无循环上限

所有 review 节点必须有循环上限（3 轮）。无上限 = AI 可无限"自评通过"，是安全漏洞。v3.4.3 修复 task-generate TR 的此问题。

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

- **v3.4.3（2026-06-26）**：新建本协议。统一退出阈值 3 轮 + 全仓加 3 轮循环上限 + 废弃"每 3 轮暂停问人"。修复 task-generate TR 无上限漏洞。
- **v3.5.8（2026-06-27）**：覆盖范围向上游延伸到 RA。requirement-analysis-skill 第七步从"16 道闸一次性终检"重构为引用本协议（反复挖掘 + 连续 3 轮无新增确认缺陷才退出 + 3 轮仍有 🔴 升级用户 + 漏报升级）。补齐"RA 是全链路事实源头却无 review 闭环"的体系性缺口。
- **v3.4.3 已知缺口（留待下个 PR）**：story-review L1213 vs L1225 的 Plan-first 内部矛盾（2026-06-06 重构未清干净的遗留），本 PR 不处理。
