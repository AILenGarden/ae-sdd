---
name: task-generate
description: 根据 Story 中的 Task 描述和约束文档，生成或更新 Task 实现文档。当 Story Update SKILL 修改了 Task 列表时自动触发，或开发者说"生成 Task"、"写 Task 文档"时触发。生成完成后触发 Coding SKILL。
---

# Task Generate — Task 文档生成 Skill

## 目标

根据 Story 文档中对 Task 的描述，结合约束文档和 Task 模板，生成具体的 Task 实现文档。确保实现者拿到文档后能直接写出一致的代码。

***

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 生成的 Task 文档在写入磁盘前**必须先调用 [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)** 的 API，**不再手写路径**：
> 1. **路径**（§0.6.1 `resolve_path()`）：通过 `intent=TASK` 自动定位到 `ae-sdd-doc/iterations/{YYYY-MM-DD}/Task/{STORY-ID}/`
> 2. **命名 + 版本号**（§0.6.7 `save_doc()`）：**设计类文档带 v{major}.{minor}**（重入时 v 递增，旧版本保留）
> 3. **重入判定**（§0.6.11 `get_latest_version()`）：Task 重入时**新增版本**（v 递增）
> 4. **ChangeLog**（§5）：`save_doc()` 自动追加
> 5. **.gitignore**（§0.6.13 `check_and_update_gitignore()`）：首次写入时自动维护

| 输出文档 | API 调用 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| Task 0（公共依赖）| `save_doc(intent="TASK", storyId, taskId="task-0-公共依赖说明", version={major:1,minor:0})` | v{major}.{minor} | 新增版本（v 递增）|
| 各 Task 文档 | `save_doc(intent="TASK", storyId, taskId, version={major,minor})` | v{major}.{minor} | 新增版本（v 递增）|
| Task 补充说明 | `save_doc(intent="TASK_SUPPLEMENT", storyId)` | 不带版本号 | 原地累加 |
| Task-WriterReport | `save_doc(intent="TASK_WRITER_REPORT", storyId)` | 带 r{N} | 新增（r 递增）|
| Task Review 报告 | `save_doc(intent="TASK_REVIEW", storyId, version={r:N})` | 带 r{N} | **新增**（r 递增）|

> **调用示例：** 详见 `document-storage-skill.md §15.5.2` 调用矩阵。

---

## 🧠 阶段记忆强制调用（🔴 横切依赖）

> **🔴 强制：** Task 生成与 CodingPlan 汇总属于总体 Coding 设计阶段，必须读取 RA / design / coding-plan 记忆，输出后写入 coding-plan 记忆。

```bash
ae-sdd memory enter --phase coding-plan --story <STORY-ID>
# 生成 Task / 统一 CodingPlan
ae-sdd memory write --phase coding-plan --story <STORY-ID> --kind decision --summary "<Task拆分/分层归属/事务边界/风险决策>"
ae-sdd memory exit --phase coding-plan --story <STORY-ID>
```

`memory exit` 未通过 = Task/CodingPlan 节点未完成，禁止进入 CodingSkill.Execute。

---

## 整体流程

```
触发
  │
  ├── 第一步：读取 Story 中的 Task 列表
  │
  ├── 第二步：读取约束文档 + Task 模板
  │       + 调用 project-assets-update-skill.assets.forTaskGenerate(projectKey)
  │       + 读取测试用例文档（已生成的 TestCase）
  │
  ├── 第三步：判断新增/更新
  │       │
  │       ├── 新增 Task → 生成新文档
  │       └── 已有 Task 被修改 → 更新已有文档
  │
  ├── 第四步：生成/更新 Task 文档
  │   │   根据 Story + 测试用例 + 项目资产 + 约束，生成每个 Task
  │   │   │
  │   │   └── 🆕 第四步 ter：每个 Task 撰写过程中调用 [Coding SKILL §④bis](../phase2-coding/coding-skill.md) 5 步 SOP
  │   │       → 生成"任务级 CodePlan 内容"（类骨架 / 方法级逻辑 / 目录对应）
  │   │       → 嵌入到 Task 文档的"实现方案"章节
  │   │       → 任务级 CodePlan 不出独立文件，仅作为 Task 文档的组成部分
  │   │
  │       └── 第四步 bis：单 Task 生成后一致性校验（TC-1~TC-7）
  │
  ├── 第五步：生成 Task 0（公共依赖说明）
  │
  ├── 第五步 bis：全局 Task Review（结合约束+Story+测试用例审查所有 Task）
  │       │
  │       ├── 发现问题 → 启动 Task 修复 → 修复完重新 Review
  │       └── 无问题 → 退出 Review
  │
  ├── 第六步：汇总所有 Task 的"任务级 CodePlan"为一份统一版 CodePlan
  │   │   提取所有 Task 的"实现方案"章节
  │   │   + 套用 [Coding SKILL §④bis 16 节 CodePlan 模板](../phase2-coding/coding-skill.md)
  │   │   + 走 [Coding SKILL §④bis 5 步 SOP](../phase2-coding/coding-skill.md) 重组为统一版 CodePlan
  │   │   │
  │   │   └── 🔴 第六步：输出统一版 `{STORY-ID}-CodingPlan.md` 给用户审核
  │   │       （用户必须明确"确认"/"同意"/"可以开始"才能进入 ⑤ Coding）
  │   │
  │   └── 第六步 bis：基于统一版 CodePlan 输出完整实现方案
  │       （兼容旧版输出格式，供下游 SKILL 调用）
  │
  ├── 第七步：触发 Coding SKILL（⑤ Coding 阶段）
  │   │   Coding SKILL 严格按统一版 CodePlan 实施
  │   │   │
  │   │   └── 🆕 异常时走 Coding SKILL 实时追溯链（参见 coding-skill.md 异常路径）
  │   │       追溯顺序：先读 Task → 再读 Story → 再读 DR → 都无问题时改代码
  │
  └── 完成
```

***

## 触发条件

| 场景                              | 触发方式                  |
| ------------------------------- | --------------------- |
| Story Update SKILL 修改了 Task 列表  | 自动触发                  |
| Story Update SKILL 修改了接口契约/数据模型 | 自动触发（影响 Task 核心代码）    |
| 开发者手动触发                         | "生成 Task"、"写 Task 文档" |

***

## 第一步：读取输入（🆕 2026-06-10 加"是否有 Story"分支）

> **2026-06-10 增强：** 任务规模分级后，本 SKILL 支持两种输入模式。

### 1.A 有 Story 上级文档（重/小任务场景）

从 Story 的「实现任务映射」章节提取：

- Task 执行顺序
- 每个 Task 的名称、说明、涉及工程/层
- Task 间的依赖关系
- Task 文档链接（判断是否已存在）

### 1.B 无 Story 上级文档（🆕 2026-06-10 微任务场景扩展）

> **触发条件：** 用户说"加个 XX" / "出个 Task" 且任务规模判定 = 小任务。但**注意：本 SKILL 不处理"微任务"**——微任务直接路由到 `coding-skill.md §CodingSkill.Plan` 出 CodingPlan，跳过本 SKILL。
>
> 小任务虽然**没有 Story 上级文档**，但需要走 TaskGenerateSkill 生成 Task 文档。输入信息由用户提供：
> - 任务简述（一句话）
> - 涉及工程 / 服务名缩写
> - 涉及文件范围（已知改哪些文件 / 函数 / 模块）
> - 约束条件（性能/安全/兼容等）

**Story 上下文缺失时的处理（2026-06-10 用户要求）：**

| Task 文档章节 | 有 Story 时的填充依据 | 无 Story 时的填充依据 |
|-------------|-------------------|---------------------|
| 任务描述 | Story 章节引用 | 用户任务简述 |
| 核心设计 → 决策基线 | Story `## 实现方案决策基线` | 标注"无 Story 上下文，依据任务简述独立决策" |
| 核心设计 → 关键设计机制 | CodingSkill.Plan 加载 CodingModel 后产出 | 同左（CodingSkill 独立产出） |
| CodingModel 决策记录 | 独立产出 + 引用 Story 上下文 | 独立产出，**不引用 Story 章节** |
| 任务级 CodePlan | 同上 | 同上（独立产出） |
| 验收映射 | Story AC | 用户提供的验收点 / 测试用例 |
| 约束检查 | 约束文档 + Story 偏离声明 | 约束文档 + 用户口述约束 |

**禁止（2026-06-10 硬规则）：**
- ❌ 在无 Story 上下文的 Task 文档里编造 Story 引用
- ❌ 在 CodingModel 决策中伪造"参考 Story §X.Y"

***

## 第一步半：前置依赖检查（门禁）

从 Story 的「前置条件」章节提取所有"系统前置：STORY-XXX 已完成"项，逐项检查：

1. 定位依赖 Story 的代码工程
2. 检查该 Story 声明的接口/SPI 是否已实现并提交
3. 若未实现 → **停止 Task 生成**，输出阻塞原因

**停止输出格式：**

```
⛔ 无法生成 STORY-{当前} Task 文档。

阻塞原因：前置依赖 STORY-{依赖} 未实现。
缺失内容：{具体缺失的接口/能力描述}
建议：先实现 STORY-{依赖}，再回来生成本 Story 的 Task。
```

**判定标准：**

| 前置条件描述 | 检查方式 | 通过标准 |
|------------|---------|---------|
| STORY-XXX 已完成 | 检查对应工程中接口实现代码是否存在 | 接口已定义 + 实现已提交 |
| STORY-XXX DDL 已完成 | 检查表是否存在或 DDL 脚本是否已提供 | DDL 已执行或脚本已就绪 |
| STORY-XXX 接口可调用 | 检查 SPI/接口类是否存在且有实现 | 接口 + 实现均存在 |

**原因：** Task 文档要求写明核心实现代码（方法签名、调用参数、返回值），如果依赖方接口未定义，无法写出正确的调用代码，强行假设会导致依赖方实际设计后不一致而返工。依赖"已实现"的判定标准：①接口已定义（接口类存在）②实现类存在（实现类文件存在）③无编译错误（导入该接口不报错）。部分实现（如只有接口无实现）视为未实现。

***

## 第二步：读取约束文档 + Task 模板

### 2.1 约束文档

> **🆕 2026-06-10 解耦改造：** 不再直接读 `constraints/` 目录。通过 `document-storage-skill.get_constraints(projectKey)` 获取约束文档路径，约束随工程走，SKILL 不依赖约束在哪里。

调用 `document-storage-skill.get_constraints(projectKey)`，从返回的 `ConstraintList` 中提取与当前 Task 相关的约束项，重点关注：

| 约束 name | 对 Task 生成的影响 |
|---------|-----------------|
| `project-structure` | 分层职责红线、包路径规范 |
| `layered-arch` | 分层依赖方向、各层职责 |
| `code-style` | 命名规范、注解使用 |
| `api` | 接口规范（仅 Interfaces 层 Task）|
| `database` | 建表、索引规范（仅 Infrastructure 层 Task）|
| `technology-stack` | 技术栈版本（确认依赖版本）|

### 2.2 Task 模板

使用 `templates/design/be-task-template.md` 作为格式规范。

***

## 第三步：判断新增/更新

| 情况                          | 处理       |
| --------------------------- | -------- |
| Task 文档不存在                  | 新建       |
| Task 文档存在，Story 中 Task 描述未变 | 跳过       |
| Task 文档存在，Story 中 Task 描述已变 | 更新受影响的章节 |
| Task 被删除                    | 标记文档为废弃  |

***

## 第四步：生成/更新 Task 文档

### 4.0 存放路径

Task 文档按 Story 分子目录存放，由 `documentStorage.resolve_path(intent="TASK", storyId, taskId, docType="Task")` 自动定位：

```
ae-sdd-doc/iterations/{YYYY-MM-DD}/Task/
├── STORY-010-BE/
│   ├── task-0-公共依赖说明-v1.0.md
│   ├── task-1-BossUserQuery-v1.0.md
│   └── task-2-BossUserCreate-v1.0.md
├── STORY-011-BE/
│   ├── task-0-公共依赖说明-v1.0.md
│   └── task-1-OrderQuery-v1.0.md
└── ...
```

**完整路径示例：** `d:\Item\icec-cloud-boss\ae-sdd-doc\iterations\2026-06-17\Task\STORY-010-BE\task-1-BossUserQuery-v1.0.md`（具体路径通过 `documentStorage.resolve_path()` 获取）

### 4.1 生成规则

| 规则               | 说明                          |
| ---------------- | --------------------------- |
| 包路径必须固定          | 从 Task 0 或 Story 中获取，不可自行命名。如 Task 0 未指定包路径，参考同层已有代码的包路径模式；如无参考，询问用户确认 |
| 核心代码必须完整         | 实现者可直接使用，不需要猜测              |
| 依赖关系必须明确         | 前置 Task 和被依赖 Task           |
| 约束检查必须列出         | 与当前 Task 相关的约束项             |
| 字段类型必须与 Story 一致 | 特别注意 String vs Long         |

### 4.2 骨架生成要求

> **原则：** Task 只写骨架（大致方向 + 依赖约束），完整实现代码由 Coding SKILL 负责。

| 要求 | 说明 |
|------|------|
| 类名确定 | 不可有歧义 |
| 包路径确定 | 完整的 package 声明 |
| 方法签名确定 | 参数类型、返回类型、方法名 |
| 核心注解完整 | `@Slf4j` / `@Service` / `@RequiredArgsConstructor` / `@Transactional` 等必须写出 |
| **依赖工具包列出** | 列出该组件依赖的核心类/注解（MyBatis-Plus / Lombok / Spring / Feign 等），说明用途 |
| **字段只列核心依赖** | Repository / Client / 外部 Service 字段，不列工具类 |
| 方法伪代码 ≤10 行 | 每步动词开头（校验/查询/转换/调用/返回），不写完整条件判断/循环/try-catch |
| **禁止写完整方法体** | 条件分支、异常处理、循环体均由 Coding SKILL 填充 |

### 4.3 检查项生成要求

每个 Task 必须包含可勾选的检查项，覆盖：

- 约束合规性
- 代码规范
- 功能完整性

***

## 第四步 bis：生成后一致性校验（强制门禁）

> **强制要求：每个 Task 文档生成后，必须立即校验与 Story、模板的一致性。不一致 → 必须修正后才行。**

### 校验清单

| # | 检查项 | 判定标准 | 不一致处理 |
|---|--------|---------|-----------|
| TC-1 | 方法签名与 Story 一致 | Task 文档中的方法名、参数类型、返回类型、参数顺序与 Story 接口契约完全一致（参数名可不同，但类型和顺序必须一致） | 修正 Task 文档，使其与 Story 一致 |
| TC-2 | 字段类型与 Story 一致 | Task 文档中的字段类型（特别是 String vs Long）与 Story 数据模型完全一致 | 修正 Task 文档，使其与 Story 一致 |
| TC-3 | 包路径与 Task 0 一致 | Task 文档中的 package 路径与 Task 0 定义的公共路径一致 | 修正 Task 文档，使用 Task 0 中的路径 |
| TC-4 | 骨架逻辑与 Story AC 一致 | Task 文档中方法级逻辑的逻辑步骤能覆盖 Story 的 AC（验收标准），各主流程分支均有伪代码步骤对应 | 补充缺失逻辑步骤，确保 AC 全覆盖 |
| TC-5 | 必填章节与模板一致 | Task 文档包含模板中标注必填的所有章节（含 `## CodingModel 决策记录` + `## 任务级 CodePlan` 的 5 个子节） | 补充缺失章节 |
| TC-6 | 格式与模板一致 | 标题层级、表格结构、代码块格式与模板一致 | 修正格式，使其与模板一致 |
| TC-7 | 约束合规性 | Task 文档中的实现方式不违反任何约束文档的强制规则 | 修正实现方式，遵守约束 |
| TC-8 | CodingModel 决策记录完整 | `## CodingModel 决策记录` 中 11 维均有明确结论（无空值、无"不知道"）；涉及核心链路时"核心链路保护"表格已填写 | 回到 `CodingSkill.Plan(task-level)` 补充缺失维度 |
| TC-9 | 任务级 CodePlan 子节完整 | `## 任务级 CodePlan` 下的 5 个子节（类骨架/方法级逻辑/DB 操作/外部依赖/测试映射）均已填写，无整节空表格 | 回到 `CodingSkill.Plan(task-level)` 补充缺失子节 |

**判定结果：**
- 全部通过 → 本 Task 校验完成，进入下一个 Task
- 任意一项不一致 → 修正 Task 文档 → 重新校验 → 直到全部通过

---

## 第五步：生成/更新 Task 0

如果 Story 的接口契约、数据模型、涉及工程发生变化，需要同步更新 Task 0（公共依赖说明）：

- 公共类包路径
- DO 定义
- Repository 接口定义
- Gateway 接口定义
- 技术栈版本

***

## 第五步 bis：全局 Task Review（强制闭环）

> **与第四步 bis 的区别：** 第四步 bis 是"单个 Task 生成时的逐项一致性校验"；本步是"全部 Task 生成后的全局 Review"，站在整体视角，结合约束规范 + Story + 测试用例，审查所有 Task 是否能完整、正确地实现 Story。

**触发时机：** Task 0 ~ Task-N 全部生成完成，且各自第四步 bis 校验通过后。

**输入：**
- 全部 Task 文档（Task-0 ~ Task-N）
- Story 主文档 + 补充说明
- 测试用例文档
- `document-storage-skill.get_constraints(projectKey)` 返回的全部约束文档
- `templates/design/be-task-template.md`

**Review 循环（强制闭环）：**

```
全局 Review 所有 Task
    │
    ├── 发现问题 → 启动 Task 修复流程 → 修复完成 → 重新全局 Review
    │
    └── 无新增问题 → 退出 Review → 进入第六步 bis
```

> **闭环规则：** 每次修复后必须重新执行完整的全局 Review（不是只看修复项），直到一轮 Review 无任何新增问题才能退出。退出计数器：本轮发现新问题 → 计数器归零；连续一轮无新增问题 → 满足退出条件。

### Review 检查清单

**TR-1 Story 覆盖完整性**
| 检查项 | 判定标准 |
|--------|---------|
| TR-1.1 | Story 的每个 AC 都能被某个 Task 实现（AC → Task 映射无遗漏） |
| TR-1.2 | Story 主流程的每一步都有对应 Task 实现 |
| TR-1.3 | Story 接口契约中的每个接口都有对应 Task 实现 |
| TR-1.4 | Story 数据模型中的每张表/字段变更都有对应 Task |

**TR-2 测试用例覆盖性**
| 检查项 | 判定标准 |
|--------|---------|
| TR-2.1 | 每个测试用例所验证的功能点，都有对应 Task 实现 |
| TR-2.2 | Task 的核心代码能让对应的测试用例 Pass（逻辑上能通过） |
| TR-2.3 | 测试用例涉及的 Mock 点（外部依赖）在 Task 中有对应的接口定义 |

**TR-3 Task 间一致性**
| 检查项 | 判定标准 |
|--------|---------|
| TR-3.1 | Task 间调用的方法签名一致（A 调用 B，B 的方法签名与 A 期望一致） |
| TR-3.2 | Task 间传递的数据类型一致（无 String vs Long 错配） |
| TR-3.3 | Task 依赖顺序无循环依赖 |
| TR-3.4 | 公共类（Task 0）被各 Task 正确引用，包路径一致 |

**TR-4 约束合规性**
| 检查项 | 判定标准 |
|--------|---------|
| TR-4.1 | 所有 Task 的实现方式不违反 `get_constraints(projectKey)` 返回的强制规则 |
| TR-4.2 | 分层架构合规（无跨层调用、无反向依赖） |
| TR-4.3 | 事务边界合规（无大事务，事务内无外部调用） |
| TR-4.4 | 类型规范合规（ID 用 Long、金额用 BigDecimal 等） |
| TR-4.5 | 🔴 分层职责归位：领域逻辑归 Domain、业务编排归 Application、Repository 只做存取。逐 Task 核对——Repository 类的方法是否都是存取语义（无状态流转/业务校验/编排）？业务规则是否写在 Domain 而非 Application？（依据 `get_constraints(projectKey)["project-structure"]` 分层职责红线） |

**TR-5 实现可行性**
| 检查项 | 判定标准 |
|--------|---------|
| TR-5.1 | 每个 Task 的核心代码可编译（依赖的类/接口都存在或在前置 Task 中定义） |
| TR-5.2 | 每个 Task 依赖的外部接口已在前置 Task 或依赖 Story 中定义 |
| TR-5.3 | 无"凭空出现"的类/方法调用（调用的东西都有出处） |

**TR-6 复用与能力归属（🔴 防重复实现）**
| 检查项 | 判定标准 |
|--------|---------|
| TR-6.1 | 🔴 每个 Task 都引用 Story 的实现方案决策基线：说明本 Task 对应哪个实现点、复用哪个现有能力或为什么新建；缺失 → 🔴 阻断 |
| TR-6.2 | 🔴 同一业务能力只能有一个 owner Task / owner 类 / owner 方法。若多个 Task 都在实现同一业务动作（如接单、结单、关闭、通知、状态流转、幂等创建），必须收敛为一个 owner，其余 Task 只能调用；否则 → 🔴 阻断 |
| TR-6.3 | 🔴 不复用决策继承 Story 证据。Task 不得绕过 Story 的复用扫描自行新建类/方法；若确需变化，先走 Story Update 更新决策基线 |
| TR-6.4 | 🔴 业内成熟方案在 Task 中落地。状态机、幂等、补偿、消息消费、外部集成等高风险能力的 Task 实现方案必须体现 Story 中选定的成熟方案/团队成熟实现 |
| TR-6.5 | 🔴 五维代码质量不退化。Task 拆分不得导致可用性、高效性、可维护性、健壮性、可读性任一维度退化（五维定义见 `ae-sdd-skill.md §实现方案决策基线`）；尤其不得因为分 Task 产生重复逻辑、跨层业务判断或不可维护的散点实现 |

**TR-7 CodingModel 合规性（🔴 防风险盲区）**
| 检查项 | 判定标准 |
|--------|---------|
| TR-7.1 | 🔴 每个 Task 都包含 `## CodingModel 决策记录`，11 维均有明确结论（无空值、无"不知道"） |
| TR-7.2 | 🔴 每个 Task 的 `## 任务级 CodePlan` 来自 `CodingSkill.Plan(task-level)`，不是 TaskSkill 自行编写 |
| TR-7.3 | 涉及回调/Webhook/支付/状态更新/消息落库/通知的 Task，已填写"核心链路保护"表格且无空值 |
| TR-7.4 | 涉及异步/批量任务的 Task，已标注资源隔离方案、队列上限、拒绝策略、补偿/DLQ 和告警 |
| TR-7.5 | 涉及外部依赖的 Task，已标注超时、重试、降级、幂等约束（对应维度⑥结论非空） |
| TR-7.6 | 涉及高并发/大批量的 Task，已标注容量估算，并在测试映射中有混合压测场景 |

### Review 结论产出

```
全局 Task Review 第 {N} 轮结果：
- TR-1 Story 覆盖完整性：✅ 通过 / 🔴 X 个问题
- TR-2 测试用例覆盖性：✅ 通过 / 🔴 X 个问题
- TR-3 Task 间一致性：✅ 通过 / 🟠 X 个问题
- TR-4 约束合规性：✅ 通过 / 🔴 X 个问题
- TR-5 实现可行性：✅ 通过 / 🔴 X 个问题
- TR-6 复用与能力归属：✅ 通过 / 🔴 X 个问题
- TR-7 CodingModel 合规性：✅ 通过 / 🔴 X 个问题

发现问题：
- [TR-1.1] xxx：xxx

本轮结论：发现 X 个问题 → 启动修复 / 无新增问题 → 退出 Review
```

### Task 修复流程

发现问题后，按问题层级判定修复范围：

| 问题层级 | 判定 | 修复动作 |
|---------|------|---------|
| Task 层 | Task 文档本身有错（与 Story 一致，但 Task 写错了） | 直接修改 Task 文档 → 重新全局 Review<br>**边界：Task 内部实现问题时可直接修；若问题影响 Story AC 或 DR 设计，必须先走 proposal-skill 流程** |
| Story 层 | Task 错误源于 Story 描述有误/遗漏 | 先走 Story Update → Task Generate → 重新全局 Review |
| DR 层 | Story 错误源于 DR 缺陷 | 先走 DR Update → Story Update → Task Generate → 重新全局 Review |

> **修复后必须重新全局 Review，不允许只检查修复项就退出。**

***

## 第六步 bis：输出完整实现方案

**触发时机：** 所有 Task 文档生成完成并通过一致性校验后，全局 Task Review 退出后，触发 Coding 之前。

**前置条件：** Task 0 ~ Task-N 全部已生成，TC-1 ~ TC-7 全部校验通过，全局 Task Review（TR-1~TR-6）已退出（无新增问题）。

**输出模板：** `templates/design/be-task-implementation-plan-template.md`

**产出物：** `{STORY-ID}-Task实现方案.md`

**填写规则：** AI 必须基于所有已生成的 Task 文档，填写模板中的所有章节。禁止留空或填"待补充"。

**核心章节（必须完整填写）：**
- 第一章：Story 信息（汇总所有 Task 的基本信息）
- 第二章：Task 清单与执行顺序（按依赖顺序排列）
- 第三章：Task 执行顺序图（可视化依赖关系）
- 第四章：各 Task 核心实现要点（每个 Task 的关键逻辑、代码片段、DB 操作、约束检查）
- 第五章：分层实现分工（按 Domain/Application/Infrastructure/Interfaces 分层；🔴 每项标注职责归属，领域逻辑→Domain、编排→Application、存取→Repository，不串味）
- 第六章：接口实现映射（Story 接口 → Task → 实现类 → 方法）
- 第七章：DB 变更清单（所有 CREATE/ALTER TABLE 操作）
- 第八章：外部依赖
- 第九章：事务边界
- 第十章：约束合规检查清单
- 第十一章：与 Story 接口契约的一致性确认
- 第十二章：Task 间依赖与调用关系
- 第十三章：实现注意事项

**禁止：**
- 不基于 Task 文档填写，凭空推断 ❌
- 跳过任一章节 ❌
- 填写内容与 Task 文档矛盾 ❌

***

## 第四步 ter（🆕 调用 `CodingSkill.Plan(task-level)`）

> **TaskSkill 不自行生成编码方案，不复制 CodingModel 决策逻辑。**  
> 每个 Task 的 `## CodingModel 决策记录` 和 `## 任务级 CodePlan` 章节  
> **必须来自 `CodingSkill.Plan(task-level)`，禁止 TaskSkill 自行填写。**

### 调用参数

| 参数 | 来源 |
|------|------|
| Story 文档路径 | 当前 Story |
| TestCase 文档路径 | 已生成 TestCase |
| 当前 Task 基础信息 | Story 实现任务映射（任务名 + 层 + 依赖） |
| 项目资产 | `project-assets-update-skill.assets.forTaskGenerate(projectKey)` — 返回 §3 + §4 + §5 + §8（分层/包路径/命名/CodePlan 输入索引）|
| 约束文档 | `document-storage-skill.get_constraints(projectKey)`（不再直接写目录路径）|
| CodingModel 路径 | `standards/thinking/be-coding-thinking-engine.md` |

### `CodingSkill.Plan(task-level)` 必须返回

| 输出块 | 写入 Task 文档章节 |
|--------|-----------------|
| 11 维 CodingModel 决策记录（含核心链路保护） | `## CodingModel 决策记录` |
| 类骨架（注解 + 字段 + 方法签名 + 伪代码） | `## 任务级 CodePlan → 类骨架` |
| 依赖工具包表格 | `## 任务级 CodePlan → 依赖工具包` |
| 方法级逻辑表格 | `## 任务级 CodePlan → 方法级逻辑` |
| DB 操作表格 | `## 任务级 CodePlan → DB 操作` |
| 外部依赖表格 | `## 任务级 CodePlan → 外部依赖` |
| 测试映射表格 | `## 任务级 CodePlan → 测试映射` |

### TaskSkill 职责边界（🔴 强制）

> **🔴 核心原则（2026-06-10 用户确认）：**  
> **Task 与代码有关的部分不会与 CodingSkill 产生冲突，因为这部分由 CodingSkill 去做决策生成。**  
> **Task 其他内容的设计（如元信息/依赖关系/包路径/任务描述/Story 决策基线引用）是 CodingSkill 做决策的依据，不是代码设计。**
>
> 也就是说：
> - **TaskSkill 设计的"上下文"**（任务范围、依赖、Story 引用、约束）→ 是 CodingSkill 的**输入**
> - **CodingSkill 设计的"代码决策"**（CodingModel 决策、骨架、关键设计机制、类骨架、DB 操作、外部依赖策略）→ 是 CodingSkill 的**输出**
> - 两者不会冲突，因为边界清晰：TaskSkill 不碰代码决策，CodingSkill 不碰任务范围

| Task 文档章节 | 填写方 | 性质 |
|-------------|--------|------|
| `## 元信息` | TaskSkill | 任务范围（输入给 CodingSkill） |
| `## 依赖关系` | TaskSkill | 任务范围（输入给 CodingSkill） |
| `## 包路径约定` | TaskSkill | 任务范围（输入给 CodingSkill） |
| `## 任务描述` | TaskSkill | 任务范围（输入给 CodingSkill） |
| `## 核心设计 → 对应决策基线` | TaskSkill | 引用 Story 设计（输入给 CodingSkill） |
| `## 核心设计 → 关键设计机制` | **CodingSkill.Plan** | **代码设计（状态机/锁/幂等）** |
| `## CodingModel 决策记录` | **CodingSkill.Plan** | **代码设计**（11 维 + 核心链路保护） |
| `## 任务级 CodePlan` 5 个子节 | **CodingSkill.Plan** | **代码设计**（骨架/方法级逻辑/DB 操作/外部依赖/测试映射） |
| `## 约束检查` | TaskSkill 预填通用约束 + CodingSkill.Plan 补充风险相关 | 部分输入，部分代码设计 |
| `## 验收映射` | TaskSkill | 引用 Story AC（输入给 CodingSkill） |

**TaskSkill 只负责：**
- 读取 Story / TestCase / 项目资产 / 约束
- 编排 Task（顺序、依赖、层级划分）
- 填写**任务范围类章节**（元信息/依赖/包路径/任务描述/Story 引用/验收映射）
- 调用 `CodingSkill.Plan(task-level)` 获取**代码设计类章节**的产出
- 把产出嵌入 Task 文档对应章节
- 做 Task Review（TC-1~TC-9 + TR-1~TR-7）

**TaskSkill 禁止：**
- 自行发明 CodingModel 之外的编码方案
- 绕过 `CodingSkill.Plan` 直接填写 `## CodingModel 决策记录` 或 `## 任务级 CodePlan`
- 自行填写 `## 核心设计 → 关键设计机制`（这是代码设计判断，必须经过 CodingModel）
- 在 Task 文档中留下未经过 CodingModel 的实现设计

---

## 第六步：汇总所有 Task 的"任务级 CodePlan"为统一版 CodePlan

> **核心流程变更（2026-06-05）：**
> - 旧流程：Task 文档生成后直接触发 Coding SKILL
> - 新流程：Task 文档生成后**先汇总**为统一版 `{STORY-ID}-CodingPlan.md`，**用户审核通过后**再触发 Coding SKILL

**汇总步骤：**

```
所有 Task 文档已生成（每 Task 已含"任务级 CodePlan"）
    ↓
1. 提取所有 Task 的"实现方案"章节
    ↓
2. 套用 [be-coding-plan-template.md](../../templates/coding/be-coding-plan-template.md) 章节结构
    │   - §0 元信息 + §0.5 Tier 选择（按 Story 复杂度）
    │   - §1 项目资产引用块
    │   - §2 抽象分层 → 项目分层映射表（覆盖所有 Task）
    │   - §3 Task 执行顺序编排
    │   - §4 文件级实现顺序
    │   - §5 关键类骨架（汇总所有 Task 的类骨架）
    │   - §6-§15 其他章节
    ↓
3. 走 [Coding-SKILL §④bis 5 步 SOP](../phase2-coding/coding-skill.md) 重组为统一版
    - 步骤 1：调用 `project-assets-update-skill.assets.forTaskGenerate(projectKey)`
      返回：§3 分层映射 + §4 DDD 落点 + §5 命名约定 + §8 Code Plan 输入索引
      → 精准查询：`assets.module(projectKey, name)` / `assets.component(projectKey, name)`
    - 步骤 2-5：基于所有 Task 的任务级 CodePlan 整合
    ↓
4. 跑 [be-coding-plan-template.md §15 14 条门禁自检](../../templates/coding/be-coding-plan-template.md)
    - 缺项目资产 = 整 Plan 打回
    - 文件顺序不可独立编译 = 重写
    - 类骨架不全 = 补
    - DO 字段不一致 / SQL WHERE 不明确 / 测试数据不可追溯 / 核心场景未标真实 DB 或 HTTP / 验证点未覆盖 / 调试回滚 < 5 类 = 修补对应章节
    ↓
5. 🔴 输出统一版 `{STORY-ID}-CodingPlan.md` 给用户审核
    - 用户必须明确说"确认"/"同意"/"可以开始"才能进入 ⑦ Coding
    - 模糊回复（如"好"/"行"/"看看"）需 AI 追问确认
    - 跳过/整体确认视为违规
```

**关键规则：**
- 🔴 统一版 CodePlan **必须**先经用户审核，**禁止**直接进入 ⑤ Coding
- 🔴 14 条门禁任何一条不通过 → 修补对应章节，**禁止**带病进入用户审核
- 🔴 临时偏离 CodePlan → 需用户确认，AI 不得自行调整
- 🔴 ⑥ ⑥bis ⑥ter 三个步骤顺序不可颠倒（先汇总→再实现方案→再触发 Coding）

---

## 第七步：触发 `CodingSkill.Execute`（⑤ Coding 阶段）

> **变更：** 原"第六步：触发 Coding SKILL"已升级为"第七步"。原因：旧流程下 Coding SKILL 直接吃 Task 文档就开始写代码；新流程下 Coding SKILL 吃的是**统一版 CodePlan**。

**触发条件：**
- 🔴 ⑥ 步输出的 `{STORY-ID}-CodingPlan.md` 已获用户明确确认

**触发参数：**
- Story 文档路径
- 统一版 CodePlan 路径（`{STORY-ID}-CodingPlan.md`）
- 项目资产（`project-assets-update-skill.assets.forTaskGenerate(projectKey)`）
- 工作目录

**异常处理：**
- ⑤ Coding 报错时 → 走 [Coding SKILL 实时追溯链](../phase2-coding/coding-skill.md)
- 追溯顺序：先读 Task（修 Task 文档 + CodePlan）→ 再读 Story → 再读 DR → 都无问题时判定 AI 犯蠢直接改代码

***

## 禁止事项

| 禁止             | 应该                |
| -------------- | ----------------- |
| 骨架留空或方法伪代码步骤不完整 | 主流程每个分支都必须有伪代码步骤；伪代码步骤数 ≥ 主流程分支数 |
| 包路径使用占位符       | 使用固定的完整包路径        |
| 方法签名模糊         | 参数类型、返回类型必须明确     |
| 忽略 Task 间依赖    | 必须标注前置依赖和被依赖      |
| 跳过约束检查         | 每个 Task 必须列出相关约束项 |
| **TaskSkill 自行填写 CodingModel 决策记录或任务级 CodePlan** | **这两个章节必须来自 `CodingSkill.Plan(task-level)`，TaskSkill 只负责调用和嵌入** |
| **跳过全局 Task Review** | **所有 Task 生成后必须执行全局 Review（TR-1~TR-7），结合约束+Story+测试用例审查** |
| **Review 发现问题只改修复项就退出** | **每次修复后必须重新全局 Review，连续一轮无新增问题才能退出** |
| **Task Review 发现 Story/DR 问题时直接改 Task** | **Story 层问题先走 Story Update，DR 层问题先走 DR Update，不在 Task 层弥补上层缺陷** |
| **Task 绕过 Story 复用决策自行新建实现** | **先回到 Story Update 更新实现方案决策基线，再重新生成 Task** |
| **同一业务能力散落多个 Task 重复实现** | **必须指定唯一 owner Task，其余 Task 只调用/编排，不复制业务逻辑** |

***

## 执行清单（逐项执行，不可跳过）

> AI 启动本 SKILL 时，必须用 TodoWrite 1:1 映射此表，每完成一行验证"产出物已生成 + 门禁已满足"后才进入下一步。

| # | 动作                 | 产出物                    | 门禁              |
| - | ------------------ | ---------------------- | --------------- |
| 1 | 读取 Story 实现任务映射    | —                      | Task 列表已提取      |
| 1.5 | 前置依赖检查           | —                      | 所有前置 Story 接口已实现，否则停止 |
| 2 | 读取约束 + Task 模板     | —                      | 全部约束已读取         |
| 3 | 判定新增/更新/删除         | —                      | 每个 Task 处理方式已确定 |
| 4 | 生成/更新 Task 0（公共依赖） | `task-{N}-0-公共依赖说明.md` | 文件已生成/更新        |
| 4.ter | **每个 Task 撰写时调用 `CodingSkill.Plan(task-level)`** | Task 文档 `## CodingModel 决策记录` + `## 任务级 CodePlan` 章节 | 7 个产出块均已嵌入 Task 文档（决策记录+类骨架+依赖工具包+方法级逻辑+DB操作+外部依赖+测试映射）；TaskSkill 不得自行填写这两个章节 |
| 4.5 | **生成后一致性校验** | — | TC-1 ~ TC-9 全部通过；任一检查项失败则修正后重新校验；若同一检查项失败 2 次仍不通过，标记为"需人工审核"，暂停自动校验，等待人工判定 |
| 5 | 按顺序生成/更新各 Task     | `task-{N}-{X}-*.md`    | 全部 Task 文件已生成 + 4.5 一致性校验通过 |
| 5bis | **全局 Task Review（结合约束+Story+测试用例）** | Review 结论 | TR-1~TR-7 全部通过；发现问题 → 启动 Task 修复 → 修复完重新全局 Review；连续一轮无新增问题才能退出 |
| 6 | **汇总所有 Task 的"任务级 CodePlan"为统一版** | `{STORY-ID}-CodingPlan.md` | 套用 be-coding-plan-template.md + **14 条门禁全过**（含 CodingModel 决策完整+核心链路保护+资源隔离+混合压测）；用户必须明确"确认"才进入下一步 |
| 6.5 | 自检（约束合规 + 骨架完整性）   | —                      | 检查项全部通过         |
| 6bis | 输出完整实现方案 | `{STORY-ID}-Task实现方案.md` | 基于所有 Task 文档填写，13 个核心章节均已填写，不留空白 |
| 7 | 触发 `CodingSkill.Execute` | ⑤ Coding 开始 | Phase ④→⑤ 调用协议 7 项前置条件全满足（见 auto-engineering-skill §④bis）；Coding SKILL 严格按 CodePlan 实施；异常时走实时追溯链 |



---

## 📖 人工审核主动讲解规范 — Task 节点

> **来源：** 原 ae-sdd-skill.md L18-225 三个审核节点讲解模板之一，本节定义 Task Review 阶段（🔍 人工审核点 2）的主动讲解规范。

**AI 必须主动讲解的内容：**

| 维度 | 必须讲清楚 |
|------|-----------|
| 任务拆分故事 | 为什么拆成 N 个 Task？拆分依据是什么（DDD 聚合根/分层/依赖关系）？ |
| 依赖链路 | Task 之间的依赖关系是什么？为什么要先做 Task-1 再做 Task-2？ |
| 每 Task 故事 | 这个 Task 做了什么？用什么技术方案？关键代码逻辑是什么？ |
| DB 变更故事 | 表结构怎么设计？索引怎么加？有没有跨服务的数据一致性问题？ |
| 事务边界故事 | 哪些操作在同一个事务？哪些操作在事务外？事务传播行为是什么？ |
| 风险 Task 标记 | 哪些 Task 风险较高？为什么？需要重点 Review 哪些点？ |

**与逐文件核对的配合：**
> 本审核点已规定必须**逐文件自上而下核对**。主动讲解规范在逐文件核对的基础上，**在每个文件核对前先讲一遍本文件的故事**（这个 Task 是什么、为什么这么拆、关键点在哪），再开始走核对流程。讲完后才进入"AI 完整读出本文件内容 → 用户审阅"的环节。

**输出模板（每个 Task 核对前）：**

```
📖 【Task 讲解 - {TASK-ID}】

【本 Task 的故事】
- 解决什么：{用户场景/Story AC}
- 实现位置：{Domain/Application/Infrastructure/Interfaces} 层的 {类} → {方法}
- 核心逻辑：{一段话讲清楚做什么}
- 关键技术：{用到的技术/设计模式/算法}

【关键点说明】
- 关键决策 1：{是什么} → 决策依据：{为什么这么选}
- 关键决策 2：{是什么} → 决策依据：{为什么这么选}

【风险点】
- 风险 1：{场景} → 应对：{方案}
- 风险 2：{场景} → 应对：{方案}

【实现方案讲解结束 - 进入逐文件核对】
下面开始读出本文件全文，请审阅。
```

**Task 节点专用反模式：**
- ❌ 一次性抛所有 Task 文档让用户"整体确认"（用户没法看）
- ❌ 拆分故事跳过"为什么不拆得更细/更粗"
- ❌ 事务边界只说"在 AppService.transition()"而不讲事务内/外分别包含哪些操作
- ❌ 风险 Task 不标记，导致 Review 时被忽略

**门禁：**
- 🔴 未逐 Task 输出 `📖 【Task 讲解 - ...】` → 视为跳过人工审核点 2 → 禁止进入 ⑤ Coding
- 🔴 拆分故事缺失 DDD 聚合根/分层依据 → 视为拆分不严谨，必须返工
- 🔴 风险 Task 未标记 → 视为审核漏项
