---
name: coding-process
description: |
  Story/TestCase->CodingPlan->Coding 全流程编排节点（v3.10.0 砍 Task 后）。持有
  CodePlan->Coding->验证->异常追溯全流程编排：加载4上下文 -> 骨架分解 -> 调用 coding-skill
  能力库做 CodeAnalysis -> 出具 CodePlan -> 按 CodePlan 写代码(Execute) -> 编译/测试/异常追溯。
  本 SKILL 是流程节点，调 coding-skill 能力库（如何写对代码的知识），不持有能力本体。
  当 state.phase 走到 coding-process 或 coding 时触发；用户说“开始 Coding/写代码/实现 Story/
---

# CodingProcess - CodePlan->Coding 全流程编排节点（流程与能力分离）

> **🔴 v3.10.0 定位（砍 Task 后全流程化）：** 本 SKILL 持有 **CodePlan->Coding 全流程编排**。
> - **流程职责（本 SKILL 持有）**：加载上下文 -> 骨架分解 -> 调能力做 CodeAnalysis -> 出 CodePlan -> Execute 写代码 -> 验证 -> 异常追溯
> - **能力职责（[`coding-skill.md`](coding-skill.md) 持有，本 SKILL 调用）**：如何写对代码的知识（决策方法/骨架展开/分层红线/CodeAnalysis 方法论/检查清单/验证判定标准）
>
> **与 v3.5.17 的差异：** v3.10.0 砍 Task phase，原 task-generate-skill 的骨架分解（类名/包路径/方法签名/≤10行伪代码）合并进本 SKILL §A1.5，CodingPlan 不再依赖 Task 文档作为输入。

---

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 涉及的所有输入/输出文档（Story / Task / TestCase / CodingPlan / CodingReport / 开发问题记录）在读写前**必须通过 `ae-sdd doc save/resolve` 命令定位/落地**，禁止手拼路径。路径定位、版本号、STORING、.gitignore 全由代码负责（对齐 document-storage-skill.md §9 写入 SOP）。

**本 SKILL 涉及文档类型与命令对应：**

| 文档类型 | 用途 | 命令 | 版本策略 |
|---------|------|------|---------|
| Story 文档 | 读取 | `ae-sdd doc resolve --intent STORY --story-id {S}` | 不带版本号（读取上游产出）|
| 测试用例 | 读取 | `ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?}` | 不带版本号 |
| 统一版 CodingPlan | 写入 | `ae-sdd doc save --intent CODING_PLAN --work-item {W} --story-id {S?} --content-file 草稿.md` | 不带版本号（原地更新）|
| CodingReport | 写入 | `ae-sdd doc save --intent CODING_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md` | 原地更新（不带版本号）|
| Test 报告 | 写入 | `ae-sdd doc save --intent TEST_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md` | 原地更新（不带版本号）|
| 开发问题记录 | 写入 | `ae-sdd doc save --intent CODING_ISSUE_LOG --work-item {W} --story-id {S?} --content-file 草稿.md` | 不带版本号 |

> **注：** 读取用途用 `ae-sdd doc resolve`（只推路径不写）；写入用途用 `ae-sdd doc save`（一步到位）。`{W}` 为 WorkItem ID，`{S}` 为可选 Story ID。

---

## 🧠 阶段记忆强制调用（🔴 横切依赖）

> **🔴 强制：** CodingPlan 与 Coding 执行都必须使用 ae-sdd 阶段记忆。禁止只凭 Agent 对话上下文继续实现。

**CodeAnalysis 阶段（产出 CodePlan）：**
```bash
ae-sdd memory enter --phase coding-plan --story <STORY-ID>
ae-sdd memory write --phase coding-plan --story <STORY-ID> --kind decision --summary "<架构决策/风险预判/复用决策/资源边界>"
ae-sdd memory exit --phase coding-plan --story <STORY-ID>
```

**Execute 阶段（写代码）：**
```bash
ae-sdd memory enter --phase coding --story <STORY-ID>
ae-sdd memory write --phase coding --story <STORY-ID> --kind finding --summary "<实现结果/失败修复/测试证据/残余风险>"
ae-sdd memory exit --phase coding --story <STORY-ID>
```

`memory exit` 未通过 = 当前节点未完成，禁止进入 CodingReport / CodeReview。

---

## CodingProcess.Run 对外契约

**调用方：** ae-sdd 流程管理器（SKILL.md 编排层）。state.phase 从 `testcase-reviewed`（大/中链）或 `initialized`（小/微链）切到 `coding-process` 时触发。

**用途：** 加载4上下文 -> 骨架分解 -> CodeAnalysis -> 出 CodePlan -> Execute 写代码 -> 验证 -> 异常追溯。

**输入参数（4 上下文，🔴 全部必填，缺一停止；均通过 `document-storage-skill` 统一定位）：**

| 参数 | 来源 |
|------|------|
| ① 项目约束文档 | `document-storage-skill.get_constraints(projectKey)` |
| ② 技术约束 / CodingModel | `document-storage-skill.get_thinking_engine(projectKey)` |
| ③ Story 文档 | `ae-sdd doc resolve --intent STORY --story-id {S}`（微任务无）|
| ④ TestCase 文档 | `ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?}`（微任务无）|

> 项目资产不再是独立上下文——已并入 §A2 调用 `coding-skill` 能力时的内部步骤（要素1：读取项目资产），避免与 CodeAnalysis 方法论重复声明。

**全流程（Phase A → B → C）：**
- **Phase A：CodeAnalysis**（§A）→ 产出 CodePlan + 用户审核点 2.5
- **Phase B：Execute**（§B）→ 按 CodePlan 写代码 + 编译/测试验证
- **Phase C：异常追溯**（§C）→ 报错时实时追溯链 A1-A6

**输出物：** 生产代码 + 测试代码 + CodingPlan + Coding 报告（coding-report-skill 产出）

**禁止项：**
- 🔴 禁止跳过任一上下文加载（4 个全必填）
- 🔴 禁止绕过 CodePlan 自行设计方案（Execute 必须按确认后的 CodePlan）
- 🔴 禁止跳过门禁（G-CODEPLAN-SRC/G-14/G-08 任一未过禁止 Execute）

---

## Phase A：CodeAnalysis（产出 CodePlan）

### §A1 加载 4 上下文（🔴 强制，缺一停止）

> **本节是 CodingProcess 的核心价值**：把"加载 4 上下文"集中为显式、可监督的前置门禁。流程管理器通过 state.phase=coding-process 校验"这步已走过"。

| 上下文 | 加载方式 | 缺失处置 |
|--------|---------|---------|
| ① 项目约束 | `document-storage-skill.get_constraints(projectKey)` 返回 9 项约束（清单见 [`coding-skill.md` §2](coding-skill.md)） | 空/缺关键约束 → 停止，走 project-assets-update-skill 生成 |
| ② 技术约束 CodingModel | `document-storage-skill.get_thinking_engine(projectKey)`，产出 11 维决策（决策表见 [`coding-skill.md` §1](coding-skill.md)） | 任一维度结论空 → 停止，向上游追溯 |
| ③ Story 文档 | `ae-sdd doc resolve --intent STORY --story-id {S}` 定位后读取，提取涉及工程/主流程伪代码/实现任务映射/接口契约/数据模型/偏离声明（微任务跳过，须标"无 Story 上下文，独立决策"） | — |
| ④ TestCase 文档 | `ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?}` 定位后读取，提取场景清单/测试分层/预期输入输出（微任务跳过） | — |

> 项目资产读取已下沉到 §A2 调用 coding-skill 能力的内部步骤（要素1），不在本表重复列为独立上下文。

### §A1.5 骨架分解（🆕 v3.10.0 从 task-generate-skill 合并）

> **🔴 v3.10.0：** 原 task-generate-skill 的骨架生成职责已合并到本节。CodingProcess 不再依赖 Task 文档作为输入，而是直接从 Story+TestCase 派生实现骨架。

**骨架分解要求（原 task-generate §4.2）：**
1. 从 Story 接口契约/数据模型 + TestCase 场景清单，分解出原子实现单元
2. 每个单元产出骨架：类名 / 包路径 / 方法签名 / 注解 / 方法伪代码（≤10 行）
3. 方法伪代码每步动词开头（校验/查询/转换/调用/返回），不写完整条件判断/循环/try-catch
4. **禁止写完整方法体**：条件分支、异常处理、循环体均由 Execute 阶段（§B）填充
5. 识别公共依赖（Task 0 等效：公共包路径/技术栈/DO 定义），作为骨架分解的第 0 项

**骨架分解产出**：直接作为 §A2 CodeAnalysis 的输入（替代原 Task 文档），不再单独落地为 Task 文档。

### §A2 调用 coding-skill 能力做 CodeAnalysis

> **🔴 能力本体在 [`coding-skill.md` §5 §6 §7](coding-skill.md)**（④bis CodePlan 输出 + 实战 SOP 方法论 + G-CODEPLAN-SRC 判定）。本 SKILL 调用这些能力，不重写。

**CodeAnalysis 调用流程：**
1. 调用 [`coding-skill.md` §6 ④bis 实战 SOP](coding-skill.md)：读取项目资产 -> 基于 §A1.5 骨架编排执行顺序 -> 按 4 层分层归类 -> 映射包路径 -> 输出完整类骨架
2. 按 [`coding-skill.md` §5 CodePlan 7 章节](coding-skill.md) 填充：文件级顺序/类骨架/DO字段/SQL/测试映射/验证点/调试回滚
3. 遵循 [`coding-skill.md` §3 分层职责红线](coding-skill.md) + [§6 的 7 条禁令](coding-skill.md)

### §A3 产出统一版 CodePlan + 跑门禁

套用 [`be-coding-plan-template.md` §0-§15](../../templates/coding/be-coding-plan-template.md) 16 节模板，Write 草稿后用 `ae-sdd doc save --intent CODING_PLAN --work-item {W} --story-id {S?} --content-file 草稿.md` 落地。

**跑门禁（🔴 全过才进 Execute）：**

| 门禁 | CLI |
|------|-----|
| G-CODEPLAN-SRC | `ae-sdd gates check --only G-CODEPLAN-SRC`（判定标准见 [`coding-skill.md` §7](coding-skill.md)）|
| G-14 | `ae-sdd gates check --only G-14` |
| G-08 | `ae-sdd gates check --only G-08` |

任一未过 → 回 §A2 补充 CodeAnalysis，重跑门禁，直至全过。

### §A4 用户审核点 2.5（CodingPlan 评审，🔴 强制）

CodePlan 过门禁后，**必须等用户明确确认**（"确认/同意/可以开始编码"）才能进 Execute。

> **📍 讲解规范见 [`SKILL.md` §📖 人工审核主动讲解规范](../../SKILL.md)。** 复核 16 章节 + 14 条门禁 + CodingModel 决策 + 风险 Task。

**模糊回复处置：** 用户回复"好/行/可以"等模糊词 → 按 ⚠️ 处理，逐项追问确认，不得当 ✅ 通过。

用户审核通过后：
1. `ae-sdd state confirm --phase coding-process`（领 coding-process confirm token，供关卡3 硬层校验）
2. `ae-sdd state write --phase coding`（切 coding phase）
3. 进入 Phase B Execute

---

## Phase B：Execute（按 CodePlan 写代码）

### §B0 第零步：复核 CodingModel（Execute 入口门禁）

> **复核（不重新产出）：** 11 维 CodingModel 决策由 Phase A 产出（[`coding-skill.md` §1](coding-skill.md)），Execute 阶段只验证与 CodePlan 一致。任一冲突立即停止追溯。

### §B1 工程预检（第一~五步合并）

**收集输入：** 复核 §A1 已加载的 4 上下文（不重新加载，已在 CodeAnalysis 阶段通过 `document-storage-skill` 定位）：

- 约束文档：`get_constraints(projectKey)` 返回的 9 项（关键规则见 [`coding-skill.md` §2](coding-skill.md)）
- Story 文档：`ae-sdd doc resolve --intent STORY --story-id {S}`，提取涉及工程/主流程伪代码分层骨架/实现任务映射/接口契约/数据模型/偏离声明
- 骨架分解（§A1.5）：CodingPlan 骨架（类名/包路径/方法签名/伪代码），替代原 Task 文档
- 测试用例文档：`ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?}`，提取场景清单/测试分层/Mock点/预期输入输出/错误码断言

**工程预检 4 项：**
1. **确认工程存在**：每个涉及工程查磁盘路径，不存在则创建子模块注册父 pom
2. **检查依赖完整性**：每个工程 pom.xml 检查必要依赖（Domain/Application/Infrastructure/Interfaces 各自必须依赖）
3. **验证第三方 SDK 包路径**：从本地 Maven 仓库 jar 解压确认实际类路径
4. **确认已有代码模式**：扫描已有 Java 文件，确认 Result/@NotBlank/@SkipAuth 包路径 + Converter/Repository/Controller 风格

### §B2 按骨架顺序生成代码（调 coding-skill 骨架展开能力）

> **🔴 严格按 §A1.5 骨架执行顺序生成。每完成一个实现单元对照检查项自检。单元间依赖不清晰或循环依赖 -> 先问用户确认执行顺序。**

**任务规模 × 文档组合：**

| 任务规模 | 生成依据 | 流程深度 |
|---------|---------|---------|
| **大任务** | Story + TestCase + CodingPlan（含骨架分解）| 全流程 |
| **中任务** | Story + TestCase + CodingPlan | 全流程 |
| **小/微任务** | CodingPlan（骨架分解，微任务无 Story/TestCase）| 全流程 |

**生成规则：**
- **骨架填肉，按序展开**：CodingPlan §A1.5 骨架给方法签名+伪代码，按 [`coding-skill.md` §4 骨架展开规则](coding-skill.md) "填肉"，不自行发挥结构
- 包路径固定（用 §A1.5 公共依赖定义，不可自行命名）
- 字段类型严格（以 CodingPlan 骨架为准，特别注意 String vs Long）
- 约束优先（约束文档规则优先于 CodingPlan 骨架示例）
- 🔴 分层职责归位：写每个方法前先问"这段属于哪层"，严禁串味（判定标准见 [`coding-skill.md` §3](coding-skill.md)）

**每个实现单元生成流程：**
```
0. 🔍 实现方案确认（必须通过，见 §B3）
1. 按类骨架创建文件（类签名 + 注解 + 字段）
2. 确认前置实现单元已完成（依赖类存在可导入）
3. 按骨架展开规则（coding-skill §4）逐方法填肉
4. 对照约束检查章节逐项自查
5. 对照验收映射验证 AC 覆盖
6. 标记实现单元完成
```

### §B3 实现方案确认（强制交互节点）

> **每个实现单元开始前必须向用户呈现并确认实现方案，用户确认后才可开始写代码。未确认禁止写代码。**
> **本节内容来自 CodingPlan §A1.5 骨架 + §A2 CodeAnalysis 产出，不重新分析。CodingModel 决策已在 Phase A 完成。**

**向用户呈现的内容：**

```
【实现单元 {N} 方案确认】

实现单元名称：{单元名称}
涉及工程/层：{Domain / Application / Infrastructure / Interfaces}

零、CodingModel 决策复核（来源：CodingPlan ## CodingModel 决策记录，只读不改）
┌─────────────────────────────────────────────────────────────┐
│ ① 原子性：{结论}   方案：{处理方案}                           │
│ ② 并发安全：{结论}   方案：{方案}                             │
│ ...（11 维，见 coding-skill §1）                              │
│ 复核结论：与统一版 CodingPlan 一致 ✅ / 发现冲突 ❌→停止追溯  │
└─────────────────────────────────────────────────────────────┘

一、类与包路径（来源：## 任务级 CodePlan → 类骨架）
二、依赖工具包（来源：## 任务级 CodePlan → 依赖工具包）
三、方法实现计划（按 coding-skill §4 骨架展开规则展开）
四、DB 操作（表/操作/WHERE/幂等键/事务边界/并发控制）
五、外部依赖（接口/超时/重试/降级/是否阻塞）
六、测试用例覆盖（TestCase ID/场景/真实DB/HTTP）
七、基准过滤器自检（coding-skill §11.3，7 项）
```

**用户选项：**
- ✅ 确认实现方案，无误
- ⚠️ 需要调整（说明修改内容，AI 记录后等待再次确认）
- ❌ 暂停本 Task

> **强制：必须等待用户确认后才能开始写本 Task 的代码。同一 Task 最多等待 3 次修改意见，3 次后强制暂停。确认语义：用户必须说"确认"/"同意"/"可以开始"，模糊回复需追问。**

### §B4 编译 + 服务启动 + 接口验证 + DB 验证 + 异常验证

> **核心原则：`mvn compile` 通过 ≠ 完成。必须逐维验证全部通过（判定标准见 [`coding-skill.md` §8](coding-skill.md)）。**

**逐工程编译：**
```bash
cd {parent-project-root} && mvn compile
```
> 🔴 必须在父工程根目录执行，不允许只编译子模块（会漏跨模块依赖问题）。

**服务启动验证：** `mvn spring-boot:run` → health 端点 UP + Bean 已注册 + 无 BeanCreationException（失败处理见 [`coding-skill.md` §8.2](coding-skill.md)，必须定位根因禁止绕过）。

**主流程接口测试：** `mvn test -Dtest=*ApiIT,*ControllerIT`（🔴 能走真实 HTTP 必须走真实 HTTP，MockMvc 仅降级）。

**错误码映射验证：** 异常场景测试返回正确 HTTP 状态码 + 错误码（参数空/状态非法/未登录/内部异常）。

**DB 写操作落库验证：** `mvn test -Dtest=*IntegrationTest`（INSERT 后可查/UPDATE 字段正确/逻辑删除生效/事务可见）。

**事务边界验证：** 事务内失败无污染/事务外操作不阻塞/调用链与 Story 一致。

**所有测试 Pass：** `mvn test` → BUILD SUCCESS。

### §B5 Test 系列交接

CodingProcess 不再内嵌测试运行细则。代码落地后交给 Test 系列：

| 子步骤 | SKILL | 职责 |
|---|---|---|
| Generate | `../phase3-review/test-generate-skill.md` | 编译、启动、运行 L1/L2/L3/L4 测试，生成 `TEST_REPORT` |
| Review | `../phase3-review/test-review-skill.md` | `test-verifier` 独立复核证据链、XML 对账、G-09/G-10 |

Test 系列未通过时，按 `test-review-skill.md` 的缺陷分类回到 Test Generate / Coding / Story-CodingPlan 节点；CodingProcess 不得用“测试通过”口头替代测试报告。

### §B6 编码后全切面一致性核查闸（CodeReview 硬前置）

> **每轮 Coding 完成后立即强制执行**（含缺陷修复轮），不可因"只改了一点"跳过。触发时机：代码写完立即跑，跑完后再跑闸7静态扫描、编译测试，最后才进 CodeReview。

**核查方向：以实际代码为锚点反向核查**（不是拿文档找代码）。**范围：🔴 全章节全文件，禁止只核当轮 diff。**

**漂移 4 级判定 + 核心落库真实 DB 硬门禁：** 见 [`coding-skill.md` §9.1](coding-skill.md)。

**出闸条件：** 核查表覆盖全部代码改动 + 无 🔴 漂移 + 核心落库路径有真实 DB 证据。未达标 → 回 §B2 修复，不允许进入 CodeReview。

### §B7 静态扫描 + 交付表（编码收尾）

**闸 7 静态扫描：** 通用 grep 命令见 [`coding-skill.md` §12](coding-skill.md)（标准库全限定名/未使用 import/静态导入滥用）。工程特定扫描由项目资产 §6.11 配置。

**闸 4 编码后交付表（🔴 每次写完代码必须输出）：**
- 必填列：类型 + 文件路径 + 变更类型 + 说明
- 变更类型枚举：`新增/修改/删除/无改动/仅测试/仅文档`
- 调用顺序：按项目分层架构（SPI → Domain → Application → Infrastructure → Interfaces/BFF → Test → 文档/配置，具体见项目资产 §3）
- 🔴 TodoWrite 所有 todo 标 completed 后，最终汇报第一项必须是交付表

**闸 6 SKILL 级联更新：** Story 文档变更（新增 AC/改主流程/改接口契约/改数据模型）→ 立即检查受影响 CodingPlan 骨架并同步更新，不中断流程询问用户；级联后重启 §A1.5 骨架分解 + §A2 CodeAnalysis。

---

## Phase C：异常追溯（报错时实时追溯链 A1-A6）

> **核心立场：** Coding 出现错误时，**先排除文档缺陷，再判定 AI 自己犯蠢**。文档缺陷（Task/Story/DR）必须先改文档、再改代码；只有文档都没问题时才允许直接改代码。

**4 层根因分类判定标准：** 见 [`coding-skill.md` §10](coding-skill.md)。

**🔴 强制顺序（5 步，命中即处理，不可跳层）：**

```
发现 Coding 报错（编译失败/单测失败/接口失败/真实HTTP失败/SQL错误/事务回滚/性能问题）
    │
    ▼
追溯层 1️⃣：先读 CodingPlan 骨架（§A1.5）-> 判定骨架是否写错/写漏/与 Story/AC 矛盾？
    ├── 🔴 骨架有错 -> 修 CodingPlan（=重新 §A1.5+§A2）-> 重新 CodeAnalysis -> 重新 Coding
    └── ✅ Task 无误 → 进入追溯层 2
    │
    ▼
追溯层 2️⃣：再读 Story 文档 → 判定 Story 是否写错/写漏/AC 与 DR 矛盾？
    ├── 🔴 Story 有错 -> 写 Supplement -> Story Update -> 重新 §A1.5 骨架分解 -> 重新 Coding
    └── ✅ Story 无误 → 进入追溯层 3
    │
    ▼
追溯层 3️⃣：照例检查 DR → 判定 DR 业务规则是否有漏洞/边界遗漏/与 PRD 矛盾？
    ├── 🔴 DR 有错 → 写 DR 补充 → DR Update → 通知受影响 Story → 重新 Review/Generate/Coding
    └── ✅ DR 无误 → 进入追溯层 4
    │
    ▼
追溯层 4️⃣：判定"AI 自己犯蠢"（兜底，前提：层 1/2/3 全部无误）
    ├── ✅ 纯实现问题（typo/漏 import/笔误/API 误用）→ 写问题记录 → 直接修代码 → 继续 Coding
    └── ❌ 不是纯实现问题 → 升级用户决策
```

**🔴 关键约束：**
- **禁止跳过追溯层 1/2/3 直接判定"AI 犯蠢"**——历史上反复多轮返工的根源正是跳层判定
- **每层判定都要写入问题记录的"根因分析"字段**——不允许"自我声明无误"
- **命中设计层（1/2/3）必须先改文档、再改代码**——顺序不可颠倒
- **追溯是"每次报错实时"，不是"事后一次性"**

### §C1 问题记录载体

> 实时追溯的结果必须落到问题记录，供后续审计/复盘。用 `ae-sdd doc save --intent CODING_ISSUE_LOG --work-item {W} --story-id {S?} --content-file 草稿.md` 写入 `ae-sdd-doc/Coding/{WORKITEM-ID}/`。

**每条问题格式：**
```markdown
### 问题 {序号}：{问题标题}
- **发现时间：** {YYYY-MM-DD HH:mm}
- **所属 Task：** Task {ID}
- **报错类型：** 编译失败/单测失败/接口失败/真实HTTP失败/SQL错误/事务回滚/性能问题
- **追溯层 1（Task）：** □ Task 设计有错 / ✅ Task 设计无误
- **追溯层 2（Story）：** □ Story 设计有错 / ✅ Story 设计无误
- **追溯层 3（DR）：** □ DR 设计有错 / ✅ DR 设计无误
- **追溯层 4（AI 犯蠢）：** ✅ 纯实现问题（前 3 层均已排除）/ ❌ 异常场景（升级用户）
- **根因分析：** {为什么会出现这个问题}
- **影响范围：** 约束文档/Task/Story/DR/仅代码
- **修复方案：** {计划如何修复}
- **状态：** Open / Fixed
```

### §C2 与流程衔接

| 触发时机 | 进入追溯链 |
|---------|-----------|
| §B4 编译/启动/接口/DB 验证任意失败 | 立即进入 |
| §B5 单测/集成测试失败 | 立即进入 |
| §B5 测试报告合规校验发现测试无效 | 立即进入 |
| §B2 实现过程中发现 Story/Task 与实际不符 | 主动进入（不等编译/测试失败） |

---

## 📖 人工审核主动讲解规范 — Code 节点（审核点 4）

> **Code Review 阶段（🔍 人工审核点 4）的主动讲解规范。**

**AI 必须主动讲解的内容：**

| 维度 | 必须讲清楚 |
|------|-----------|
| 代码实现故事 | 实际代码怎么把 Story 落到位？用 walkthrough 带用户走主流程调用链 |
| 分层 walkthrough | Domain/Application/Infrastructure/Interfaces 各层核心类做什么、关键方法签名、关键代码在第几行 |
| 状态机实现 | canTransition() 实际怎么写？状态流转代码长什么样？条件校验在哪一行？ |
| 事务实现 | `@Transactional` 边界在哪个方法？事务传播行为？回滚规则？ |
| 异常处理 | 核心异常怎么抛？怎么捕获？错误码怎么映射？HTTP 状态码？ |
| 测试覆盖 | 哪些 AC 已被测试覆盖？测试方法名？覆盖率？ |
| CodeReview 发现 | 阻塞型/严重型问题有哪些？整改方案？ |

**输出模板：**
```
📖 【Code 讲解 - {STORY-ID} v{N}-r{M}】

【代码实现故事】
本轮 Coding 实现了 {Task-1, Task-2, ...} 共 {N} 个 Task。
调用链路：用户{操作} → {Controller} → {AppService} → {DomainService} → {Repository} → DB

【Domain 层 walkthrough】
- {XxxAggregate}：{职责}，核心方法 {method1, method2}
- 关键代码：{文件}:{行号} → {代码片段或逻辑描述}

【Application 层 walkthrough】
- {XxxAppService}：{职责}，编排逻辑：{调用链}

【事务边界】
- {XxxAppService.transition()}：@Transactional 边界 = {包含哪些操作}
- 回滚规则：{触发条件}

【异常处理】
- {XxxException}：{抛出条件} → 错误码 {code} → HTTP {status}

【测试覆盖】
- AC-001：测试方法 {TestClass.testXxx} ✓
- 覆盖率：{数字}%

【CodeReview 关键发现】
- 🔴 阻断：{数量} 个 → 整改方案
- 🟠 严重：{数量} 个 → 整改方案
```

**门禁：**
- 🔴 未输出 `📖 【Code 讲解 - ...】` → 视为跳过人工审核点 4 → 禁止进入完成判定
- 🔴 关键代码未附文件:行号 → 视为讲解无证据
- 🔴 CodeReview 问题未给整改方案 → 视为报告不完整

---

## 执行清单（TodoWrite 1:1 映射，禁止跳过）

> **强制：AI 启动本 SKILL 时，必须用 TodoWrite 1:1 映射此表。每完成一行，验证"产出物已生成 + 门禁已满足"后才进下一步。**

| # | 动作 | 产出物 | 门禁 |
|---|------|--------|------|
| **Phase A：CodeAnalysis** | | | |
| A1 | 加载4上下文（项目约束/技术约束/Story/TestCase，均经 document-storage-skill 定位） | - | 4 上下文齐备 |
| A2 | 调 coding-skill 能力做 CodeAnalysis | 类骨架/分层映射 | 分层归类无错（coding-skill §3） |
| A3 | 产出统一版 CodePlan + 跑门禁 | {STORY-ID}-CodingPlan.md | G-CODEPLAN-SRC/G-14/G-08 全过 |
| A4 | 用户审核点 2.5 确认 | 用户确认记录 | 用户明确"确认/同意/可以开始" |
| **Phase B：Execute** | | | |
| B0 | 复核 CodingModel（不重新产出） | — | 与 CodePlan 一致 |
| B1 | 工程预检（4 项） | — | 工程/依赖/SDK/代码模式确认 |
| B2 | 按 Task 顺序生成代码 | 源代码文件 | 每个 Task 检查项通过 |
| B3 | 每个 Task 实现方案确认（强制节点） | 用户确认记录 | 用户已确认本 Task 方案 |
| B4 | 编译+启动+接口+DB+异常验证 | — | mvn compile + 启动 + 接口 Pass（coding-skill §8）|
| B5 | 运行测试 + 测试报告合规校验 | 测试报告 + 证据链 | 扫描 BLOCKER=0；XML 一致；无假修复 |
| B6 | 编码后全切面一致性核查闸 | 《一致性核查表》 | 无 🔴 漂移；核心落库真实 DB 证据 |
| B7 | 静态扫描 + 交付表 | 扫描结果 + 交付表 | 通用扫描 0 命中；交付表按调用顺序 |
| **Phase C：异常追溯（任意步骤可触发）** | | | |
| C | 报错走追溯链 A1-A6 | {story}-开发问题记录.md | 4 层逐层判定，不跳层 |

**异常路径（任意步骤可触发）：**

| 触发条件 | 动作 | 产出物 |
|---------|------|--------|
| 发现问题（编译错误/设计矛盾/约束遗漏/启动失败） | 立即记录问题 + 走追溯链 | 开发问题记录更新 |
| Story 缺陷 | 更新 Supplement → 触发 Story Update | Supplement 更新 |

---

## 完成标准

**CodingProcess 的完成需同时满足：**

| 条件 | 说明 |
|------|------|
| ✅ 编译通过 | `mvn compile` 无错误 |
| ✅ 测试全部通过 | `mvn test` 所有用例 Pass |
| ✅ 问题反馈闭环 | 异常路径中无 Open 状态的问题 |
| ✅ 测试报告已出具 | 最终一轮测试报告已生成 |
| ✅ 全切面一致性核查闸通过 | 无 🔴 漂移，核心落库路径有真实 DB 证据 |

**不满足时：** 循环执行"修复 → 编译 → 测试 → 出报告"，直到全部通过。

---

## 与其他 SKILL 的关系

| SKILL | 关系 |
|-------|------|
| `coding-skill.md` | **能力库（被调用）**：CodeAnalysis 方法论(§5/§6/§7) + 骨架展开(§4) + 分层红线(§3) + 验证判定(§8) + 漂移核查(§9) + 根因分类(§10) + 检查清单(§11) + 静态扫描(§12)。本 SKILL 在 Phase A 和 Phase B 都调用 |
| `story-review-skill.md` / `testcase-review-skill.md` | **上游**：Story+TestCase Review 通过 -> 移交本 SKILL（🆕 v3.10.0 砍 Task，不再经 task-generate）|
| `document-storage-skill.md` | **横切依赖**：4 上下文加载 API + 文档落地 |
| `coding-report-skill.md` | **下游**：出 Coding 报告 |
| `code-review-skill.md` | **下游**：测试真实性/⑥bis/⑦bis 等评审规则 |
| `SKILL.md` | **编排层**：流程图含 CodingProcess 全流程 + 审核点 2.5/4 |
