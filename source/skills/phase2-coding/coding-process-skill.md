---
name: coding-process
description: |
  Task→Coding 之间的独立流程节点（v3.5.16 流程与能力分离）。严格加载 5 个上下文
  （项目约束/技术约束 CodingModel/Story/Task/项目资产）→ CodeAnalysis → 产出统一版
  CodePlan → 移交 CodingSkill.Execute。本 SKILL 只管"流程编排 + 上下文加载 + 移交"，
  不持有 CodeAnalysis 能力本体（能力由 coding-skill §④bis 提供，本 SKILL 指针引用），
  不写生产代码。当 state.phase 走到 coding-process 时触发。
---

# CodingProcess SKILL — Task→Coding 之间的流程节点（流程与能力分离）

> **🔴 核心定位（v3.5.16）：** 本 SKILL 是**流程节点**，不是**能力提供方**。
>
> - **流程职责（本 SKILL 持有）**：严格加载 5 上下文 → 调度 CodeAnalysis → 产出 CodePlan → 跑门禁 → 移交 CodingSkill.Execute
> - **能力职责（coding-skill 持有，指针引用）**：CodeAnalysis 的 5 步 SOP、CodePlan 模板/门禁定义
>
> **为什么分离：** 旧架构把"加载上下文 + 产出 CodePlan + 写代码"全塞进 coding-skill，导致流程与能力耦合——流程管理器无法独立监督"是否走完了 CodingProcess"。分离后，流程管理器通过 state.phase 校验 coding-process 已完成（产物校验），CodingSkill.Execute 只负责按 CodePlan 写代码。

---

## 📦 文档存放前置调用（横切依赖，落地前必读）

> 同 coding-skill.md §0：流程产出文档（CodePlan 等）落地前必须先调 `document-storage-skill.resolve_path()` 推导路径，禁止硬编码绝对路径。详见 [`document-storage-skill.md` §0](../cross-cutting/document-storage-skill.md)。

---

## 对外调用契约：CodingProcess.Run

**调用方：** ae-sdd 流程管理器（SKILL.md 编排层）。state.phase 从 `task-reviewed`（大/中/小链）或 `initialized`（微链）切到 `coding-process` 时触发。

**用途：** 加载 5 上下文 → CodeAnalysis → 产出统一版 CodePlan → 跑门禁 → 等用户审核点 2.5 确认 → 移交 CodingSkill.Execute。

**输入参数（5 上下文，🔴 全部必填，缺一停止）：**

| 参数 | 来源 | 加载方式 |
|------|------|---------|
| ① 项目约束文档 | `document-storage-skill.get_constraints(projectKey)` | §1.1 |
| ② 技术约束 / CodingModel | `document-storage-skill.get_thinking_engine(projectKey)` | §1.2 |
| ③ Story 文档 | 当前 Story（微任务无） | §1.3 |
| ④ Task 文档 | `resolve_path(intent="TASK", ...)`（微任务无） | §1.4 |
| ⑤ 项目资产 | `ae-sdd assets read coding --project <projectKey>` | §1.5 |

**强制执行步骤（🔴 严格按顺序，每次调用均须）：**

1. §1 加载 5 上下文（缺一 → 停止，要求补充上游信息）
2. §2 CodeAnalysis（复用 `coding-skill §④bis 实战 SOP` 5 步，指针引用）
3. §3 产出统一版 CodePlan（套 `be-coding-plan-template.md` 16 节）
4. §3 跑 G-CODEPLAN-SRC + G-14 + G-08 门禁（缺项 → 停止，回到步骤 2 补充）
5. §4 等用户审核点 2.5 确认（用户明确"确认/同意/可以开始编码"才能进 coding phase）
6. §5 移交：state.phase 切 `coding` → CodingSkill.Execute 接管

**输出物：**
- 统一版 `{STORY-ID}-CodingPlan.md`（过 G-CODEPLAN-SRC/G-14/G-08 门禁，落地路径由 `resolve_path(intent="CODING_PLAN", ...)` 推导）

**禁止项：**
- 🔴 禁止写生产代码（写代码是 CodingSkill.Execute 的职责）
- 🔴 禁止跳过任一上下文加载（5 个全必填）
- 🔴 禁止跳过门禁（G-CODEPLAN-SRC/G-14/G-08 任一未过禁止移交）
- 🔴 禁止不经用户审核点 2.5 确认就切 coding phase

---

## §1 五上下文加载（🔴 强制，缺一停止）

> **本节是 CodingProcess 的核心价值**：把"加载 5 上下文"从散落在 coding-skill 各步骤集中为一个**显式、可监督的前置门禁**。流程管理器通过 state.phase=coding-process 校验"这步已走过"。

### §1.1 项目约束文档（constraints）

调用 `document-storage-skill.get_constraints(projectKey)` 加载 9 项约束，逐项记住关键规则：

> **📍 9 项约束清单与关键规则见 [`coding-skill.md` §约束文件引用](coding-skill.md)（technology-stack / project-structure / layered-arch / code-style / api / database / security / testing / be-coding-thinking-engine）。** 本 SKILL 不重复列约束内容，复用 coding-skill 的约束引用表。

**缺失处置：** `get_constraints` 返回空或缺关键约束 → 停止，提示"项目约束未初始化，走 project-assets-update-skill 生成"。

### §1.2 技术约束 / CodingModel（thinking engine）

加载 `standards/thinking/be-coding-thinking-engine.md`（通过 `get_thinking_engine(projectKey)` 封装入口），产出本轮 **11 维 CodingModel 决策记录**。

> **📍 11 维决策表与门禁见 [`coding-skill.md` §第零步 加载 CodingModel](coding-skill.md)。** CodingProcess 产出 11 维决策（CodingSkill.Execute 阶段只复核不复产）。

**门禁：** 任一维度结论为空或"不知道" → 停止，向上游追溯补充。

### §1.3 Story 文档

读取当前 Story（微任务无 Story，跳过本步但须在 CodePlan 标注"无 Story 上下文，独立决策"）。

提取：涉及工程 / 主流程伪代码 / 实现任务映射 / 接口契约 / 数据模型 / 偏离声明。

> **📍 提取清单见 [`coding-skill.md` §第三步 阅读 Story 文档](coding-skill.md)。**

### §1.4 Task 文档（含 Task 0）

读取 Task 文档目录（微任务无 Task，跳过）。先读 Task 0（公共依赖说明），再按执行顺序逐个读 Task。

> **📍 读取流程见 [`coding-skill.md` §第四步 阅读 Task 文档](coding-skill.md)（4.1 先读 Task 0 + 4.2 按序读 Task）。**

### §1.5 项目资产

调用 `ae-sdd assets read coding --project <projectKey>`，返回 §4 DDD 内部分层落点 + §5 命名约定 + §6 工程约束。

> **📍 详细动作见 [`coding-skill.md` §④bis 实战 SOP 步骤1 读取项目资产](coding-skill.md)。**

**缺失处置：** 项目资产不存在 → 停止，走 project-assets-update-skill §3 生成。

---

## §2 CodeAnalysis（调用 CodingSkill 能力，🔴 不持有能力本体）

> **🔴 v3.5.16 定位：** CodingProcess **不持有 CodeAnalysis 能力**——它**调用 CodingSkill 的能力**（§④bis 实战 SOP）对代码做分析。CodeAnalysis 的 5 步 SOP（读资产 → Task 执行顺序编排 → 抽象 4 层分层归类 → 项目分层映射包路径 → 输出类骨架）完整定义在：
>
> **[`coding-skill.md` §④bis 实战 SOP：分层拆分 + 项目资产映射 5 步流程](coding-skill.md)**
>
> **调用关系：** CodingProcess §1 五上下文加载完成后 → **调用 CodingSkill §④bis 5 步 SOP** 执行 CodeAnalysis → 收集产出（类骨架/方法级逻辑/DB操作/外部依赖/测试映射）→ 进入 §3 出具 CodePlan。

**本 SKILL 的职责**：确保 §1 五上下文已加载后，才调用 CodingSkill 做 CodeAnalysis（这是旧架构缺失的监督点——旧架构里 CodeAnalysis 在 coding-skill 内部，上下文加载散落，无前置门禁）。

---

## §3 产出统一版 CodePlan + 跑门禁

### §3.1 产出 CodePlan

套用 `be-coding-plan-template.md` 16 节模板，把 §2 CodeAnalysis 的产出（类骨架 / 方法级逻辑 / DB 操作 / 外部依赖 / 测试映射）填入模板。

> **📍 模板见 [`templates/coding/be-coding-plan-template.md` §0-§15](../../templates/coding/be-coding-plan-template.md)（16 节模板正文 + §15 门禁自检表）。**

落地：`document-storage-skill.resolve_path(intent="CODING_PLAN", ...)` 推导路径 → 写 `{STORY-ID}-CodingPlan.md`。

### §3.2 跑门禁（🔴 全过才移交）

| 门禁 | 校验内容 | CLI |
|------|---------|-----|
| G-CODEPLAN-SRC | 关键类骨架每个新增/修改类附【已读源码：】标记，待核实清单非空则阻断 | `ae-sdd gates check --only G-CODEPLAN-SRC` |
| G-14 | CodingPlan 引用 Story + AC 对齐 + 偏离项有 Proposal | `ae-sdd gates check --only G-14` |
| G-08 | CodingPlan 14 门禁关键词全在 | `ae-sdd gates check --only G-08` |

> **📍 门禁详细判定标准见 [`coding-skill.md` §④bis G-CODEPLAN-SRC 源码核对详细判定标准](coding-skill.md) + [`SKILL.md` §🛡️ G-14 / G-CODEPLAN-SRC](../../SKILL.md)。**

**任一门禁未过 → 回 §2 补充 CodeAnalysis，重跑门禁，直至全过。**

---

## §4 用户审核点 2.5（CodingPlan 评审，🔴 强制）

CodePlan 过门禁后，**必须等用户明确确认**（"确认/同意/可以开始编码"）才能进 coding phase。

> **📍 审核点 2.5 的讲解规范见 [`SKILL.md` §📖 人工审核主动讲解规范](../../SKILL.md) + [`coding-skill.md` §📖 Code 讲解模板](coding-skill.md)。**

审核要点：复核 16 章节 + 14 条门禁 + CodingModel 决策 + 风险 Task。

**模糊回复处置：** 用户回复"好/行/可以"等模糊词 → 按 ⚠️ 处理，逐项追问确认，不得当 ✅ 通过。

---

## §5 移交 CodingSkill.Execute

用户审核点 2.5 通过后：

1. `ae-sdd state confirm --phase coding-process`（领 coding-process confirm token，供关卡3 产物校验）
2. `ae-sdd state write --phase coding`（切 coding phase）
3. 流程管理器加载 `coding-skill.md`，执行 `CodingSkill.Execute`

> **📍 CodingSkill.Execute 契约见 [`coding-skill.md` §CodingSkill.Execute](coding-skill.md)。** Execute 阶段按确认后的 CodePlan 写代码，两层监督生效（关卡3 产物校验 + stop_check 自报标记）。

---

## 与其他 SKILL 的关系

| SKILL | 关系 |
|-------|------|
| `coding-skill.md` | **能力提供方**：④bis 实战 SOP（CodeAnalysis）+ CodePlan 模板/门禁定义 + CodingSkill.Execute 契约。本 SKILL 指针引用其能力，不复制 |
| `task-generate-skill.md` | **上游**：产出 Task 文档 + Task Review 通过 → 移交本 SKILL。task-generate 不再产出 CodePlan（v3.5.16 释放职责） |
| `document-storage-skill.md` | **横切依赖**：5 上下文加载的 API（get_constraints/get_thinking_engine/assets read/resolve_path） |
| `SKILL.md` | **编排层**：流程图含 CodingProcess 节点 + 审核点 2.5 |

---

## 📖 CodeAnalysis 讲解规范（审核点 2.5 前）

> **📍 详细讲解模板复用 [`coding-skill.md` §📖 Code 讲解模板](coding-skill.md)。** 本 SKILL 在审核点 2.5 前主动讲解：5 上下文加载情况 + CodeAnalysis 关键决策（分层映射/类骨架/风险预判）+ 门禁结果。

---

## 执行清单（TodoWrite 映射）

CodingProcess.Run 执行时，必须用 TodoWrite 1:1 映射以下清单：

1. [ ] §1.1 加载项目约束（get_constraints）
2. [ ] §1.2 加载技术约束 + 产出 11 维 CodingModel 决策
3. [ ] §1.3 加载 Story 文档（微任务跳过）
4. [ ] §1.4 加载 Task 文档（微任务跳过）
5. [ ] §1.5 加载项目资产（assets read coding）
6. [ ] §2 CodeAnalysis（④bis 5 步 SOP）
7. [ ] §3.1 产出统一版 CodePlan（套模板 16 节）
8. [ ] §3.2 跑 G-CODEPLAN-SRC + G-14 + G-08 门禁
9. [ ] §4 用户审核点 2.5 确认
10. [ ] §5 移交：confirm coding-process + 切 coding phase

**🔴 禁止裸 ✅：** 每项完成必须附客观证据（命令输出/文件路径/门禁报告），遵循 [`code-review-skill.md` §闸4 禁裸✅](../phase3-review/code-review-skill.md)。
