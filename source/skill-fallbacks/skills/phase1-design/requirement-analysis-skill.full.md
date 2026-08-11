---
name: requirement-analysis
description: 需求分析 SKILL — ae-sdd 的首个业务分析阶段。只回答"需求是什么、为什么、边界是什么、如何验收、风险和未决是什么、需求规模多大"，产出唯一一份随需求规模自适应的需求规格说明书（SRS），并以纯需求事实裁定规模；RA 关闭冲突并获得用户一次批准后，daemon 才冻结唯一 EngineeringRoute。当用户说"分析需求"/"从 PRD 开始"/"需求拆解"/"需求分析"时触发。
---

# Requirement Analysis — 需求分析 SKILL（RA-first 首个业务阶段）

> **职责边界：** 本 SKILL 只做需求分析。它回答六件事——需求是什么、为什么、边界是什么、如何验收、风险和未决是什么、需求规模多大。它**不**做技术方案、不选代码路径、不裁定 TaskKind、不生成 PRD/Issue 前置产物、不生成 RA GeneratePlan/Impact/ReverseIssues/ReviewReport/下游关联矩阵等旁车。

> **唯一产物：** 每个 Work Item 只保存一份 `intent RA` 的核心文档——需求规格说明书（SRS）。已有 RA 原地修订，不创建 sidecar。用户批准不回写 SRS 正文，由 daemon 的 confirmation receipt 持有。

> **RA-first 流程位置：**
> ```text
> Initialized
>   -> 记录原始 intent、调用方已提供的引用与 provisional intake facts
>   -> 无最终 RouteDecision 启动 Requirement Analysis Series
>   -> 分离 source fact / interpretation / assumption
>   -> 写 SRS Core
>   -> 对条件维度逐项判定 applicable / not_applicable / unknown
>   -> 只加载支撑适用维度或现有系统事实所需的上下文
>   -> 只展开 applicable 与阻断性 unknown 章节
>   -> 仅针对阻断性冲突/歧义向用户提问
>   -> 校验 REQ 来源、REQ-AC 覆盖、风险和 gap
>   -> 按纯需求影响事实裁定 micro/small/medium/large + confidence
>   -> 保存唯一 `intent=RA` SRS（analysisState=complete）
>   -> 收集并验证 RA SeriesReceipt
>   -> RequirementAnalyzed [G-RA-1 -> G-RA-2 -> G-RA-3 -> G-RA-4]
>   -> RouteEngine 从已验证 RA scale + evidence + receipt 生成 route candidate
>   -> 用户一次批准 SRS 与规模
>   -> G-RA-FLOW-VIOLATION 验证 receipt/digest/scale/route binding
>   -> freeze EngineeringRoute 并进入 RouteSelected
>   -> SeriesPlanner 按冻结 Route 规划 downstream Series
> ```
>
> 退出 RA 内容分析的条件：Core 完整、适用性闭合、所有 REQ 可追溯且可验收、无 blocking gap、规模有依据。没有固定迭代次数；出现新事实、Gate finding、用户拒绝或 blocking gap 时才在同一 SRS 上进入 correction loop。

---

## 0. 与现有 SKILL 的分工

- `SKILL.md` = 流程编排（Hook 启动评估 -> RA -> daemon 冻结工程路由）
- **`requirement-analysis-skill.md`（本文件）** = 需求分析的环节内具体规则（怎么分析）
- `dr-generate-skill.md` = 架构/跨模块设计（RA 之后、按冻结 Route 触发）
- `story-generate-skill.md` = 行为契约设计（RA 之后、按冻结 Route 触发）
- `state.executionPlan` = 用户批准的执行投影；仅 micro 可用它替代独立 CodingPlan
- `coding-skill.md` = 编码实施
- [`templates/design/ra-template.md`](../../templates/design/ra-template.md) = SRS 空白模板

---

## 📦 文档存放前置调用（横切依赖）

> **强制：** 本 SKILL 产出的文档**必须通过 `ae-sdd doc save` 命令落地**，禁止手拼路径直接 Write。路径定位、版本号、STORING、.gitignore 全由代码负责（对齐 document-storage-skill.md §9 写入 SOP）。

### 写入 SOP（2 步）

1. **Write 草稿**：用 Write 工具把 SRS 内容写到 `.ae-sdd/tmp/{doc-id}-draft.md`
2. **存文档**：
   ```bash
   ae-sdd doc save \
     --intent RA \
     --doc-id {DOC-ID} \
     --content-file .ae-sdd/tmp/{doc-id}-draft.md \
   ```
   代码自动完成：resolve 推路径 -> 写核心文档 -> 更新索引与 .gitignore -> 删草稿。RA 只写当前有效核心文档，原地更新，不带版本号。

> RA 是**唯一产物**。不生成 PRD/Issue 前置，不生成 RA GeneratePlan、Impact、ReverseIssues、ReviewReport，不写 changelog 旁车。

---

## 1. 纯需求分析立场（RA 不越界做什么）

RA 只回答六件事：

1. **需求是什么**——规范性需求清单（REQ-*），每条绑定来源。
2. **为什么**——问题、目标与非目标。
3. **边界是什么**——范围 In/Out Scope，以及对每个条件维度的适用性判定。
4. **如何验收**——验收与追溯（AC-*），每条 REQ 至少被一个 AC 覆盖。
5. **风险和未决是什么**——约束、假设、冲突、风险与未决（含 GAP-*）。
6. **需求规模多大**——纯需求六维规模裁定。

**RA 越界红线（禁止）：**

- 禁止自行选择代码路径、表、索引、缓存、MQ、框架或架构方案——这些属于 DR/CodingPlan。
- 禁止生成技术方案、接口设计、数据模型设计、状态机实现、迁移脚本。
- 禁止裁定 TaskKind（由通用 `InputSource` 合同负责，不在本轮 RA 范围）。
- 技术名词**只有在用户已冻结为约束或用于引用现状时**允许出现；RA 引用现状是合法的，但不得把 AI 自行选择的技术当作需求。

> 设计泄漏判定：用户冻结的技术约束是合法需求事实，不能简单禁止所有技术名词。Gate 只检查来源和用途，不做关键词封禁。

---

## 2. 上下文规则（全局选填，结论依赖时条件必需）

> **全局选填：** 项目资产、代码、历史 RA、协议、日志、运行证据等上下文**对所有 RA 全局选填**。没有这些上下文也能做 RA——只是规模置信度会下降。

**三条规则：**

1. **全局选填**：上下文不强制加载。micro 文档可以完全不加载任何项目资产。
2. **结论依赖时条件必需**：当 RA 在 SRS 中声称某个**现有系统事实**（"当前 X 是这样实现的""Y 表已存在""接口 Z 返回 W"），对应证据（REF-*）必需；缺证据的现有系统事实声明会触发 Gate finding。
3. **未加载不得猜测**：未加载的上下文不得凭记忆或推断写成事实；要么加载证据，要么标注为假设/unknown。

> 上下文缺失本身不失败。只有 SRS 声称现有系统事实却没有对应 REF-*，或影响范围/验收/风险/规模的 unknown 未形成并关闭 GAP-* 时才失败。

---

## 3. 输入与启动

### 3.1 输入

- **原始 intent**：用户口述、PRD/Issue 引用、对话需求（必填，唯一硬输入）。
- **调用方已提供的引用**：用户提供或采纳的文档、资产路径（选填）。
- **provisional intake facts**：Hook 登记的 `BootstrapAssessment`、影响事实（选填；只是 RA 输入来源，**不是**规模或路由权威）。

> Intake 仍可做 `BootstrapAssessment`/影响事实的 provisional 记录，但不得写入 `scale`、`selectedDesign`、已批准 `routeDecision` 或 `EngineeringRoute`。provisional intake facts 不是规模或路由权威。

### 3.2 启动条件

RA 在 `Initialized` 状态即可启动，**不需要**最终 `RouteDecision`、预选 scale 或 design route。RA 完成唯一 SRS 并通过 `G-RA-1~4` 后，才由 RouteEngine 生成 route candidate。

---

## 4. 分析流程

### 第一步：分离 source fact / interpretation / assumption

把所有输入分为三类：

- **source fact**：来自原始 intent 或引用的客观事实（原文怎么说的）。
- **interpretation**：AI 对事实的解读、归纳、推断。
- **assumption**：未经证据支撑的假设。

> 这一步防止把推断当事实写进 REQ。REQ-* 只记录规范性需求（source fact + 明确 interpretation），assumption 进 §6 的假设表。

### 第二步：写 SRS Core

按模板填写固定 Core（§0~§7）：

- **§0 文档与需求身份**：schema=`ae-sdd-ra-srs/v2`、RA ID、Work Item、Revision、Analysis state、Scale、Scale confidence，以及来源与实际使用的上下文表（REF-*）。
- **§1 问题、目标与非目标**。
- **§2 范围**：In/Out Scope。
- **§3 适用性判定**：对七个条件维度逐项判定。
- **§4 需求清单**：REQ-*，每条至少绑定一个 REF-*。
- **§5 验收与追溯**：AC-*，每条 REQ 至少被一个 AC 覆盖。
- **§6 约束、假设、冲突、风险与未决**：含 GAP-*。
- **§7 规模裁定**：纯需求六维评分。

### 第三步：适用性判定（七个条件维度）

对以下七个维度逐项判定 `applicable / not_applicable / unknown`：

| 条件维度（key） | 含义 |
| --- | --- |
| `participants` | 参与方、权限与职责 |
| `scenarios` | 场景与交互 |
| `state_lifecycle` | 状态、生命周期与不变量 |
| `data_semantics` | 数据与信息语义 |
| `external_contracts` | 外部行为契约与依赖 |
| `quality_security_compliance` | 质量属性、安全与合规 |
| `compatibility_migration_operations` | 兼容、迁移与运行约束 |

**适用性规则：**

- 只有状态为 `applicable` 的维度生成对应条件章节（§8.1~§8.7）。
- `not_applicable` 只在 §3 留下有依据的判定，**不生成空章节**。
- `unknown` 若影响范围、验收、风险或规模，必须创建阻断性 `GAP-*`；非阻断 unknown 可保留并说明下游验证点。
- 关键 `unknown` 必须通过 GAP 关闭，否则 SRS 保持 `analysisState=draft`，不得进入 `RequirementAnalyzed`。

### 第四步：只加载所需上下文，只展开 applicable 章节

- 只加载支撑适用维度或现有系统事实所需的上下文（遵循 §2 全局选填规则）。
- 只展开 `applicable` 与阻断性 `unknown` 章节。
- 图（mindmap、状态图、时序图、ER 图）**不再固定必填**；只在关系复杂且比表格/文字更清晰时生成。

### 第五步：仅针对阻断性冲突/歧义提问

- 只在出现**阻断性冲突或歧义**（无法继续分析、影响范围/验收/规模）时向用户提问。
- 不做固定三轮、不做每步用户确认。
- 非阻断问题可记为 GAP/assumption 继续推进。

### 第六步：校验 REQ 来源、REQ-AC 覆盖、风险和 gap

- 每条 REQ 至少绑定一个 REF（来源可追溯）。
- 每条 REQ 至少被一个 AC 覆盖（可验收）。
- 影响范围/验收/风险/规模的 unknown 已形成并关闭 GAP。
- AC 类型允许 `example`、`property`、`invariant`、`compatibility`、`operational`，不强制 Given-When-Then。

### 第七步：纯需求规模裁定

按 §5 的纯需求六维算法裁定规模，写 §7 评分表，取最高分。

### 第八步：保存 SRS

- `analysisState=complete`（Core 完整、适用性闭合、REQ 可追溯可验收、无 blocking gap、规模有依据）。
- 通过 `ae-sdd doc save --intent RA` 保存唯一 SRS。
- 收集并验证 RA SeriesReceipt，绑定 Work Item/Series/DocumentId/version/content digest/source revision。

> 用户批准由 daemon 的 confirmation receipt 持有，**不回写 SRS 正文**。receipt 绑定 `documentId + version + contentDigest + scale + routeCandidateDigest`，避免批准后修改文档造成循环 digest。

---

## 5. 纯需求规模算法

六个维度分别评分 1-4，最终 scale 取**最高分**（避免"多个中风险平均后被降级"）：

1. **可观察行为与场景广度**
2. **参与方、权限或业务域广度**
3. **状态、数据语义与不变量复杂度**
4. **外部契约与协调范围**
5. **性能、安全、合规、可用性等质量风险**
6. **兼容、迁移、回滚和运行影响**

定义：`1=micro`、`2=small`、`3=medium`、`4=large`。

**评分红线：**

- 评分**不得引用**文件/类数量、预计人天、数据库表、中间件选型或测试实现层级——这些是实现视角，不是需求规模。
- 证据不足时**降低 confidence**，不得为了获得较小路线把 unknown 当作"无影响"。
- 规模是分析输出，不是预选 profile；RA 自己在需求充分后裁定规模，不预选、不依赖技术方案。

> scale 与评分必须一致：§0 header 的 Scale 必须等于 §7 六维的最高分对应档位。`G-RA-4` 会校验此一致性。

---

## 6. REQ/AC/REF/GAP 与 traceability 规则

### 6.1 ID 契约

- 规范性需求使用 `REQ-*`，每条至少绑定一个 `REF-*`（来源）。
- 验收使用 `AC-*`，每条 `REQ-*` 至少被一个 `AC-*` 覆盖。
- 来源引用使用 `REF-*`。
- 缺口/未决使用 `GAP-*`。

### 6.2 traceability

- `REF -> REQ`：每条 REQ 可追溯到来源。
- `REQ -> AC`：每条 REQ 可被验收。
- blocking GAP 必须关闭才能 `analysisState=complete`。

### 6.3 一次最终批准

- 只有**一次**最终 SRS + 规模批准。
- 批准由 daemon receipt 持有，不把 approval status/ref 回写进被批准的 SRS 内容。
- SRS revision 变化必须使旧 Gate receipt、approval receipt 与 route candidate 全部失效。

---

## 7. correction 与重入

- Gate failure 只触发一次基于聚合 findings 的 RA correction；修订后仅重跑 selector fingerprint 已失效的 Gate，不使用固定"三轮无新增"。
- 出现新事实、Gate finding、用户拒绝或 blocking gap 时才在同一 SRS 上进入 correction loop。
- 同一入口支持 `RequirementAnalyzed` 中因用户拒绝、新事实或 Gate finding 触发的 RA correction。
- 旧无 schema / 旧 v1 RA 在重新推进流程时返回可操作的 migration finding，由 RA correction 重新生成 SRS；禁止通过 legacy fallback 绕过新 Gate。

---

## 8. 出闸条件（何时 RA 完成）

退出 RA 内容分析的条件（全部满足）：

- [ ] Core 完整（§0~§7 结构齐全，无 placeholder、无重复 ID）
- [ ] 适用性闭合（七维均已判定；applicable 有章节；unknown 无遗留阻断 GAP）
- [ ] 所有 REQ 可追溯（有 REF）且可验收（有 AC）
- [ ] 无 blocking gap
- [ ] 规模有依据（§7 六维评分 + 最高分 = header Scale）
- [ ] `analysisState=complete`
- [ ] RA SeriesReceipt 已收集并验证

满足后进入 `RequirementAnalyzed [G-RA-1 -> G-RA-2 -> G-RA-3 -> G-RA-4]`。RouteEngine 从已验证 RA 结果生成 route candidate；用户一次批准后，`G-RA-FLOW-VIOLATION` 验证 receipt/digest/scale/route binding，通过后 freeze `EngineeringRoute`。

> `RequirementAnalyzed` 只表示 RA 内容和 receipt 已闭合，**不**表示用户已批准或路由已冻结；只有随后的 `RouteSelected` 才是最终 Route 权威起点。

---

## 9. SRS 内容合同速查

固定 Core 与条件章节的完整合同见 [`templates/design/ra-template.md`](../../templates/design/ra-template.md)。要点：

- **固定 Core**：§0 身份+来源、§1 问题/目标/非目标、§2 范围、§3 适用性判定、§4 需求清单、§5 验收与追溯、§6 约束/假设/冲突/风险/未决、§7 规模裁定。
- **条件章节**（仅 applicable 生成）：§8.1 参与方、§8.2 场景、§8.3 状态生命周期、§8.4 数据语义、§8.5 外部契约、§8.6 质量安全合规、§8.7 兼容迁移运行。
- **同一模板覆盖 micro/small/medium/large**；条件章节只由 applicability 激活，规模是分析输出而不是模板/Gate profile，不改变文档种类，也不引入按规模计算的字数、表格数或章节数门槛。

---

## 10. 常见错误（避免）

- ❌ 在 RA 里写技术方案、选型、接口设计 -> 越界，应在 DR/CodingPlan。
- ❌ 生成 PRD/Issue 作为前置 -> RA 不生成前置产物。
- ❌ 生成 RA GeneratePlan/Impact/ReverseIssues/下游关联矩阵等旁车 -> RA 只有一个 SRS。
- ❌ 把推断当 source fact 写进 REQ -> 应分离 fact/interpretation/assumption。
- ❌ 声称现有系统事实却不给 REF -> 结论依赖时证据必需。
- ❌ 用文件/类/表数量裁定规模 -> 应用纯需求六维。
- ❌ 为了小路线把 unknown 当"无影响" -> 应降低 confidence 或开 GAP。
- ❌ 把 approval 回写进 SRS 正文 -> 批准由 daemon receipt 持有。
- ❌ 固定三轮 / 每步确认 -> 只有阻断性歧义才中途提问，最终只一次批准。
