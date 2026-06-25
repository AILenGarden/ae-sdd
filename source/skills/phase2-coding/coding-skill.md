---
name: coding
description: 根据 Story + Task 文档 + 测试用例 + 项目约束，按 Task 执行顺序生成完整的生产代码并通过编译验证。每个 Task 开始前必须向用户呈现实现方案并获确认，未确认禁止写代码。编码过程中发现问题时，自动进入问题反馈环节，分析并修复文档缺陷。当开发者说"生成代码"、"写代码"、"实现 Story"、"根据 Story 开发"、"根据 Task 实现"、"确认实现方案"、"Coding 遇到问题"时触发。
---

# Coding — Task 驱动代码生成 Skill

## 📦 文档存放前置调用（🔴 横切依赖）

> **🔴 强制：** 本 SKILL 涉及的所有输入/输出文档（Story / Task / TestCase / CodingPlan / CodingReport / 开发问题记录）在读写前**必须先调用 [`document-storage-skill.md`](../cross-cutting/document-storage-skill.md)** 的 API，**不再手写路径**：
> 1. **路径**（§0.6.1 `resolve_path()`）：通过各 `intent` 自动定位
> 2. **命名 + 版本号**（§0.6.7 `save_doc()`）：CodingReport 带 `v{N}-r{M}`（事件类）
> 3. **重入判定**（§0.6.11 `get_latest_version()`）：CodingReport 重入时 r 递增
> 4. **ChangeLog**（§5）：`save_doc()` 自动追加
> 5. **.gitignore**（§0.6.13 `check_and_update_gitignore()`）：首次写入时自动维护

**本 SKILL 涉及文档类型与 API 调用对应：**

| 文档类型 | API 调用 | 命名规则 | 重入时动作 |
|---------|---------|---------|----------|
| Story 文档 | `resolve_path(intent="STORY", storyId)` | v{major}.{minor} | 新增版本（v 递增）|
| Task 文档目录 | `resolve_path(intent="TASK", storyId, taskId)` | v{major}.{minor} | 新增版本（v 递增）|
| 测试用例 | `resolve_path(intent="TESTCASE", storyId)` | v{major}.{minor} | 新增版本（v 递增）|
| 统一版 CodingPlan | `save_doc(intent="CODING_PLAN", storyId, version={major,minor})` | v{major}.{minor} | 新增版本（v 递增）|
| CodingReport | `save_doc(intent="CODING_REPORT", storyId, version={v:N,r:M})` | 带 v{N}-r{M} | 新增（r 递增）|
| Test 报告 | `save_doc(intent="TEST_REPORT", storyId, version={v:N,r:M})` | 带 v{N}-r{M} | 新增（r 递增）|
| 开发问题记录 | `save_doc(intent="CODING_ISSUE_LOG", storyId)` | 不带版本号 | 原地累加 |

> **调用示例：** 详见 `document-storage-skill.md §15.5.2` 调用矩阵。

---

## 🧠 阶段记忆强制调用（🔴 横切依赖）

> **🔴 强制：** CodingPlan 与 Coding 执行都必须使用 ae-sdd 阶段记忆。禁止只凭 Agent 对话上下文继续实现。

### CodingSkill.Plan

```bash
ae-sdd memory enter --phase coding-plan --story <STORY-ID>
# 生成或修订 CodingPlan
ae-sdd memory write --phase coding-plan --story <STORY-ID> --kind decision --summary "<架构决策/风险预判/复用决策/资源边界>"
ae-sdd memory exit --phase coding-plan --story <STORY-ID>
```

### CodingSkill.Execute

```bash
ae-sdd memory enter --phase coding --story <STORY-ID>
# 编码、编译、测试、真实 DB/HTTP 验证
ae-sdd memory write --phase coding --story <STORY-ID> --kind finding --summary "<实现结果/失败修复/测试证据/残余风险>"
ae-sdd memory exit --phase coding --story <STORY-ID>
```

`memory exit` 未通过 = 当前 Coding 节点未完成，禁止进入 CodingReport / CodeReview。

---

## 目标

根据 Story 文档中的 Task 执行顺序，逐个读取 Task 文档，结合测试用例定义的验收场景，按 Task 内的核心代码和设计生成完整的生产代码。编码过程中发现问题时，记录问题、分析根因、修复文档，形成闭环。

---

## 整体流程

```
正常路径（Plan）：
  第零步 加载 CodingModel → 产出 11 维决策记录
    ↓
  读 Story / TestCase / Task → 生成任务级骨架 + CodePlan
    ↓
  输出统一版 CodingPlan（14 条门禁）→ 等待用户确认

正常路径（Execute，用户确认 CodingPlan 后）：
  第零步 加载 CodingModel → 复核 11 维决策记录
    ↓
  按 Task 顺序实现 → 编译 → 测试 → 真实 DB/HTTP → 全切面一致性核查 → Coding 报告

异常路径（任意步骤可触发）：
  发现问题 → 记录问题文档 → 分析根因 → 修复文档 → 回到正常路径继续
```

---

## CodingSkill 对外调用契约

> CodingSkill 暴露两个子能力供外部调用。**调用方不得绕过本契约直接引用内部章节号。**

### `CodingSkill.Plan`

**调用方：**
- `task-generate-skill.md`（每个 Task 撰写时，生成任务级 CodePlan）
- `task-generate-skill.md` 第六步（汇总统一版 CodingPlan）
- `ae-sdd-skill.md` Task 汇总阶段
- **🆕 2026-06-10**：`ae-sdd-skill.md` 微任务场景（直接调用，不经 task-generate-skill）

**用途：** 生成任务级 CodePlan 或统一版 CodingPlan。只产出设计，不写生产代码。

**输入参数：**

| 参数 | 来源 | 任务规模适用性 |
|------|------|---------------|
| Story 文档路径 | 当前 Story | 重/小任务必填；**🆕 微任务不传**（无 Story 上下文）|
| TestCase 文档路径 | 已生成 TestCase | 重任务必填；小任务按需；**🆕 微任务按需** |
| 当前 Task 基础信息 / Task 列表 | Story 实现任务映射（重/小任务）<br>**🆕 微任务**：任务简述 + 涉及工程 + 涉及文件范围 | 必填 |
| 项目资产 | `ae-sdd assets read coding --project <projectKey>` — 返回 §4 + §5 + §6 | ✅ 必填 |
| 约束文档 | `document-storage-skill.get_constraints(projectKey)` | ✅ 必填 |
| CodingModel | `document-storage-skill.get_thinking_engine()` | ✅ 必填 |

**🆕 2026-06-10 微任务场景扩展（无 Story 上下文）：**

- **输入**不含 Story 文档路径、不含 TestCase 文档路径
- **任务级 CodePlan 中所有章节独立产出**（基于 CodingModel + 约束 + 用户任务简述）
- **不引用**任何 Story 章节
- **禁止伪造** "参考 Story §X.Y" 之类的引用
- **验证点**直接从用户口述的验收点/约束推导（不从 Story AC 推导）

**强制执行（每次调用均须）：**
1. 第零步：加载 CodingModel，产出本轮 **11 维 CodingModel 决策记录**
2. 完成 5 项需求理解（量级 / 边界 / 依赖 / 约束 / AC 映射）——**🆕 微任务无 AC 时用"用户验收点"替代**
3. 按 §④bis 5 步 SOP 生成骨架
4. 对任意 🔴 维度结论为"需要"的，必须在骨架/伪代码中体现处理方案
5. 输出通过 §④bis 14 条门禁的 CodePlan 块
6. **🆕 任务级 CodePlan 的"参考"章节**：有 Story → 填"参考 Story §X.Y"；**无 Story → 填"无 Story 上下文，独立决策"**

**输出：**
- 任务级 CodePlan 块（含 CodingModel 决策记录），嵌入 Task 文档的 `## CodingModel 决策记录` + `## 任务级 CodePlan` 章节
- 或统一版 `{STORY-ID}-CodingPlan.md`

**禁止：**
- 禁止写生产代码
- 禁止跳过 CodingModel 11 维决策（任一维度结论为空 → 停止，要求补充上游信息）
- 禁止只输出类名/方法名而不输出决策依据

---

### `CodingSkill.Execute`

**调用方：**
- `ae-sdd-skill.md`（TaskSkill 全部完成 + 用户确认统一版 CodingPlan 后）—— **重/小任务场景**
- 用户直接触发"开始 Coding / 写代码 / 实现 Story"
- **🆕 2026-06-10**：`ae-sdd-skill.md` 微任务场景（直接调用，不经 task-generate-skill）

**用途：** 按确认后的统一版 CodingPlan 生成代码、测试代码，并完成完整验证。

**输入参数：**

| 参数 | 来源 | 任务规模适用性 |
|------|------|---------------|
| Story 文档路径 | 当前 Story | 重/小任务必填；**🆕 微任务不传**（无 Story 上下文）|
| TestCase 文档路径 | 已生成 TestCase | 重任务必填；小任务按需；**🆕 微任务按需** |
| Task 文档目录 | `documentStorage.resolve_path(intent="TASK", storyId, taskId)`（重/小任务）<br>**🆕 微任务不传** | 条件必填 |
| 统一版 CodingPlan 路径 | `documentStorage.resolve_path(intent="CODING_PLAN", storyId)`（重/小任务）<br>**🆕 微任务**：`documentStorage.resolve_path(intent="CODING_PLAN", taskName=事务简称, scope="service")` | 必填 |
| 项目资产 | `ae-sdd assets read coding --project <projectKey>` — 返回 §4 + §5 + §6 | ✅ 必填 |
| 工程目录 | `document-storage-skill.get_git_path(projectKey)`（不再由用户直接传入磁盘路径）| ✅ 必填 |
| CodingModel | `document-storage-skill.get_thinking_engine()` | ✅ 必填 |
| **🆕 任务规模标识** | `task_scale: heavy / small / micro` | ✅ 必填（决定 §4.2 / §6 走哪条分支）|

**强制执行（每次调用均须）：**
1. 第零步：加载 CodingModel，复核 CodingPlan 中的 11 维决策记录
2. 校验统一版 CodingPlan 已通过 14 条门禁（缺项 → 停止，回到 Plan 阶段补充）
3. 每个 Task 开始前复核该 Task 的 CodingModel 决策记录
4. 按 CodePlan 写代码，不得重新发明方案
5. 编译 → 测试 → 真实 DB/HTTP 验证 → 全切面一致性核查
6. 触发 `coding-report-skill` 出 Coding 报告

**输出：**
- 生产代码
- 测试代码
- Coding 报告（`coding-report-skill` 产出）

**禁止：**
- 禁止绕过统一版 CodingPlan 自行设计方案
- 禁止跳过 CodingModel 复核直接写代码
- 发现 CodingPlan 缺陷时，必须走实时追溯链（Task → Story → DR → AI 实现问题），不得直接改代码

---

## 第零步：加载 CodingModel（🔴 强制，Plan 和 Execute 均须执行）

> `CodingSkill.Plan` 和 `CodingSkill.Execute` 的入口门禁。未完成本步，禁止进入后续步骤。

**加载路径：** `standards/thinking/be-coding-thinking-engine.md`

**必须产出本轮 CodingModel 决策记录：**

| 维度 | 本轮结论 | 处理方案 | 证据（文件:行号 / Story AC / TestCase ID） |
|------|----------|----------|----------------------------------------|
| ① 原子性 | 需要 / 不需要 | 事务边界 / TCC / 无 | |
| ② 并发安全 | 需要 / 不需要 | 乐观锁 / 分布式锁 / 无 | |
| ③ 幂等性 | 需要 / 不需要 | 幂等键 / 唯一索引 / 状态机 / 无 | |
| ④ 同步/异步解耦 | 同步 / 异步 | MQ / Outbox / 线程池 / 无 | |
| ⑤ 数据一致性 | 强一致 / 最终一致 | 本地事务 / 补偿 / 无 | |
| ⑥ 外部依赖容错 | 有 / 无 | 超时 / 重试 / 降级 / 无 | |
| ⑦ 性能瓶颈 | 有 / 无 | 索引 / 批量 / 限流 / 无 | |
| ⑧ 资源隔离 | 需要 / 不需要 | 独立线程池 / 分级队列 / 读写分离 / 无 | |
| ⑨ 安全 | 需要 / 不需要 | 鉴权 / 加密 / 参数校验 / 无 | |
| ⑩ 可观测性 | 已覆盖 / 未覆盖 | 日志 / Metrics / Trace / 告警 | |
| ⑪ 可运维性 | 已覆盖 / 未覆盖 | 开关 / 回滚 / 灰度 / 无 | |

**门禁：** 任一维度结论为空或"不知道" → 停止，向上游（Story / TestCase / DR）追溯补充信息，补充完毕后重新填写本表。

---

## 约束文件引用

> **🆕 2026-06-10 解耦改造：** 不再直接写死路径。实现前通过 `document-storage-skill.get_constraints(projectKey)` 加载约束——约束文档随工程走，SKILL 不知道它们在哪里。

实现前**必须通过 `document-storage-skill.get_constraints(projectKey)` 加载**以下约束：

| 约束 name | 关键规则 |
|---------|---------|
| `technology-stack` | Java 版本、Spring Boot 版本、框架版本 |
| `project-structure` | 包路径规范、模块结构、分层职责红线 |
| `layered-arch` | 分层依赖方向、各层职责 |
| `code-style` | 命名规范、Lombok 使用、异常定义、枚举结构、**日志格式三要素（`[服务][类][方法][业务动作] key=value`）** |
| `api` | URL 命名、HTTP 方法、响应结构 |
| `database` | 建表规范、必备字段、索引命名 |
| `security` | 鉴权方式、@SkipAuth 使用 |
| `testing` | 测试分层、Mock 策略、覆盖率要求 |
| **`be-coding-thinking-engine`** | **设计→实现→测试全链路思考框架（通过 `document-storage-skill.get_thinking_engine(projectKey)` 加载）**（已在第零步加载）|

---

## 第一步：收集输入

向开发者请求以下信息：

```
请提供：
1. Story 文档路径（.md 文件）— 通过 `documentStorage.resolve_path(intent="STORY", storyId)` 取得
2. Task 文档目录（该 Story 的 task/ 子目录）— 通过 `documentStorage.resolve_path(intent="TASK", storyId, taskId)` 取得
3. 测试用例文档路径 — 通过 `documentStorage.resolve_path(intent="TESTCASE", storyId)` 取得
4. 工作目录（各工程所在的磁盘根路径，如 E:\softWare\Project）
```

如果开发者已在消息中提供了这些信息，直接进入第二步。Task 文档目录和测试用例路径如未显式提供，按 `documentStorage.resolve_path()` API 自动定位（详见 `document-storage-skill.md §0.6.1`）。

---

## 第二步：阅读约束文档

> **🆕 2026-06-10 解耦改造：** 通过 `document-storage-skill.get_constraints(projectKey)` 加载，不直接读目录路径。

从 `get_constraints(projectKey)` 返回的 `ConstraintList` 逐项读取，记住关键规则：

| 约束 name | 关键规则 |
|------|---------|
| technology-stack | Java 版本、Spring Boot 版本、框架版本 |
| project-structure | 包路径规范、模块结构 |
| layered-arch | 分层依赖方向、各层职责 |
| code-style | 命名规范、Lombok 使用、异常定义、枚举结构、**日志格式三要素（`[服务][类][方法][业务动作] key=value`）** |
| api | URL 命名、HTTP 方法、响应结构 |
| database | 建表规范、必备字段、索引命名 |
| security | 鉴权方式、@SkipAuth 使用 |
| testing | 测试分层、Mock 策略、覆盖率要求 |
| **be-coding-thinking-engine** | **设计→实现→测试全链路思考框架；11维风险决策树；基准过滤器7条；每个操作的4问决策链** |

---

## 第三步：阅读 Story 文档

阅读 Story 文档，提取以下信息：

1. **涉及工程**：工程名、父工程、类型
2. **主流程 + 实现伪代码**：主流程业务步骤 + 实现伪代码的分层调用骨架（哪层调哪层、调用顺序、事务边界、领域逻辑落点）——这是实现的大方向骨架，编码时按此骨架填充各层，🔴 不得偏离伪代码体现的分层职责（领域逻辑归 Domain、编排归 Application、Repository 只存取）
3. **实现任务映射**：Task 执行顺序、Task 文档链接
4. **接口契约**：REST 接口和 SPI 接口定义（含出入参四维）
5. **数据模型 + DB 操作逻辑**：表结构、索引、CRUD 运行时逻辑（触发条件/WHERE/字段变更/幂等键）
6. **偏离声明**：与约束不同的特殊处理

---

## 第三步 bis：阅读测试用例文档

> **目的：** 测试用例定义了每个功能点的验收边界。编码前先读测试用例，才能写出"能通过用例"的代码，避免编码完成后才发现场景覆盖不到而返工（测试驱动思路）。

阅读测试用例文档，提取以下信息：

1. **覆盖的场景清单**：每个 AC 对应的正常/异常/边界/并发场景
2. **测试分层**：哪些用例是 L1（接口）/L2（应用）/L3（领域）/L4（基础设施）
3. **Mock 点**：用例中标记的外部依赖 Mock 位置（决定代码中哪些依赖要可注入/可替换）
4. **预期输入输出**：每个用例的 Given-When-Then，作为方法实现的验收依据
5. **错误码断言**：异常场景预期返回的错误码（编码时必须与之一致）

**编码时的对照规则：**
- 每个 Task 的核心方法实现前，先找到对应的测试用例，明确该方法要让哪些用例 Pass
- 实现的分支逻辑必须覆盖测试用例中的所有场景（正常 + 异常 + 边界）
- 抛出的异常错误码必须与测试用例的断言一致

---

## 第四步：阅读 Task 文档

### 4.1 先读 Task 0（公共依赖说明）

Task 0 包含所有 Task 共用的信息：
- 公共类完整包路径（Result、@SkipAuth、@NotBlank 等）
- 技术栈版本
- 包路径约定（固定，不可自行命名）
- 跨 Story 依赖的表结构和 DO 定义
- 公共接口定义
- MetaObjectHandler 配置

### 4.2 按执行顺序逐个读取 Task

从 Story 的「实现任务映射」中获取执行顺序，按顺序读取每个 Task 文档。

**🆕 2026-06-10 任务规模判定：**

- **重任务** → 走原 §4.2 全章节读取流程
- **小任务** → 走原 §4.2 全章节读取流程（无 Story 但有 Task 文档）
- **微任务** → 跳过 §4.2 全文读取（无 Task 文档），直接进入第 5 步工程预检，第 6 步按 CodingPlan 实施

**每个 Task 文档的读取顺序与提取目标：**

| 章节 | 提取目标 | 用途 |
|------|---------|------|
| `## 元信息` | 涉及工程/层、状态 | 确认本 Task 的实现范围 |
| `## 依赖关系` | 前置 Task、被依赖 Task | 确认执行顺序，前置未完成则停止 |
| `## 包路径约定` | 各层完整包路径 | 写代码时固定使用，不可自行命名 |
| `## 核心设计` | 决策基线引用 + 关键设计机制（状态机/幂等/锁） | 理解设计意图，避免偏离 |
| `## CodingModel 决策记录` | 11 维结论 + 核心链路保护 | **复核**（不重新填写）：验证 Execute 阶段与 Plan 阶段结论一致，任一冲突立即停止追溯 |
| `## 任务级 CodePlan → 类骨架` | 类名、层、包路径、依赖字段 | 确定文件结构，创建类文件 |
| `## 任务级 CodePlan → 依赖工具包` | import 列表 + 用途 | 写 import，不引入计划外依赖 |
| `## 任务级 CodePlan → 方法级逻辑` | 方法签名、逻辑步骤、异常处理 | 骨架展开的主要依据（见 §6.1 展开规则） |
| `## 任务级 CodePlan → DB 操作` | WHERE/幂等键/事务边界/并发控制 | 生成 Mapper/Repository 的 SQL 和锁 |
| `## 任务级 CodePlan → 外部依赖` | 超时/重试/降级/是否阻塞主流程 | 生成 Feign 调用的容错代码 |
| `## 任务级 CodePlan → 测试映射` | TestCase ID + 覆盖点 + 真实DB/HTTP | 确认本 Task 要让哪些用例 Pass |
| `## 约束检查` | 已列约束项 | 编码时反复对照，违反即停止 |
| `## 验收映射` | AC → 实现点 | 完成后自验，确认 AC 已覆盖 |

> **🔴 CodingModel 决策记录是只读的。** Execute 阶段不重新分析 11 维，只做复核（Plan 阶段已决策）。若发现 Execute 阶段的实际情况与 Plan 阶段结论不符，走实时追溯链（Task → Story → DR）先修文档再继续实现。

---

## 第五步：工程预检

### 5.1 确认工程存在

对每个涉及工程，在工作目录下查找其磁盘路径。若工程不存在，创建子模块并注册到父 pom。

### 5.2 检查依赖完整性

对每个涉及工程的 pom.xml，检查必要依赖是否存在：

| 模块类型 | 必须依赖 |
|----------|----------|
| Domain | icec-cloud-commons、lombok |
| Application | domain 模块、im-spi 模块、icec-cloud-commons、lombok |
| Infrastructure | domain 模块、application 模块、mybatis-plus、lombok、第三方 SDK |
| Interfaces | application 模块、im-spi 模块、spring-web、lombok |

### 5.3 验证第三方 SDK 包路径

若 Task 涉及第三方 SDK：
1. 在本地 Maven 仓库（`~/.m2/repository`）中找到对应 jar
2. 解压查看实际类路径
3. 记录正确的 import 前缀

### 5.4 确认已有代码模式

扫描工程中已有的 Java 文件，确认：
- `Result<T>` 的完整包路径
- `@NotBlank` 的来源包
- `@SkipAuth` 的来源包
- 已有 Converter、Repository、Controller 的代码风格

---

## 第六步：按 Task 顺序生成代码

**严格按 Task 执行顺序生成代码。每完成一个 Task，对照其检查项自检。如果 Task 间的依赖关系不清晰或存在循环依赖，必须先询问用户确认执行顺序，不得自行假设。**

### 6.0 🆕 2026-06-10 任务规模 × 文档组合

| 任务规模 | 生成依据 | 流程深度 | 文档数量 |
|---------|---------|---------|---------|
| **重任务** | Story + Task 文档 + CodingPlan（4 件套）| 全流程 | Story / Task / CodingPlan / Coding 报告 / CodeReview 报告 / 测试报告 |
| **小任务** | Task 文档 + CodingPlan（2 件套）| 全流程（跳 Story）| Task / CodingPlan / Coding 报告 / CodeReview 报告 / 测试报告 |
| **微任务** | CodingPlan（1 件套）| 全流程（跳 Story + 跳 Task）| CodingPlan / Coding 报告 / CodeReview 报告 / 测试报告 |

**全流程不变（重/小/微 三类 100% 一致）：**
- CodingModel 11 维决策 + 核心链路保护
- CodingPlan 14 条门禁
- 全切面一致性核查闸（⑥bis）
- 全链路对称性闸（⑦bis）
- CodeReview 6 阶段评审
- TR-1~TR-7 全过程（自动跳过 Story/Task 相关项）

### 6.1 生成规则

| 规则 | 说明 |
|------|------|
| **骨架填肉，按序展开** | Task 文档给的是骨架（方法签名 + 伪代码），Coding SKILL 按 §6.1.1 展开规则"填肉"，不自行发挥结构 |
| 包路径固定 | 使用 Task 0 和各 Task 中定义的包路径，不可自行命名 |
| 字段类型严格 | 字段类型以 Task 文档为准（特别注意 String vs Long） |
| 约束优先 | 约束文档的规则优先于 Task 文档中的示例代码 |
| 冲突处理 | Task 文档与约束文档冲突时，以 Task 文档为准（Task 文档已经过约束检查） |
| 🔴 分层职责归位 | 写每个方法前先问"这段属于哪层"，严禁串味（见下） |

> ### 🔴 分层职责红线（写代码时反复对照，违反即阻断）
>
> **Domain 写领域逻辑，Application 写业务编排，Repository 只做数据存取。** 完整清单见 `get_constraints(projectKey)["project-structure"]` 的「分层职责红线」节。写代码现场用判定口诀自查：
>
> | 这段代码是… | 归属层 | 落点 |
> |---|---|---|
> | 业务规则 / 能不能 / 算什么（状态能否流转、金额怎么算、不变量校验） | **Domain** | 实体充血方法 / DomainService |
> | 先做A再做B / 协调谁调谁 / 事务从哪到哪 / 转 DTO | **Application** | AppService |
> | 把数据存进去 / 取出来 / 转 PO↔DO 格式 / 拼查询条件 | **Repository** | RepositoryImpl |
> | 参数格式校验（@Valid） | **Interfaces** | Controller/Impl |
>
> **🔴 Repository 绝对禁止：** 状态流转判断、业务规则校验、跨聚合编排、存取方法里塞 if-业务分支。仓储方法名只能是 `findByXxx`/`save`/`updateStatus` 这类存取语义；一旦出现 `handleXxx`/`processXxx`/`checkXxx业务` 就是放错层。
> **🔴 Application 绝对禁止：** 写领域规则（下沉到 Domain）、写 SQL/持久化细节。
> **🔴 Domain 绝对禁止：** 串多个外部服务的编排、出现 PO/DTO/SQL。

---

### 6.1.1 骨架展开规则（Task 伪代码 → 完整方法体）

> Task 文档的"方法级逻辑"表格中，每个逻辑步骤以动词开头（校验/查询/调用/转换/返回/抛异常）。  
> Execute 阶段按以下展开规则，将每个动词步骤翻译成完整代码。

| 伪代码动词 | 展开规则 | 示例 |
|---------|---------|------|
| **校验** xxx | 优先用 `@Valid` / `@NotBlank` 做入参校验；业务规则用 Domain 实体的校验方法；查不到实体用 `Optional.orElseThrow(() -> new XxxException(ErrorCode.XXX))` | `user = userRepository.findById(id).orElseThrow(...)` |
| **查询** xxx | 按 Task DB 操作表格的 WHERE 条件调 Repository 方法；查询结果 Optional 处理，不得裸调 `.get()` | `repository.findByXxx(param)` |
| **调用** 外部服务 | 按 Task 外部依赖表格的超时/重试/降级填入；非幂等操作禁止自动重试，必须有幂等键 | Feign + `@HystrixCommand(fallbackMethod="...")` |
| **转换** | 调用 Converter 静态方法（`XxxConverter.toXxx(source)`），禁止在 AppService/Controller 内手工 set 字段 | `XxxConverter.toVO(do)` |
| **返回** | 明确返回值构造方式（从 DO 转换 / 直接返回布尔 / 包装成 ApiResult）；不得返回 null，空集合用 `Collections.emptyList()` | `return ApiResult.success(vo)` |
| **抛异常** | 使用项目已有的业务异常类 + 错误码枚举（来源：Task 0 或 `get_constraints(projectKey)["code-style"]`）；禁止直接 throw new RuntimeException | `throw new BizException(ErrorCode.XXX)` |
| **组装** | 构造复合对象时，先列出所有必填字段来源，逐字段赋值；Builder 模式优先 | `XxxDO.builder().field(val).build()` |
| **发送** MQ | 先写库（事务内），后发 MQ（事务外）；失败需有本地消息表或 DLQ 兜底 | 事务提交后在 `@TransactionalEventListener` 发送 |

**展开顺序（每个方法体固定执行）：**
```
Step 1：看类骨架 → 确定类注解、字段依赖
Step 2：看方法级逻辑表格 → 确定方法签名 + 每步动词
Step 3：按展开规则翻译每个动词步骤为代码
Step 4：看 DB 操作表格 → 填入 WHERE/幂等键/事务注解/锁
Step 5：看外部依赖表格 → 填入超时/重试/降级配置
Step 6：看测试映射 → 确认异常分支覆盖了所有 TestCase 场景
```

### 6.2 每个 Task 的生成流程

```
0. 🔍 Task 实现方案确认（必须通过）
   │
   ├── 读取 Task 文档所有章节（按 §4.2 读取顺序）
   ├── 复核 CodingModel 决策记录（不重新分析，只验证与统一版 CodingPlan 一致）
   ├── 向用户呈现本 Task 实现方案（见 §6.2.1）
   └── 用户确认后才能进入第 1 步

1. 按类骨架创建文件（类签名 + 注解 + 字段）
2. 确认前置 Task 已完成（依赖类存在可导入）
3. 按骨架展开规则（§6.1.1）逐方法填肉
   ├── Step 1-6 顺序执行（骨架→逻辑→DB→外部依赖→测试映射）
4. 对照约束检查章节逐项自查
5. 对照验收映射验证 AC 覆盖
6. 标记 Task 完成
```

### 6.2.1 Task 实现方案确认（强制节点）

> **每个 Task 开始前必须向用户呈现并确认实现方案，用户确认后才可开始写代码。未确认禁止写代码。**  
> **本节内容来自 Task 文档的已有章节，不重新分析。CodingModel 决策已在 Plan 阶段完成。**

**向用户呈现的内容：**

```
【Task {N} 实现方案确认】

Task 名称：{Task 名称}
涉及工程/层：{Domain / Application / Infrastructure / Interfaces}

零、CodingModel 决策复核（来源：Task 文档 ## CodingModel 决策记录，只读不改）
┌─────────────────────────────────────────────────────────────┐
│ ① 原子性：{Task 文档中的结论}   方案：{Task 文档中的处理方案} │
│ ② 并发安全：{结论}   方案：{方案}                           │
│ ③ 幂等性：{结论}   方案：{方案}                             │
│ ④ 同步/异步：{结论}   方案：{方案}                          │
│ ⑤ 数据一致性：{结论}   方案：{方案}                         │
│ ⑥ 外部依赖容错：{结论}   方案：{方案}                       │
│                                                             │
│ 复核结论：与统一版 CodingPlan 一致 ✅ / 发现冲突 ❌→停止追溯  │
└─────────────────────────────────────────────────────────────┘

一、类与包路径（来源：## 任务级 CodePlan → 类骨架）
{列出本 Task 涉及的所有类，含完整包路径}

二、依赖工具包（来源：## 任务级 CodePlan → 依赖工具包）
{列出 import 列表}

三、方法实现计划（来源：## 任务级 CodePlan → 方法级逻辑，按 §6.1.1 展开）
{每个方法列：签名 + 展开后的实现要点 + 异常处理}

四、DB 操作（来源：## 任务级 CodePlan → DB 操作）
- 表：{表名}
- 操作：{INSERT/UPDATE/DELETE/SELECT}
- WHERE / 幂等键：{具体条件}
- 事务边界：{哪个方法加 @Transactional}
- 并发控制：{乐观锁/分布式锁/无}

五、外部依赖（来源：## 任务级 CodePlan → 外部依赖）
- 依赖：{接口名}
- 超时：{连接Xs / 读Xs}
- 重试：{次数/策略}
- 降级：{降级方案}
- 是否阻塞主流程：{是/否}

六、测试用例覆盖（来源：## 任务级 CodePlan → 测试映射）
- 需要 Pass 的 TestCase：{ID 列表}
- 覆盖场景：{正常/异常/边界/并发}
- 是否需要真实 DB/HTTP：{是/否}

七、基准过滤器自检（来源：be-coding-thinking-engine §基准过滤器）
- [ ] ①可用性：方案能正常完成业务目标？
- [ ] ②正确性：骨架逻辑步骤覆盖所有 TestCase 场景（含异常分支）？
- [ ] ③高效性：无循环IO、无慢SQL、无N+1？
- [ ] ④可维护性：分层清晰、每方法 ≤50 行？
- [ ] ⑤健壮性：CodingModel ①②③⑥维度方案已体现在展开代码中？
- [ ] ⑥可读性：命名见名知意、无魔法值？
- [ ] ⑦可演进性：无跨层调用、无强耦合？
- [ ] 包路径与 Task 0 一致？
- [ ] 字段类型与 Story 一致？
```

**用户选项：**
- ✅ 确认实现方案，无误
- ⚠️ 需要调整（说明修改内容，AI 记录后等待再次确认）
- ❌ 暂停本 Task

> **强制要求：必须等待用户确认后才能开始写本 Task 的代码。同一个 Task 最多等待 3 次用户提出修改意见（用户每次说"需要调整"算 1 次），3 次后强制暂停并询问是否继续。确认语义：用户必须说"确认"、"同意"、"可以开始"，模糊回复需追问。**

### 6.3 代码生成顺序（对应 Task 顺序）

| 顺序 | Task | 生成内容 |
|------|------|---------|
| 1 | Task 2.1 | PO → Mapper → Converter |
| 2 | Task 2.2 | Enum → Exception → DO → Context → Hook 接口 → HookRegistry → Repository 接口 → Gateway 接口 |
| 3 | Task 2.4 | Config → Gateway 实现 → Facade → Repository 实现 → MetaObjectHandler |
| 4 | Task 2.3 | DTO → AppService |
| 5 | Task 2.5 | Controller |
| 6 | Task 2.6 | 单元测试 |

---

## 第七步：编译 + 服务启动 + 接口验证 + DB 验证 + 异常验证（必须通过）

> **核心原则：`mvn compile` 通过 ≠ 完成。必须逐维验证全部通过，才算本步骤完成。**

### 7.1 逐工程编译（6.1）

> **强制要求：必须在父工程根目录执行 `mvn compile`，不允许只编译子模块。分模块编译会漏掉跨模块的依赖问题（如 interfaces 层未实现接口、service 层缺少 Bean）。**

```bash
cd {parent-project-root} && mvn compile
```

**通过标准：** BUILD SUCCESS，无 error，所有子模块（domain/application/infrastructure/interfaces/service）全部编译通过。

**如失败：** 读取错误信息，定位问题文件和行号，修复后重新编译。不得只编译通过的子模块就认为完成。

---

### 7.2 服务启动验证（6.2）

```bash
cd {parent-project-root} && mvn spring-boot:run
```

**通过标准（三项全部满足，不是三选一）：**
- `curl localhost:{port}/actuator/health` 返回包含 `"status":"UP"`（响应可包含其他字段，不影响判定）
- `curl localhost:{port}/actuator/beans | grep {本Story新增的Bean名}` 确认本 Story 的 Bean 已注册
- 启动日志含 `Started XxxApplication in X seconds`，无 `BeanCreationException`、`BeanNotOfRequiredTypeException`

**启动失败处理原则：必须定位根因，禁止绕过。**

| 失败现象 | 正确处理 | 禁止做法 |
|---------|---------|---------|
| `Port already in use` | 读启动日志，查明是哪个进程占用端口、为什么冲突（如 Nacos 下发了错误端口配置），修复配置 | ❌ 直接 kill 占端口进程后重试 |
| `BeanCreationException` | 检查 @Autowired 依赖、Bean 扫描路径，确认接口实现类存在 | ❌ 注释掉报错的 Bean 绕过 |
| `DataSource connection failed` | 检查数据库连接配置，确认 DB 可达 | ❌ 改用内存 DB 绕过 |
| `BeanNotOfRequiredTypeException` | 检查接口实现类是否匹配，确认没有多个实现类冲突 | ❌ 强制类型转换绕过 |

> **启动失败属于 🔴 阻断型问题，必须修复后重新验证，不得跳过或绕过。若失败原因是外部服务不可用（如数据库断开、注册中心不可达），记录原因并暂停，不可用修改代码绕过外部依赖问题，需由人工介入解决。**

---

### 7.3 主流程接口测试（6.3）

> 🔴 **能走真实 HTTP 的接口测试必须走真实 HTTP。** L2 接口测试默认用 `@SpringBootTest(webEnvironment = RANDOM_PORT)` + `TestRestTemplate`，经真实端口、真实网络、真实容器栈验证；Service 层用 `@MockBean` 隔离。MockMvc 不开真实端口、不走真实网络，仅在框架过老无法启动嵌入式容器时降级，且须在测试报告注明降级原因（详见 `strategies/be-testcase-strategy.md` 强制原则）。

```bash
cd {service-root} && mvn test -Dtest=*ApiIT,*ControllerIT
```

**通过标准：**
- 主流程对应接口（AC-001 场景）经**真实 HTTP**测试 Pass
- HTTP 状态码 200
- 响应结构与 Story 接口契约一致（字段名、类型、数量）
- 测试经过真实端口（RANDOM_PORT），非 MockMvc 模拟（除非已注明降级原因）

**如无真实 HTTP 测试类：**
手动用 curl 调用并验证（curl 本身就是真实 HTTP）：
```bash
curl -X POST http://localhost:{port}/api/xxx -H "Content-Type: application/json" -d '{"param": "value"}'
```
检查返回状态码和响应体与 Story 一致。

---

### 7.4 错误码映射验证（6.4）

执行异常场景测试，验证错误码映射正确：

| 场景 | 预期结果 |
|------|---------|
| 参数为空 | HTTP 400 + Story 定义的错误码 |
| 状态非法流转 | HTTP 400 + 2104X（根据 Story） |
| 未登录访问 | HTTP 401 或 Story 定义的错误码 |
| 服务内部异常 | HTTP 500 或 Story 定义的兜底错误码 |

**如 Story 未定义错误码：** 走系统统一错误码（100500 等）。

---

### 7.5 DB 写操作落库验证（6.5）

执行 L3 集成测试（真实 H2/TestContainers DB）：

```bash
cd {service-root} && mvn test -Dtest=*IntegrationTest
```

**验证点：**
- INSERT 后 SELECT 能查到（主键生成策略生效）
- UPDATE 后字段值与预期一致（乐观锁 version 递增）
- 逻辑删除字段生效（is_deleted = 1）
- 事务提交后数据对其他事务可见

**如无 L3 测试：** 手动执行 SQL 验证：
```sql
SELECT * FROM {table} WHERE id = ?;
-- 验证字段值是否符合预期
```

---

### 7.6 事务边界验证（6.6）

**验证方式：** 查看启动日志 + 分析代码 `@Transactional` 标注

**验证点：**
- 事务内操作失败（如校验不通过）→ 数据库无污染（数据未写入）
- 事务外操作（通知、消息）不在事务内，不阻塞主流程
- 事务方法调用链与 Story 描述一致

**如不确定：** 在事务方法内故意抛异常，确认数据库回滚。

---

### 7.7 所有测试 Pass（6.7）

```bash
cd {service-root} && mvn test
```

**通过标准：** BUILD SUCCESS，无 FAILURE。

**如 L4 测试失败：** 检查是否依赖外部系统（外部系统不可用可暂时跳过，但必须在报告里说明）。

---

### 7.8 完成标准汇总

**以下全部通过，本步骤才算完成：**

| # | 门禁 | 验证命令/方式 |
|---|------|-------------|
| 6.1 | mvn compile 通过（全工程） | 父工程根目录 `mvn compile` → BUILD SUCCESS，所有子模块通过 |
| 6.2 | 服务启动成功 | health 端点 UP + 本 Story Bean 已注册 + 无 BeanCreationException |
| 6.3 | 主流程接口 Pass | `mvn test -Dtest=*ControllerTest` → Pass（或 curl 手动验证） |
| 6.4 | 错误码映射正确 | 异常场景测试返回正确 HTTP 状态码 + 错误码 |
| 6.5 | DB 写操作落库 | L3 测试 PASS 或手动 SQL 验证 |
| 6.6 | 事务边界正确 | 代码分析 + 事务回滚验证 |
| 6.7 | 所有测试 Pass | `mvn test` → BUILD SUCCESS |

**任意一项未通过 → 修复 → 重新验证 → 全部通过才进入测试阶段。**

---

## 第八步：运行测试

### 8.1 执行测试

```bash
cd {service-root} && mvn test
```

### 8.2 出具测试报告

每次测试执行后，必须生成测试报告文档：

**文件路径：** 通过 `documentStorage.resolve_path(intent="TEST_REPORT", storyId, version={v:N,r:M})` 自动定位（详见 `document-storage-skill.md §0.6.1`），按 Story 分子目录存放

```
ae-sdd-doc/iterations/{YYYY-MM-DD}/Test/
├── STORY-002-BE/
│   ├── STORY-002-BE-testcase-v1.0.md         ← 测试用例
│   └── STORY-002-BE-Report-v1-r1.md          ← 测试报告
├── STORY-007-BE/
│   ├── 2c-im-testcase-007-客服会话状态机-BE.md
│   └── 2c-im-testcase-007-客服会话状态机-BE-Report.md
└── ...
```

**报告内容：**

```markdown
# {Story ID} - 测试报告

## 执行信息

| 项 | 值 |
|----|-----|
| 执行时间 | {日期时间} |
| 执行轮次 | 第 {N} 轮 |
| 触发方式 | Coding SKILL 自动触发 |

## 执行结果

| 指标 | 值 |
|------|-----|
| 总用例数 | {X} |
| 通过 | {Y} |
| 失败 | {Z} |
| 跳过 | {W} |
| 通过率 | {Y/X * 100}% |

## 用例明细

| 用例 | AC ID | 结果 | 失败原因 |
|------|-------|------|---------|

## 失败用例分析

| 用例 | 失败原因 | 根因分类 | 处理方式 |
|------|---------|---------|---------|
```

### 8.3 测试失败处理

测试失败时，进入异常路径（A1-A6），分析根因并修复，修复后重新执行测试。

---

## 第八步 bis：测试报告合规性校验

**触发时机：** 每次测试执行并出具报告后

**输入：** 测试报告 + 原始测试日志 + Surefire/Failsafe XML + `scripts/test_authenticity_scan.py` 扫描报告 + `strategies/be-testcase-strategy.md` 模板 + 测试用例文档

**🔴 原始证据归档（缺任一项 = 测试报告无效）：**

| 证据 | 路径/来源 | 说明 |
|---|---|---|
| 实际执行命令 | 测试报告"执行证据"章节 | 必须包含完整命令、工作目录、Profile、环境变量摘要、退出码；禁止只写"mvn test 已通过" |
| 原始 stdout/stderr | `.auto-engineering/{STORY-ID}/evidence/test-run-v{N}-r{M}.log` | 由真实命令输出落盘，失败栈必须原样保留 |
| Surefire XML | `{module}/target/surefire-reports/TEST-*.xml` | 解析 `tests/failures/errors/skipped` 作为报告统计来源 |
| Failsafe XML | `{module}/target/failsafe-reports/TEST-*.xml` | 有 IT/verify 时必须解析；没有时在报告说明"本 Story 无 Failsafe 测试" |
| 测试真实性扫描 | `.auto-engineering/{STORY-ID}/evidence/test-authenticity-scan-v{N}-r{M}.md` | 由脚本生成，BLOCKER 必须为 0 |
| AC 对账表 | 测试报告"AC × 测试方法对账"章节 | 每个 AC 至少映射到一个真实执行过的测试方法 |

**🔴 真实执行命令规范：**

```bash
python document/life-team-ai-standards/skills/ae-sdd/scripts/test_authenticity_scan.py --root {service-root} --require-reports --output .auto-engineering/{STORY-ID}/evidence/test-authenticity-scan-v{N}-r{M}.md
```

- 跑测试命令时禁止出现 `-DskipTests`、`-Dmaven.test.skip=true`、`testFailureIgnore=true`。
- 模块级验证必须写清楚 `mvn -pl {module} -am test/verify` 的实际模块名；全量完成优先 `mvn clean verify`。
- 只跑单个 `-Dtest=...` 不能声明"全量通过"，只能声明该测试类/方法通过。
- XML 中 `failures > 0` / `errors > 0` / `skipped > 0` 默认阻断；`skipped` 只有在 Story "可跳过 AC/用例"章节已有明确记录、负责人和补测时间时才允许降级。
- 测试报告中的用例总数、失败数、跳过数必须来自 XML 解析结果，不得人工估算。

**校验清单：**

| 检查项 | 说明 |
|--------|------|
| 证据链完整 | 原始命令、日志、XML、扫描报告、AC 对账表是否都存在且路径可打开？ |
| XML 对账 | 测试报告统计是否与 Surefire/Failsafe XML 一致？是否存在未解释的 skipped？ |
| 跳测参数扫描 | 命令、POM、插件配置中是否存在 skipTests / maven.test.skip / testFailureIgnore？ |
| 真实性脚本扫描 | `test_authenticity_scan.py` 是否 BLOCKER=0？WARN 是否逐项解释？ |
| 测试发现数对账 | 测试用例文档应跑数、测试源码方法数、XML 实际执行数是否一致或有合理解释？ |
| 用例层级覆盖 | L1/L2/L3/L4 是否都有对应的用例执行？ |
| 失败原因分类 | 失败用例的根因是否已正确分类（代码 bug/ 环境问题/ 用例设计问题）？ |
| 假修复识别 | 是否识别出"测试通过但实际代码有问题"的情况？ |
| 补充测试建议 | 用例设计不足时，是否有补充建议？ |

**假修复识别规则：**
- 缺原始日志或 XML，只在 Markdown 中写"通过" → 虚报成功
- 测试命令带跳测参数，或 POM 中配置跳过/忽略失败 → 虚报成功
- XML 实际执行数小于测试用例应跑数，且无明确解释 → 假修复风险
- `test_authenticity_scan.py` 出现 BLOCKER → 测试无效
- 测试用例覆盖不完整（未按模板生成）→ 假修复风险
- 某测试层级完全缺失（如只有 L1，没有 L2/L3）→ 假修复风险
- 失败用例被标记"跳过"而非真正修复 → 假修复
- 通过率异常高（如 100%）但 L3/L4 缺失 → 假修复风险
- 核心 AC 只有 happy path，没有失败注入/负向断言 → 假修复风险

**判定结果：**
- ✅ 无假修复风险 → 进入完成判定
- ⚠️ 有假修复风险 → 补充测试用例 → 重新执行测试
- 🔴 证据链缺失 / XML 不一致 / 扫描 BLOCKER / 跳测参数命中 → 测试报告作废，回到第八步重新执行真实测试

---

## 第九步：编码后全切面一致性核查闸（🔴 强制，CodeReview 的硬前置，每轮编码完成后立即跑）

> **每一轮 Coding 完成后立即强制执行**（含缺陷修复轮、增量补充轮），不可因"只改了一点"而跳过。这是补齐"编码后无人回头核对设计与代码是否还一致"的真空——历史上 STORY-002 改 5 轮仍漂移，正因缺这道闸。详见 [[post-coding-cross-cut-consistency-gate]]。
>
> **历史教训：** STORY-021-BE 实施时 AI 只跑了编译 + 测试，**没跑**这一道闸，直接出 Coding Report——结果"在 `ImSessionAppService` 写了纯数据访问封装"的分层错误漏到用户反馈才发现。🔴 本闸的触发时机不是"出 CodeReview 才跑"，而是"代码写完立即跑"——出 CodeReview 之前应该已经跑过本闸并修复完毕。

**触发时机：** 代码写完（"按 Task 顺序生成代码"完成后）立即跑，跑完后再跑闸 7 静态扫描、闸 5 真实 DB 集成测试，最后才进 CodeReview。
> 
> **跑通顺序：写代码 → 跑本闸（一致性）→ 跑闸 7（静态扫描）→ 跑编译 + 启动 + 接口测试 + DB 测试 → 出 Coding Report → 出 CodeReview 报告。**

**核查方向（关键）：以实际代码为锚点反向核查，不是拿文档去代码里找。**
- ❌ 错误：拿 Story 逐条找"代码实现了没"——会漏掉代码多做/做歪但文档未写的部分。
- ✅ 正确：拿每段**代码实际行为**反查它对应哪条 DR / Story / Task / 测试用例，确认五方一致。

**核查范围：🔴 全章节全文件，禁止只核当轮 diff**（本轮改动可能让前几轮已"一致"的部分失效）。见 [[codereview-full-doc-rescan-gate]]。

**核查方式：🔴 禁止裸 ✅，每行结论必须附客观证据**（类型/取值核对、文件:行号、真实 DB 输出）。见 [[evidence-bound-check-no-bare-checkmark]]。

**产出物：《全切面一致性核查表》**，嵌入 CodeReview 报告"零、"章节。逐行以代码为单位：

| 代码位置(文件:行号) | 代码实际行为 | DR | Story 章节 | Task | 测试用例 ID | 一致性结论 | 证据 |
|---|---|---|---|---|---|---|---|

**漂移处置：**

| 类型 | 处置 |
|---|---|
| 🔴 代码多做（设计无） | 必要→补回 DR/Story/Task；不必要→删代码 |
| 🔴 代码漏做（设计有） | 回第六步补实现 |
| 🔴 代码做歪（与设计冲突，如 ID 类型不符） | 设计对→改代码；代码对→按层级回改 DR/Story/Task |
| 🟠 测试用例未覆盖 | 补测试用例并跑通 |

**修复按层级回改并级联**（漂移定位到哪层就从那层向下级联到代码+测试），修复后**重新执行本闸全量核查**，直至无 🔴 漂移。

**🔴 核心落库路径真实 DB 硬门禁：** 凡 INSERT/UPDATE/DELETE 的核心路径，核查表"证据"列禁止填 Mock 结果，必须填真实 DB（H2/TestContainers）落库验证输出。全 Mock 视为"未验证"，按 🔴 漏做处理。

**出闸条件：** 核查表覆盖全部代码改动 + 无 🔴 漂移 + 核心落库路径有真实 DB 证据。未达标 → 回第六步修复，不允许进入 CodeReview。

---

## 完成标准

**Coding SKILL 的完成需同时满足以下条件：**

| 条件 | 说明 |
|------|------|
| ✅ 编译通过 | `mvn compile` 无错误 |
| ✅ 测试全部通过 | `mvn test` 所有用例 Pass |
| ✅ 问题反馈闭环 | 异常路径中无 Open 状态的问题 |
| ✅ 测试报告已出具 | 最终一轮测试报告已生成 |
| ✅ 全切面一致性核查闸通过 | 《全切面一致性核查表》无 🔴 漂移，核心落库路径有真实 DB 证据 |

**不满足时：** 循环执行"修复 → 编译 → 测试 → 出报告"，直到全部通过。

---

## 异常路径：Coding 实时追溯链（🔴 每次报错都走）→ 触发 Proposal

> **核心立场：** Coding 出现错误时，**先排除文档缺陷，再判定 AI 自己犯蠢**。文档缺陷（Task/Story/DR）必须先改文档、再改代码；只有文档都没问题时才允许直接改代码。
>
> **与历史"问题说明文档"流程的区别：**
> - **旧流程：** 每次报错 → 写"问题说明文档" → 一次性事后分析根因
> - **新流程：** 每次报错 → **立即走实时追溯链** → 自下而上逐层判定 Task → Story → DR → AI 犯蠢
> - 旧"问题说明文档"作为追溯结果的**记录载体**仍保留（每个判定结论都写入问题记录），但**判定时机前置到每次报错实时做**
>
> **🔴 核心约束（Code Plan 写在 Task 内）：**
> - **Coding 全程严格按 Task 文档实现** — Task 文档包含"实现方案"章节（即任务级 CodePlan）
> - **Coding 出问题一定先完善 Task 文档**（包括其内嵌的 CodePlan）→ 然后**重新照着 Task 文档来实现**
> - 这意味着追溯层 1 命中 = **修 Task 文档 = 修 CodePlan**（两者一体）
> - 重新 Coding = 严格按更新后的 Task 文档 + CodePlan 重新实现

**🔴 强制顺序（5 步，命中即处理，不可跳层）：**

```
发现 Coding 报错（编译失败 / 单测失败 / 接口测试失败 / 真实 HTTP 失败 / SQL 错误 / 事务回滚异常 / 性能问题）
    │
    ▼
追溯层 1️⃣：先读 Task 文档
    │   ┌──────────────────────────────────────┐
    ├──┤ 判定：Task 文档是否写错/写漏？              │
    │   │  - 核心代码/方法签名/字段类型/依赖/包路径  │
    │   │  - 是否与 Story/AC 矛盾？                │
    │   └──────────────────────────────────────┘
    │   ├── 🔴 Task 有错 → 修 Task 文档 → 重新生成该 Task 的 CodePlan → 重新 Coding（回到正常路径）
    │   └── ✅ Task 无误 → 进入追溯层 2
    │
    ▼
追溯层 2️⃣：再读 Story 文档
    │   ┌──────────────────────────────────────┐
    ├──┤ 判定：Story 文档是否写错/写漏？             │
    │   │  - 接口契约/数据模型/字段类型/异常流程     │
    │   │  - AC 是否与 DR 矛盾？                  │
    │   └──────────────────────────────────────┘
    │   ├── 🔴 Story 有错 → 写 Supplement → 触发 Story Update SKILL → 触发 Task Generate SKILL 重新生成受影响的 Task → 重新 Coding
    │   └── ✅ Story 无误 → 进入追溯层 3
    │
    ▼
追溯层 3️⃣：照例去检查 DR
    │   ┌──────────────────────────────────────┐
    ├──┤ 判定：DR 文档是否写错/写漏？                │
    │   │  - 业务规则本身是否有漏洞/边界条件遗漏     │
    │   │  - 是否与 PRD/上游约束矛盾？             │
    │   └──────────────────────────────────────┘
    │   ├── 🔴 DR 有错 → 写 DR 补充说明 → 触发 DR Update SKILL → 通知受影响的所有 Story → 重新 Story Review → 重新 Task Generate → 重新 Coding
    │   └── ✅ DR 无误 → 进入追溯层 4
    │
    ▼
追溯层 4️⃣：判定"AI 自己犯蠢"（兜底）
    │   ┌──────────────────────────────────────┐
    ├──┤ 前提：追溯层 1/2/3 全部判定"无误"          │
    │   │  类型：typo / 漏 import / 笔误 / API 误用  │
    │   └──────────────────────────────────────┘
    │   ├── ✅ 纯实现问题 → 写入问题记录（log）→ 直接修代码 → 继续 Coding
    │   └── ❌ 不是纯实现问题（异常场景）→ 升级用户决策
    │
    ▼
继续 Coding / 提交用户
```

**🔴 关键约束：**
- **禁止跳过追溯层 1/2/3 直接判定"AI 犯蠢"** — 这是 STORY-002 反复改 5 轮的根源
- **每层判定都要写入问题记录的"根因分析"字段** — 不允许"自我声明无误"
- **命中设计层（第 1/2/3 层）必须先改文档、再改代码** — 顺序不可颠倒
- **追溯不是"事后一次性"，而是"每次报错实时"** — 编译失败/单测失败/接口失败/真实 HTTP 失败/性能问题每次都要走

---

### 问题记录载体（追溯结果的沉淀位置）

> 实时追溯的结果必须落到问题记录里，供后续审计/复盘。

**文件位置：** 通过 `documentStorage.resolve_path(intent="CODING_ISSUE_LOG", storyId)` 自动定位（详见 `document-storage-skill.md §0.6.1`），与 CodingReport 同目录

**每条问题格式：**

```markdown
### 问题 {序号}：{问题标题}

- **发现时间：** {YYYY-MM-DD HH:mm}
- **所属 Task：** Task {ID}
- **报错类型：** 编译失败 / 单测失败 / 接口测试失败 / 真实 HTTP 失败 / SQL 错误 / 事务回滚异常 / 性能问题
- **追溯层 1（Task）：** □ Task 设计有错 / ✅ Task 设计无误
- **追溯层 2（Story）：** □ Story 设计有错 / ✅ Story 设计无误
- **追溯层 3（DR）：** □ DR 设计有错 / ✅ DR 设计无误
- **追溯层 4（AI 犯蠢）：** ✅ 纯实现问题（前 3 层均已排除） / ❌ 异常场景（升级用户）
- **根因分析：** {为什么会出现这个问题}
- **影响范围：** 约束文档 / Task 文档 / Story 文档 / DR 文档 / 仅代码
- **修复方案：** {计划如何修复}
- **状态：** Open / Fixed
```

**门禁：**
- 🔴 每次报错**必须**先走追溯链，**然后**才能写问题记录
- 🔴 问题记录的"追溯层 1/2/3/4"四字段**必须**逐层填写，**禁止**跳过任何一层
- 🔴 命中追溯层 1/2/3 → 根因在设计/约束层，**先**改对应文档（Task/Story/DR），**后**改代码
- 🔴 命中追溯层 4 → 写问题记录后**直接**改代码，**不**改任何文档

---

### 命中各追溯层的处理动作

#### 追溯层 1 命中：Task 文档缺陷

```
1. 写问题记录（追溯层 1 勾选"Task 设计有错"）
2. 修 Task 文档（核心代码/方法签名/字段类型/依赖/包路径）
3. 重新生成该 Task 的 CodePlan（走 coding-skill §④bis 5 步 SOP）
4. 按更新后的 CodePlan 重新 Coding
5. 更新问题记录状态为 Fixed
6. 继续 Coding
```

#### 追溯层 2 命中：Story 文档缺陷

```
1. 写问题记录（追溯层 2 勾选"Story 设计有错"）
2. 写 Story Supplement（追加缺陷描述 + 修复建议）
3. 触发 Story Update SKILL → 更新 Story 主文档
4. 分析该 Story 受影响的 Task 范围
5. 触发 Task Generate SKILL → 重新生成受影响的 Task
6. 重新生成受影响 Task 的 CodePlan
7. 按更新后的 CodePlan 重新 Coding
8. 更新问题记录状态为 Fixed
```

#### 追溯层 3 命中：DR 文档缺陷

```
1. 写问题记录（追溯层 3 勾选"DR 设计有错"）
2. 写 DR 补充说明
3. 触发 DR Update SKILL → 更新 DR 主文档
4. 评估影响范围：DR 变更影响哪些 Story？（按 Story 的"领域"或"上游引用"反查）
5. 通知受影响的所有 Story（每个 Story 走追溯层 2 流程）
6. 重新 Story Review → 重新 Task Generate → 重新 CodePlan → 重新 Coding
7. 更新问题记录状态为 Fixed
```

#### 追溯层 4 命中：纯实现问题（AI 犯蠢）

```
1. 写问题记录（追溯层 4 勾选"纯实现问题（前 3 层均已排除）"）
2. 直接修代码（typo/漏 import/笔误/API 误用）
3. 重新跑编译/测试验证
4. 更新问题记录状态为 Fixed
5. 继续 Coding
```

#### 异常：追溯层 4 判定后仍无法归类

```
1. 写问题记录（追溯层 4 勾选"❌ 异常场景（升级用户）"）
2. 暂停 Coding
3. 向用户呈现问题描述 + 追溯过程 + 建议方案
4. 等待用户决策
5. 根据用户决策继续 Coding
```

---

### 与 9 步流程的衔接

| 触发时机 | 进入追溯链 |
|---------|-----------|
| 第七步（编译+服务启动+接口验证+DB 验证）任意失败 | 立即进入追溯链 |
| 第八步（运行测试）单测失败 / 集成测试失败 | 立即进入追溯链 |
| 第八步 bis（测试报告合规性校验）发现测试无效 | 立即进入追溯链 |
| 第六步（按 Task 生成代码）实现过程中发现 Story/Task 文档与实际不符 | **主动**进入追溯链（不等编译/测试失败） |

### A5. 触发 Story 更新 SKILL

当 Story 文档存在需要修复的缺陷时，触发 [Story Update SKILL](../phase1-design/story-update-skill.md)。

**触发条件：**
- Story 接口契约需要修改
- Story 数据模型需要修改
- Story 异常流程需要补充
- Story AC 需要修改

**触发链路：**
```
Coding SKILL（发现 Story 缺陷）
    → 更新 Story 补充说明
    → 触发 Story Update SKILL
        → 更新 Story 主文档
        → 分析是否 DR 缺陷
            → 是 → 更新 DR 补充说明 → 触发 DR Update SKILL
                    → 更新 DR 主文档
                    → 评估影响范围
                    → 通知受影响 Story
```

### A6. 回到正常路径

问题处理完成后，回到触发异常的步骤继续执行。

---

## 经验检查清单

以下是历史踩坑总结，每次生成代码前必须逐项确认：

| # | 检查项 | 说明 |
|---|--------|------|
| 1 | pom 依赖是否被注释 | 新工程模板中 SPI 依赖常被注释，需取消注释 |
| 2 | lombok 是否显式声明 | scope=provided 不传递，每个模块需单独声明 |
| 3 | 第三方 SDK 实际包路径 | 从 jar 中确认，不要凭记忆猜测（如融云是 io.rong 不是 cn.rongcloud.im） |
| 4 | @NotBlank 来源包 | Spring Boot 1.5.x + hibernate-validator 5.x 用 `org.hibernate.validator.constraints.NotBlank` |
| 5 | Result.code 类型 | 确认是 Integer 还是 String，错误码枚举类型要匹配 |
| 6 | 字段类型与 Task 一致 | 特别注意 ID 字段是 Long 还是 String（varchar） |
| 7 | ApiResult 完整 import | 不同工程的 ApiResult 包路径不同（life vs boss） |
| 8 | 新模块注册到父 pom | 创建子模块后必须在父 pom 的 modules 中添加 |
| 9 | BFF Controller 实现 Rest 接口 | 不要自己加 @Api/@GetMapping，从 Rest 接口继承 |
| 10 | Feign 注解版本 | Spring Cloud Dalston 用 `org.springframework.cloud.netflix.feign.FeignClient` |
| 11 | CurrentUserUtil 返回 String | 不要做 Long.valueOf() 转换（除非确认下游需要 Long） |
| 12 | VO 和 DTO 分离 | bff-api 定义 VO，SPI 定义 DTO，Controller 中做转换 |
| 13 | Task 0 必读 | 公共包路径、DO 定义、接口定义在 Task 0 中 |
| 14 | 审计字段自动填充 | 需要 MetaObjectHandler 配置，否则 FieldFill 不生效 |
| 15 | 事务外执行 | 使用 TransactionSynchronizationManager.afterCommit() |

> **🔴 工程特定经验检查清单**（针对具体项目）见各项目资产 `project-assets/{project-key}/{project-key}.assets.md` §6.10。
> Skill 只列通用经验（任何 Java/Spring 项目适用）；**项目特定**的工程经验（如分层职责硬约束、跨层误引用扫描、特定包路径违规）按"约束也是工程的一部分"原则下沉到项目资产，不在本 Skill 圈定。

---

## 禁止事项

| 禁止 | 应该 |
|------|------|
| 凭记忆猜测第三方 SDK 包路径 | 从本地 jar 中解压确认 |
| 跳过编译验证直接报告完成 | mvn compile + 服务启动成功 + 接口测试通过才算完成 |
| 一次性生成所有代码再验证 | 按 Task 顺序生成，关键节点中间验证 |
| 使用 javax.validation.constraints.NotBlank | 使用 org.hibernate.validator.constraints.NotBlank（5.x） |
| 自行命名包路径 | 使用 Task 0 和 Task 文档中定义的固定包路径 |
| 忽略 Task 文档中的核心代码 | 核心代码是模板，直接使用 |
| 跳过 Task 0 直接开始实现 | Task 0 是公共依赖说明，必须先读 |
| 修改 Task 文档中的方法签名 | 方法签名已确定，不可自行修改 |
| **跳过 Task 实现方案确认** | **每个 Task 开始前必须向用户呈现实现方案并获确认。用户必须明确说"确认"、"同意"、"可以开始"，模糊回复（如"好"、"行"、"看看"）需追问确认，未获明确确认前禁止写代码** |
| **发现问题直接修复，不记录** | **必须先写入开发问题记录（A1），再分析根因（A2），再修复** |
| **修复后补写问题记录** | **问题记录必须在修复之前完成，不可事后补写** |
| **Task/Story 文档有缺陷时直接改代码** | **必须先更新 Task/Story 文档，再按更新后的文档修复代码** |
| **用户反馈 IDE 报红时凭 mvn compile 通过否定** | **必须先读 pom.xml 验证依赖链，确认依赖传递是否完整，再下结论** |
| **启动失败时 kill 进程或绕过** | **启动失败必须读日志定位根因，修复后重新验证，属于 🔴 阻断型** |
| **只编译子模块就认为编译通过** | **必须在父工程根目录执行 mvn compile，全工程通过才算通过** |
| **编码完成直接出 CodeReview，跳过全切面核查** | **每轮编码后必须先过"编码后全切面一致性核查闸"（第九步），以代码为锚反向核查五方一致、无 🔴 漂移，才能出 CodeReview** |
| **核查/Review 用裸 ✅ 自我声明通过** | **关键检查项的 ✅ 必须附客观证据（类型核对/文件:行号/真实 DB 输出），禁止无证据打勾** |
| **核心落库路径用 Mock 测试充当落库验证** | **INSERT/UPDATE/DELETE 核心路径必须用真实 DB（H2/TestContainers）验证落库，全 Mock 视为未验证** |
| **在 Repository 里写业务/领域逻辑** | **Repository 只做数据存取（findByXxx/save/update）；状态流转、业务规则校验、跨聚合编排属于 Domain/Application** |
| **在 Application 里写领域规则** | **状态能否流转、金额怎么算等业务规则下沉到 Domain；Application 只做编排（调谁、顺序、事务边界）** |
| **在 Domain 里写编排或持久化** | **Domain 只写领域逻辑，不串外部服务流程、不出现 SQL/PO/DTO** |

---

## 执行清单（逐项执行，禁止跳过）

> **强制要求：AI 启动本 SKILL 时，必须用 TodoWrite 1:1 映射此表。每完成一行，必须验证"产出物已生成 + 门禁已满足"后才进入下一步。未满足门禁不得继续。**

| # | 动作 | 产出物 | 门禁 |
|---|------|--------|------|
| 1 | 收集输入（Story 路径 + Task 目录 + 测试用例路径 + 工作目录） | — | 四项信息已确认 |
| 2 | 阅读约束文档（全部） | — | 8 个约束文件已读取 |
| 3 | 阅读 Story + 测试用例 + Task 0 + 各 Task | — | 全部文档已读取（含测试用例场景清单） |
| 4 | 工程预检（依赖/包路径/已有代码） | — | 工程结构已确认 |
| 5.0 | **Task 实现方案确认（每个 Task 前）** | 用户确认记录 | 用户已确认本 Task 实现方案 |
| 5.1 | 按 Task 顺序生成代码 | 源代码文件 | 每个 Task 检查项已通过 |
| 6 | **编译 + 服务启动 + 接口验证** | — | mvn compile 通过 + 服务启动成功 + 接口测试 Pass |
| 7 | Code Review 自审（对照约束） | `{story}-开发问题记录.md`（如有问题） | 无阻断型/严重型问题 |
| 8 | 运行测试 | — | `mvn test` 全部 Pass |
| 9 | 测试报告合规性校验 | 原始日志 + XML 对账 + `test-authenticity-scan` 报告 | 证据链齐全；扫描 BLOCKER=0；XML 与报告一致；无跳测/忽略失败参数；无假修复风险 |
| 10 | 出具测试报告 | `{story}-Report.md` | 文件已生成 |
| 10a | **编码后全切面一致性核查闸（🔴 强制）** | 《全切面一致性核查表》 | 以代码为锚反向核查全章节五方一致；无 🔴 漂移；核心落库路径有真实 DB 证据；未达标禁止进入 CodeReview |
| 11 | 出具 Coding 报告 | `{story}-Coding-Report.md` | 文件已生成 |

**异常路径（任意步骤可触发）：**

| 触发条件 | 动作 | 产出物 |
|---------|------|--------|
| 发现问题（编译错误/设计矛盾/约束遗漏/启动失败） | 立即记录问题 | `{story}-开发问题记录.md` 更新 |
| Story 缺陷 | 更新 Supplement → 触发 Story Update | Supplement 更新 |


---

## 📖 人工审核主动讲解规范 — Code 节点

> **来源：** 原 ae-sdd-skill.md L18-225 三个审核节点讲解模板之一，本节定义 Code Review 阶段（🔍 人工审核点 4）的主动讲解规范。

**AI 必须主动讲解的内容：**

| 维度 | 必须讲清楚 |
|------|-----------|
| 代码实现故事 | 实际代码是怎么把 Story 落到位的？用 walkthrough 带用户走一遍主流程调用链 |
| 分层 walkthrough | Domain/Application/Infrastructure/Interfaces 各层核心类做什么、关键方法签名是什么、关键代码在第几行 |
| 状态机实现 | canTransition() 实际怎么写？状态流转代码长什么样？条件校验在哪一行？ |
| 事务实现 | `@Transactional` 边界在哪个方法？事务传播行为是什么？回滚规则是什么？ |
| 异常处理 | 核心异常在代码里怎么抛？怎么捕获？错误码怎么映射？HTTP 状态码是什么？ |
| 测试覆盖 | 哪些 AC 已被测试覆盖？测试方法名是什么？覆盖率如何？ |
| CodeReview 发现 | 阻塞型/严重型问题有哪些？整改方案是什么？ |

**输出模板：**

```
📖 【Code 讲解 - {STORY-ID} v{N}-r{M}】

【代码实现故事】
本轮 Coding 实现了 {Task-1, Task-2, ...} 共 {N} 个 Task。
让我们按调用链路走一遍：用户{操作} → {Controller} → {AppService} → {DomainService} → {Repository} → DB

【Domain 层 walkthrough】
- {XxxAggregate}：{职责}，核心方法 {method1, method2, ...}
- 关键代码：{文件}:{行号} → {一段代码片段或逻辑描述}
- 状态流转：{canTransition 实际逻辑}（在 {文件}:{行号}）

【Application 层 walkthrough】
- {XxxAppService}：{职责}，编排逻辑：{调用链}

【Infrastructure 层 walkthrough】
- {XxxRepositoryImpl}：{落库方式}，关键 SQL：{SQL 描述}

【事务边界】
- {XxxAppService.transition()}：@Transactional 边界 = {包含哪些操作}
- 事务外：{哪些操作在事务外}（如：通知发送）
- 回滚规则：{触发条件}

【异常处理】
- {XxxException}：{抛出条件} → 错误码 {code} → HTTP {status}
- {XxxDomainException}：{抛出条件} → 错误码 {code}

【测试覆盖】
- AC-001：测试方法 {TestClass.testXxx} ✓
- AC-002：测试方法 {TestClass.testXxx} ✓
- 异常路径：测试方法 {TestClass.testXxx} ✓
- 覆盖率：{数字}%

【CodeReview 关键发现】
- 🔴 阻断：{数量} 个 → 整改方案：{方案}
- 🟠 严重：{数量} 个 → 整改方案：{方案}
- 🟡 一般：{数量} 个 → 建议优化
- 🟢 建议：{数量} 个

【讲解结束 - 请审阅】
1. 代码实现是否正确？
2. 事务边界是否合理？
3. 异常处理是否完整？
4. 测试覆盖是否充分？
5. CodeReview 问题是否接受？
```

**Code 节点专用反模式：**
- ❌ 只说"测试通过"不讲解实现细节（用户没法判断对错）
- ❌ 分层 walkthrough 跳过具体类:方法（用户要去翻代码才能对账）
- ❌ 关键代码不给文件:行号（违反"证据化检查"）
- ❌ CodeReview 发现只列问题不给整改方案（违反"问题必附修复建议"）

**门禁：**
- 🔴 未输出 `📖 【Code 讲解 - ...】` → 视为跳过人工审核点 4 → 禁止进入 ⑩ 完成判定
- 🔴 关键代码未附文件:行号 → 视为讲解无证据，按 [[evidence-bound-check-no-bare-checkmark]] 整改
- 🔴 CodeReview 问题未给整改方案 → 视为报告不完整，按 [[feedback_report-code-reconciliation]] 整改


---

## 📋 ④bis CodingPlan 输出（🔴 强制 — Coding 前的实现方案文档）

> **来源：** 原 ae-sdd-skill.md ④bis 节点章节。⑤ Coding 之前必须输出 `{STORY-ID}-CodingPlan.md`，含 7 章节、10 条门禁全部通过，未通过禁止进入 Coding。

### ④bis CodingPlan 输出（🔴 强制）

> **为什么必须有 CodingPlan：Task 文档是"做什么"，Task 实现方案是"汇总层概要"，但中间缺一个"具体怎么写代码"的执行层 plan。** 没有 CodingPlan，AI 在 ⑤ Coding 时会边写边拍脑袋，出现：
> - 文件顺序错乱（先写 Controller 再写 Domain，编译一直失败）
> - 关键方法签名反复修改（改一次影响下游 N 个文件）
> - 测试数据临时凑数（用 default value 凑通过）
> - 中途"修测试"代替"修代码"（详见下文 `🔴 测试真实性强制规范`）
>
> CodingPlan 的核心作用是 **"让 AI 在动键盘前把代码骨架定死，编码过程变成填空"**。

**触发时机：** ④ Task Generate 通过人工审核后，⑤ Coding 之前

**输入：**
- Task 文档（含 Task 0 环境准备 + 所有实现 Task）
- {STORY-ID}-Task实现方案.md（汇总层）
- Story 文档
- 测试用例文档
- 约束规范（分层职责红线 / database.md / api.md / security.md 等）
- 工作目录（确认工程结构）

**输出：**
- `{STORY-ID}-CodingPlan.md`（与 Task 文档同目录）
- 该文件是 ⑤ Coding 的**直接施工蓝图**，AI 编码时必须按此执行，不得临时偏离

**🔴 CodingPlan 输出前置：风险预判（必须先于7章节执行）**

> **为什么要前置：** CodingPlan 的7章节是"怎么写"，但在动笔之前必须先想清楚"这个 Story 有哪些风险，方案是否已覆盖"。漏掉风险预判，章节4的SQL写法可能缺乐观锁，章节2的类骨架可能缺幂等设计，写完了再改代价是写之前的10倍。

按 `../../standards/thinking/be-coding-thinking-engine.md §1.4 风险预判·11维度` 对本 Story 逐维过一遍，每个维度给出三列答案：

| 维度 | 本 Story 是否涉及 | 方案 / 有意不做的理由 |
|---|---|---|
| ① 原子性 | 是 / 否 | |
| ② 并发安全 | 是 / 否 | |
| ③ 幂等性 | 是 / 否 | |
| ④ 可解耦性 | 是 / 否 | |
| ⑤ 数据一致性 | 是 / 否 | |
| ⑥ 外部依赖容错 | 是 / 否 | |
| ⑦ 性能瓶颈 | 是 / 否 | |
| ⑧ 资源隔离 | 是 / 否 | |
| ⑨ 安全 | 是 / 否 | |
| ⑩ 可观测性 | 是 / 否 | |
| ⑪ 可运维性 | 是 / 否 | |

> 🔴 **门禁：** 风险预判表未完成 → 禁止进入7章节。每个「是」必须在对应章节中体现（如：①原子性=是 → 章节4 SQL 必须写事务边界；②并发=是 → 章节4 必须写乐观锁/分布式锁 WHERE 条件）。
> 每个「否」必须写明理由，不允许空列——空列等于「没想过」，不等于「不需要」。

---

**🔴 CodingPlan 必须包含的 7 个章节：**

#### 章节 1：文件级实现顺序

> **目标：** 让 AI 知道"先写哪个文件、再写哪个"，每一步写完都能 `mvn compile` 通过。

| # | 文件路径 | 类型 | 依赖（前置必须先写） | 完成后必须通过的验证 |
|---|---------|------|------------------|-------------------|
| 1 | `domain/entity/XxxAggregate.java` | 新增 | — | `mvn -pl domain compile` |
| 2 | `domain/repository/XxxRepository.java` | 新增 | #1 | `mvn -pl domain compile` |
| 3 | `infrastructure/persistence/XxxMapper.java` | 新增 | #1 | `mvn -pl infrastructure compile` |
| 4 | `infrastructure/persistence/XxxRepositoryImpl.java` | 新增 | #2, #3 | `mvn -pl infrastructure compile` |
| 5 | `application/XxxAppService.java` | 新增 | #1, #2 | `mvn -pl application compile` |
| 6 | `interfaces/XxxController.java` | 新增 | #5 | `mvn -pl interfaces compile` |
| 7 | `infrastructure/persistence/XxxRepositoryIT.java` | 新增 | #4 | `mvn verify -Dit.test=XxxRepositoryIT` |

> 🔴 顺序原则：**Domain → Infrastructure → Application → Interfaces → Test**（依赖倒置，编译永远能过）
> 🔴 每个文件完成后必须有"验证列"——不允许写完一堆文件再统一编译（中途报错难以定位）

#### 章节 2：关键类骨架

> **目标：** AI 编码时按骨架"填肉"，不需要现场设计类结构。

每个核心类必须包含：
- **类签名**（注解 + 类名 + 继承/实现）
- **核心字段**（含类型 + 一句话说明）
- **核心方法签名**（方法名 + 入参 + 返回值 + 关键注解如 `@Transactional`）
- **关键方法体的伪代码**（10-30 行，描述核心逻辑，不写完整实现）

**示例：**

```java
// Domain 层聚合根
public class Ticket {
    private Long id;
    private String conversationId;
    private TicketStatus status;
    private Long version;
    private LocalDateTime createTime;
    private LocalDateTime updateTime;
    
    // 核心方法签名
    public void transition(TicketStatus target, Long operatorId, String reason) {
        // 伪代码：
        // 1. 校验 canTransition(current, target)
        // 2. 校验 operatorId 不为空
        // 3. 变更 status
        // 4. 记录 history（不在本方法内，由 AppService 编排）
    }
    
    private boolean canTransition(TicketStatus from, TicketStatus to) {
        // 状态机校验逻辑
    }
}
```

#### 章节 3：数据结构 / DO 字段

> **目标：** 数据库表结构、DO/Entity/DTO 字段全部定死，编码时直接抄。

| 表名 | 字段名 | 类型 | 约束 | 对应 DO 字段 | 对应 DTO 字段 | 备注 |
|------|--------|------|------|-------------|-------------|------|
| im_ticket | id | BIGINT | PK | id | ticketId | 雪花 ID |
| im_ticket | status | VARCHAR(32) | NOT NULL | status | status | 枚举字符串 |
| im_ticket | version | BIGINT | NOT NULL DEFAULT 0 | version | — | 乐观锁 |
| im_ticket | create_time | DATETIME | NOT NULL | createTime | createTime | 自动填充 |

#### 章节 4：Mapper / Repository 关键 SQL

> **目标：** 关键 SQL 提前写好（带 WHERE 条件、乐观锁、字段映射），编码时直接用。

| # | 操作 | Mapper 方法 | 关键 SQL/条件 | 乐观锁 | 备注 |
|---|------|------------|-------------|--------|------|
| 1 | INSERT | `TicketMapper.insert(po)` | 标准 insert，无特殊条件 | 否 | 业务端生成 ID |
| 2 | UPDATE | `TicketMapper.updateStatus(po)` | `WHERE id=#{id} AND version=#{version} AND status=#{expectedStatus}` | 是 | 状态前置 + 乐观锁 |
| 3 | SELECT | `TicketMapper.findById(id)` | `WHERE id=#{id}` | — | 走主键 |

> 🔴 关键 UPDATE 的 WHERE 条件必须明确写出（状态前置、乐观锁），不允许"看情况"。

#### 章节 5：测试用例的对应实现

> **目标：** 每个 AC 对应一个测试方法签名 + 真实测试数据来源，编码时直接实现。**禁止临时凑数据。**

| AC ID | 测试类 | 测试方法 | 测试数据来源 | Mock 范围 | 真实 DB | 真实 HTTP |
|-------|--------|---------|------------|---------|---------|----------|
| AC-001 | `TicketAppServiceIT` | `transition_success` | `Story §AC-001 示例值` | Mock: 无 | ✅ 走 H2 | ✅ 走 SpringBootTest |
| AC-002 | `TicketAppServiceIT` | `transition_userNotFound` | `Story §AC-002 示例值` | Mock: 无 | ✅ 走 H2 | ✅ 走 SpringBootTest |
| AC-003 | `TicketAppServiceIT` | `transition_concurrent_versionConflict` | `Story §AC-003 示例值` | Mock: 无 | ✅ 走 H2 | ✅ 走 SpringBootTest |
| AC-004 🔴 | `TicketStatusBadge.test.ts` | `renders_inactive_state` | `Story §前端契约 状态展示建议` | — | — | — |
| AC-005 🔴 | `FreezeForm.test.ts` | `preventDoubleSubmit` | `Story §前端契约 边界处理` | — | — | — |

> 🔴 **测试数据来源必须从 Story/Task 中可追溯**，禁止"假设用户 ID=1L 就能跑"。
> 🔴 **核心落库路径必须用真实 DB（H2/TestContainers）**，禁止全 Mock。
> 🔴 **核心接口必须用真实 HTTP（SpringBootTest RANDOM_PORT）**，禁止 MockMvc 代替。

#### 章节 6：编译与测试验证点

> **目标：** 把验证动作拆成"每步可验证"，不要等写完全部代码再统一跑。

| 阶段 | 触发时机 | 验证命令 | 通过标准 |
|------|---------|---------|---------|
| 单文件完成 | 每写完一个文件 | `mvn -pl {module} compile` | BUILD SUCCESS |
| 单层完成 | Domain/Infrastructure/Application/Interfaces 每层完成 | `mvn -pl {module} -am compile` | BUILD SUCCESS |
| 单 Task 完成 | 一个 Task 对应的所有文件完成 | `mvn -pl {module} -am test -Dtest={XxxTest}` | 全部 Pass |
| 全量完成 | ⑤ Coding 全部完成 | `mvn clean verify` | 全部 Pass + 无 `Tests in error` |
| 真实 HTTP 验证 | 涉及 Controller 的 Task 完成后 | `mvn -pl {interfaces} -am verify -Dtest={XxxIT}` | SpringBootTest 起服务 + 真实 HTTP 请求 200 |

> 🔴 不允许"写完所有代码再统一编译"——任何文件写完必须能独立编译。

#### 章节 7：调试与回滚策略

> **目标：** 编码过程中出问题，知道怎么定位、怎么安全回退。

| 失败类型 | 定位方法 | 回滚策略 |
|---------|---------|---------|
| 编译失败 | IDE 跳转 + 错误栈 | git stash 当前未提交改动 → 回到上一个 commit 状态 |
| 单测失败 | 测试报告 + assertion 失败信息 | **先看是不是测试期望值错了**（详见 `🔴 测试真实性强制规范`） |
| 集成测试失败 | H2/TestContainers 日志 + SQL 输出 | 检查 DB 初始化脚本 + 数据准备 |
| 真实 HTTP 失败 | 服务启动日志 + curl 输出 | 检查 Bean 注入 + Controller 路径 |
| 性能问题 | JProfiler / Arthas | 先定位慢 SQL（EXPLAIN）→ 再定位慢方法 |

---

**🔴 CodingPlan 门禁（未通过禁止进入 ⑤ Coding）：**

- [ ] 7 个章节全部填写
- [ ] 文件级实现顺序满足"每步可独立编译"
- [ ] 关键类骨架覆盖所有 Domain/Application/Infrastructure/Interfaces 层核心类
- [ ] 数据结构 / DO 字段与 Story 数据模型章节完全一致
- [ ] Mapper 关键 SQL 的 WHERE 条件、乐观锁、状态前置明确写出
- [ ] 测试用例对应实现的测试数据**可追溯到 Story/Task**（禁止"假设 userId=1L"）
- [ ] 核心落库路径标记"真实 DB"
- [ ] 核心接口标记"真实 HTTP"
- [ ] 编译与测试验证点覆盖到每个文件/每层/每个 Task
- [ ] 调试与回滚策略完整（至少 5 种失败类型）
- [ ] 🆕 v3.4.0 **G-CODEPLAN-SRC 源码核对**：关键类骨架每个类附【已读源码：】（文件存在）或【待核实源码】标记，待核实清单为空
- [ ] 🆕 v3.4.0 **G-14 CodingPlan-Story 一致性**：Plan 引用 Story 文档 + 测试章节 AC ID 对齐 + 偏离项有 Proposal

> **📍 完整 15 条门禁自检表（含判定 SOP）在 [`be-coding-plan-template.md` §15](../../templates/coding/be-coding-plan-template.md)。** 本处列出 12 条概览，门禁 11-14（CodingModel/核心链路/资源隔离/混合压测）见模板 §15。

### 📋 G-CODEPLAN-SRC 源码核对详细判定标准（🆕 v3.4.0 — 对标建议书1）

> 本节是 `SKILL.md §🛡️ G-CODEPLAN-SRC` 的下沉详细规则。CLI：`ae-sdd gates check --only G-CODEPLAN-SRC`。

**为什么需要**（实战复盘，life 项目 STORY-020）：

| # | Plan 中的错误 | 源码实际事实 | 危害 |
|---|---|---|---|
| 1 | 把 application 层 `ImMessageConverter` 当成要改的 | 实际要改的是 infrastructure 层 `LatestSideMessagePOConverter` | 改错文件 |
| 2 | 说"新增 Converter 映射" | `ImMessageConverter.toDTO` 已存在 | 重复造轮子（红线 #10）|
| 3 | 设计嵌套 Anchor 值对象 | 现有 PO/DO 全是扁平字段 | 与现有建模范式不符 |
| 4 | 测试范式标"JUnit4/5 待确认" | 代码里就是 JUnit4 + SpringRunner + H2 | 本可读源码确认却标待确认 |
| 5 | Converter 写法按 AGENTS.md 写 `@UtilityClass` | 实际代码用 `@NoArgsConstructor(PRIVATE)`+static | AGENTS.md 与实际有出入，应以代码为准 |

**判定标准——"现有同类源码"范围：**

| 类别 | 同类范围 |
|------|---------|
| DO/实体 | 同包同类（如 `domain/.../entity/` 下已有 DO 的字段/注解写法）|
| Converter | 同类型 Converter（application 层 DTO Converter / infrastructure 层 PO Converter 各自对照）|
| PO | 同包 PO 的扁平/嵌套范式、`@TableName`/`@TableId` 用法 |
| Repository Impl | 同层 Repository 的 Mapper 注入方式、事务边界 |
| 测试 | 同模块测试的框架（JUnit4/5）、Runner、H2/真实 DB 范式 |

**标记格式：**

```
【已读源码：domain/message/model/entity/ImMessageDO.java】   ← 已核对，文件存在
【待核实源码：Converter 写法】                                ← 未核对，须补读
【待核实源码】                                                ← 未核对（简写）
```

**待核实清单格式**（CodingPlan 文档内独立小节）：

```markdown
### 待核实源码清单（G-CODEPLAN-SRC）
- [ ] ImMessageConverter 现有 toDTO 方法签名（domain/.../ImMessageConverter.java）
- [ ] LatestSideMessagePO 扁平/嵌套范式（infrastructure/.../LatestSideMessagePO.java）
- [ ] 测试框架版本（src/test/java/.../现有测试类）
```

**门禁规则：**
- 类骨架章节**无任何标记** → 🔴 阻断（每个新增/修改类须附标记）
- 标【已读源码：】但**文件不存在** → 🔴 阻断（防伪造标记）
- **待核实清单非空** → 🔴 阻断（CodingPlan 视为草案，须补读后改为【已读源码：】才进 ⑤ Coding）
- CodingPlan **无关键类骨架章节**（微任务场景）→ 跳过，不阻断

**禁止（违者视为 CodingPlan 不完整）：**
- ❌ 7 个章节中任一缺失
- ❌ 文件顺序不满足"每步可独立编译"（如先写 Controller 再写 Service）
- ❌ 关键类只有方法名没有方法体伪代码
- ❌ SQL 写法模糊（"按主键更新" → 必须明确 WHERE 条件）
- ❌ 测试数据来源标注"假设" / "随便填" / "TODO"
- ❌ 🆕 v3.4.0 类骨架无来源标记 / 标已读但文件不存在 / 待核实清单未闭环

**与已有步骤的衔接：**
- ④ Task Generate 的"全局 Task Review"负责 Review Task 文档的"做什么"层面
- 本 ④bis CodingPlan 负责"具体怎么写代码"层面，是 ④ 的执行层细化
- 🔍 人工审核点 2 必须把 CodingPlan 纳入审核范围，逐文件（按章节）核对
- ⑤ Coding 阶段 AI 必须严格按 CodingPlan 执行，临时偏离需用户确认

---

## 📚 ④bis 实战 SOP：分层拆分 + 项目资产映射 5 步流程（🔴 必读）

> **本章配套资产（🆕 2026-06-10 路由改造：不再直接引用路径）：**
> - 项目资产 schema：`document-storage-skill.get_assets_schema()`
> - 项目资产 starter 模板：`document-storage-skill.get_assets_template()`
> - **当前项目资产：`ae-sdd assets read coding --project <projectKey>` — 返回 §4 DDD 内部分层落点 + §5 命名约定 + §6 工程约束**（不再硬编码 `icec-cloud-boss`）
> - **精准查询（按需）：`ae-sdd assets query "<name>"`（module/component/table 通用）**
> - Code Plan 模板：`../../templates/coding/be-coding-plan-template.md`（SKILL 内模板，不随工程变化）

### 核心设计哲学

1. **抽象分层规则不变**（4 类 + 2 可选）：请求处理 / 业务编排 / 领域逻辑 / 基础能力 + 跨模块 SPI / BFF 入口
2. **项目资产是跨项目复用的关键**：每项目一份项目资产；无资产 → 走项目资产 §9 探查 SOP 构建 → 再做映射
3. **Code Plan 不重写实现**：实现细节在 Task 里，Code Plan 只做 **Task 编排 + 类骨架 + 方法级逻辑 + 目录对应**

### 5 步 SOP（🔴 严格按顺序执行）

#### 步骤 1：读取项目资产（🔴 详细动作已迁入 project-assets-update-skill.md §6）

**输入：** `{STORY-ID}` + `{project-key}`

**动作：**
> **📍 直接调用场景化 API，不再跳转 project-assets-update-skill §6 动作 4。**

1. 调用 `ae-sdd assets read coding --project <projectKey>`
   返回：§4 DDD 内部分层落点 + §5 命名约定 + §6 工程约束
   → 精准查询（需要时）：`ae-sdd assets query "<name>"`（module/component/table 通用）
2. 资产不存在 → 停止，先运行 `project-assets-update-skill §3 生成动作`（**禁止**继续 ④bis）
3. 资产过期（`lastAuditedAt > 90 天`）→ 停止，先运行 `project-assets-update-skill §5 审计`（**禁止**继续 ④bis）
4. 资产 30-90 天 → 建议先跑 `project-assets-update-skill §4 更新`（推荐）
5. 完成后在 Plan 头部 §1 写"项目资产已就绪"声明（路径/lastAuditedAt/引用章节）

**输出：** 已在 Plan §1 项目资产引用块填写完整

**门禁：**
- 🔴 缺项目资产 = 整 Plan 打回，禁止跳过
- 🔴 `lastAuditedAt > 90 天` = 资产过期，禁止直接使用
- 🟠 `lastAuditedAt > 30 天` = 建议先跑增量更新

#### 步骤 2：对 Task 做执行顺序编排（不重写实现）

**输入：** `{STORY-ID}-Task实现方案.md`

**动作：**
1. 抽出 Task 列表（Task-0 公共依赖 + Task-1..N）
2. 按依赖关系画执行顺序图：Domain → Infrastructure → Application → Interfaces → BFF → Test
3. 输出 Code Plan §3 Task 表格
4. 进一步拆分到文件级（§4 文件级顺序）

**原则：**
- 每个 Task 输出"输入/输出/前置依赖/估时"4 项
- **不写实现细节**（具体方法体在 Task 文档里）
- **不重写 Task 文档**

**输出：** Code Plan §3 + §4 完整

**门禁：** 🔴 重写 Task 文档（粘贴 Task 已写的方法体）= 打回

#### 步骤 3：按抽象 4 层对每个 Task 做分层归类

**输入：** Task 列表 + 每个 Task 涉及的所有类

**动作：**
1. 把每个 Task 涉及的所有类按抽象 4 层打标
2. 填写 Code Plan §2 抽象分层 → 项目分层映射表

**判定口诀：**
- 业务规则（状态机/不变量/聚合一致性）→ **Domain**
- 协调谁调谁（事务/顺序/跨域）→ **Application**
- 存取数据（findByXxx/save/update）→ **Repository / Infrastructure**
- 接 HTTP / 协议适配 → **Interfaces**
- 跨服务契约 → **SPI**
- BFF 场景 → **BFF 入口**

**边缘案例判定（🔴 必读）：**
- 状态机（业务规则核心）→ **Domain**（写在 `domain/.../service/{Resource}DomainService` 或 `entity/{Resource}DO.transition()`）
- 跨聚合事务（协调多聚合）→ **Application**（写在 `appservice/{Resource}AppService` 的 `@Transactional` 方法）
- 缓存读（带业务策略如"先查缓存再查 DB"）→ **Application**（业务编排的一部分）
- 全局唯一性校验（需查 DB）→ **Domain**（写在 DomainService，因这是聚合不变量）

**输出：** Code Plan §2 表格完整

**门禁：** 🔴 分层写错（业务规则塞到 Repository/状态机计算塞到 Controller/编排逻辑塞到 Domain）= 整 Plan 打回

#### 步骤 4：把每个类按项目分层映射到确切包路径

**输入：** §2 分层归类结果 + 项目资产 §4 DDD 内部分层落点

**动作：**
1. 调用 `ae-sdd assets section §4 --project <projectKey>` 或直接使用步骤 1 返回的 §4 内容，匹配每个类对应的精确包路径
2. 例：Application 层的 `BossUserAppService` → `icec-cloud-boss-user/icec-cloud-boss-user-application/src/main/java/com/casstime/cloud/boss/user/application/appservice/BossUserAppService.java`
3. 填入 Code Plan §5 类骨架的"包路径"列

**原则：**
- **禁止**写"包路径待定/TBD/按项目惯例"
- **禁止**复述项目资产内容（避免双源不一致）
- 包路径必须能定位到项目资产 §4 的具体包路径模板

**输出：** Code Plan §5 类骨架的"包路径"列完整

**门禁：** 🔴 包路径写"待定"= 整 Plan 打回

#### 步骤 5：输出类骨架（方法签名 + 方法逻辑说明）

**输入：** §4 映射好的类 + 类的方法列表（来自 Task 文档）

**动作：**
1. 对每个类输出一张 §5 类骨架子卡
2. 每张卡含：类签名 + 所在层 + 包路径（步骤 4 已定）+ 核心字段 + 核心方法签名 + 方法伪代码

**方法伪代码分级：**
| 复杂度 | 行数上限 | 触发条件 |
|--------|---------|---------|
| 简单 | ≤ 10 行 | 单一职责 |
| 中等 | ≤ 20 行 | 含分支/循环 |
| 复杂 | ≤ 40 行 | 多分支/异常处理重，**需 @复杂 标注**触发用户确认 |

**伪代码格式：** 每步以**动词开头**（校验/查询/转换/返回/抛异常/组装/调用）。

**✅ 合规示例：**
```
1. 校验入参 cmd 必填字段
2. 调 cmd.toDO() 转换
3. 调 domainService.validateBusinessRules(do)
4. 调 repository.save(do)
5. 调 converter.toDTO(savedDO)
6. 返回 dto
```

**❌ 违规示例：**
```java
// 不要这样写！
public {Resource}DTO create(CreateCommand cmd) {
    if (cmd == null) throw new IllegalArgumentException("cmd is null");
    {Resource}DO do = new {Resource}DO();
    do.setName(cmd.getName());
    // ... 20 行实现
    return converter.toDTO(do);
}
```

**原则：**
- **禁止**贴完整方法体（完整方法体是 ⑤ Coding 的事）
- **禁止**省略项目资产引用（每张类卡必须能追溯到 §4 / §5）
- 复杂方法（>20 行）必须 @复杂标注 + 触发用户确认

**输出：** Code Plan §5 类骨架卡完整

**门禁：** 🔴 贴完整方法体 / 写 > 40 行伪代码 / 省略项目资产引用 = 章节打回

### 7 条禁令（🔴 任何一条违反 = 整 Plan 打回）

| # | 禁令 | 反例 |
|---|------|------|
| 1 | 禁止在 Code Plan 写完整实现代码 | ❌ 贴 30 行方法体 / 完整 SQL / import 块 |
| 2 | 禁止跳过项目资产直接写 Code Plan | ❌ 无项目资产就开始 §3 Task 编排 |
| 3 | 禁止把分层写错 | ❌ 业务规则塞到 Repository / 状态机塞到 Controller / 编排塞到 Domain |
| 4 | 禁止把分层映射表里的"包路径"写成"待定/TBD" | ❌ "包路径按项目惯例" |
| 5 | 禁止重写 Task 文档 | ❌ 复制 Task 已写的方法体到 Code Plan |
| 6 | 禁止省略 7 个强制章节 | ❌ 任何章节"无则填 N/A"必须有说明 |
| 7 | 禁止用 default value 凑测试数据 | ❌ "假设 userId=1L 就能跑"（必须可追溯到 Story/Task 章节） |

### 与 9 步流程的衔接

```
9 步流程：① 需求 → ② 架构 → ③ 任务拆分 → ④ Task 生成
                                              ↓
                              ④bis CodePlan（本 SOP 落点）← 5 步流程
                                              ↓
                              ⑤ Coding（按 CodePlan 施工）→ ⑥ 自审 → ⑦ CR → ⑧ 测试 → ⑨ 交付
```

**衔接点：**
- SOP 步骤 1 消费 **② 架构** 的项目资产（项目资产由 ② 架构阶段产出，或在 ④bis 之前按 schema §9 探查 SOP 构建）
- SOP 步骤 2-4 消费 **④ Task** 文档（Task 已确定"做什么"，Code Plan 只编排"怎么落"）
- SOP 步骤 5 产出物 = Code Plan §5 类骨架 = **⑤ Coding** 的直接施工蓝图
- ⑤ AI 必须严格按此执行，**临时偏离需用户确认**
- SOP 步骤 5 的测试对应表 = **⑦ Code Review + ⑧ 测试** 的对照索引
- SOP 步骤 5 的验证点表 = **⑤ Coding 结束**的 gate
- SOP 步骤 5 的调试回滚表 = ⑤/⑥ 阶段出问题的应急手册

### Code Plan 失败回退机制

> **门禁 N 不通过 → 只修补章节 N，其他章节复用。** 避免整 Plan 推翻重写。

| 失败类型 | 回退动作 |
|---------|---------|
| 门禁 1（缺项目资产） | 走 schema §9 探查 SOP 构建项目资产，回到步骤 1 |
| 门禁 2（文件顺序不可独立编译） | 修补 §4，重跑门禁 2 |
| 门禁 3（类骨架不全） | 补 §5 类骨架，**不动** §3/§4 |
| 门禁 4（DO 字段不一致） | 修补 §6，回到 Story 阶段核对数据模型 |
| 门禁 5（SQL WHERE 不明确） | 修补 §7，回到 Task 阶段核对 Mapper 设计 |
| 门禁 6（测试数据不可追溯） | 修补 §8"测试数据来源"列，必须含 Story 章节号 + 行号 |
| 门禁 7/8（核心场景未标真实 DB/HTTP） | 修补 §8"核心标识"列 |
| 门禁 9（验证点未覆盖每 Task） | 修补 §9 |
| 门禁 10（调试回滚 < 5 类） | 补 §10 失败类型 |

**重要：** 修补后重跑全部 10 条门禁；全 ✅ 后进入 ⑤ Coding。**禁止**"自我声明 ✅"通过——每条门禁必须按 SOP 判定（详见 Code Plan 模板 §15）。

### 与本 SKILL 异常路径的关系

- 本 SOP 与"异常路径 A1-A6"叠加：SOP 失败回退 + 异常路径问题记录
- 如果 SOP 步骤 1 探查项目资产时发现项目结构本身有缺陷 → 走异常路径 A3（先更新文档再修复代码）→ 修复项目资产后回到步骤 1
- 详见本 SKILL "异常路径" 章节

---

## 📋 测试真实性强制规范（8 类禁止手段 + 5 条保障要求）

> **🔴 【2026-06-06 重大重构】本章定义已迁出到 [`code-review-skill.md` §阶段 D 测试真实性 + 真实 DB/HTTP 覆盖核查](../../code-review-skill.md)。coding-skill.md 中保留原文作为"实现阶段自检"参考；评审阶段请走 code-review-skill.md。**
>
> **来源：** 原 ae-sdd-skill.md Phase 3 测试真实性强制规范章节。AE 流程⑥完成判定的硬前置 — `test-verifier` 必须独立扫描 8 类禁止伪造手段，命中任一 = 测试报告作废 + Coding 返工。

### 🔴 测试真实性强制规范（防止 AI 伪造测试通过）

> **核心立场：AI 在测试时有强烈的"自圆其说"倾向——为了让测试标绿，可能采用各种"小聪明"绕过真实验证。** 这些手段表面上让 `mvn test` 全绿，但实际业务逻辑并没有被正确测试。本节是 ⑥ 完成判定的硬前置，列出禁止的伪造手段和真实性保障要求。**任一伪造手段出现 → 测试报告作废，⑤ Coding 必须返工。**

#### 8 类禁止的伪造手段（违者 = 测试无效）

##### 1. 隐藏失败测试

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ 用 `@Disabled` / `@Ignore` 注解跳过失败测试 | 假装"该测试不重要" | 修复代码或测试 |
| ❌ 删除失败的测试方法 | 直接消失，假装没写过 | 修复代码或测试 |
| ❌ 用 `if (false) { ... }` 包裹测试逻辑 | 测试被跳过但代码还在 | 删除死代码或修复 |
| ❌ 用 `assumeTrue(false)` 跳过 | JUnit 假设失败 = 跳过 | 修复代码或测试 |
| ❌ 改测试方法名加后缀 `_disabled` | 隐藏原失败 | 不允许 |

##### 2. 永真断言

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ `assertTrue(true)` / `assertFalse(false)` | 永远通过 | 断言真实业务值 |
| ❌ `assertNotNull(new Object())` | 永远通过 | 断言被测代码输出 |
| ❌ `assertEquals(1, 1)` 恒等断言 | 与代码无关 | 断言 actual vs expected |
| ❌ `assertEquals("string", "string")` 字面量相等 | 永远通过 | 断言被测方法返回 |

##### 3. 吞噬异常

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ `try { ... } catch (Exception e) { /* 忽略 */ }` | 异常被吃掉 | 让异常抛出 |
| ❌ `try { ... } catch (Exception e) { return; }` | 测试提前返回 | 让异常抛出，断言异常类型 |
| ❌ catch 后断言 `noExceptionThrown()` | 颠倒黑白 | 断言异常类型 + 错误码 |

##### 4. 全 Mock 替代

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ 把所有 Repository/Service 都 mock 掉，测的是 mock 本身 | mock 返回啥就是啥 | 核心路径用真实 DB/真实 HTTP |
| ❌ Mock 返回 `any()` / `anyString()` | 所有断言都通过 | 用具体值 mock |
| ❌ Mock 链太深（Mock → Mock → Mock） | 验证的不是真实业务 | 减少 Mock 层级 |
| ❌ 用 Mockito 的 `RETURNS_DEEP_STUBS` 链式 mock | 测的是 mock 不是代码 | 拆解测试 |

##### 5. 调整断言方向（最隐蔽的伪造）

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ 期望值=实际值：先跑代码得 actual，再写 `assertEquals(actual, actual)` | 自证自明 | expected 是 hardcode 的预期值 |
| ❌ 期望值变量名是 expected，但赋值是测试运行时算的 | 名字骗人 | expected 必须是常量/字面量 |
| ❌ 断言放在 try 里，catch 后断言 `assertTrue(true)` | 出错也通过 | 让 assertion 在 try 外 |

> **🔴 这类伪造最危险，因为它"看起来"在测真实逻辑，实际什么都没测。** 判定规则：**expected 必须是 hardcode 的字面量或常量，actual 必须是被测方法的返回值。**

##### 6. 无效测试数据

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ 随机生成数据但只验证"非空" | 数据无意义 | 数据对应真实 AC 场景 |
| ❌ 测试数据用 default value（null/0/空字符串） | 不触发业务逻辑 | 用业务真实场景数据 |
| ❌ 测试场景不覆盖真实业务分支（只测了"新建成功"没测"重复创建失败"） | 覆盖率虚高 | 测正向 + 反向 + 边界 |

> **🔴 测试数据可追溯性：每个测试数据的来源必须在 CodingPlan §5 中明确，编码时直接复用，禁止"假设 userId=1L 就能跑"。**

##### 7. 睡眠绕过

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ `Thread.sleep(N)` 等待异步完成 | 不稳定，时快时慢 | 用 CountDownLatch / Awaitility 真实等待 |
| ❌ `Awaitility` 用超长 timeout（如 30s）掩盖死锁 | 测试一直挂 | 缩短 timeout，定位真正问题 |
| ❌ 用 `TimeUnit.SECONDS.sleep` 配固定数值 | 容易 flaky | 用确定性等待机制 |

##### 8. 篡改覆盖率

| 伪造手段 | 真实表现 | 正确做法 |
|---------|---------|---------|
| ❌ 写大量重复测试方法只为凑覆盖率 | 数字虚高 | 测试方法覆盖真实业务场景 |
| ❌ 测 get/set 一类纯 getter 凑数 | 不是业务测试 | 测业务方法 |
| ❌ 测试方法和被测方法一对一硬编码（脆弱） | 改实现就挂 | 测业务行为 |

---

#### AI 测试真实性保障要求（强制）

##### 保障 1：测试代码可见性

> AI 不得只说"测试通过"就完事，必须**输出关键测试代码**让用户能 review。

| 必须呈现 | 示例 |
|---------|------|
| 测试方法签名 | `void transition_success() throws Exception` |
| 测试数据准备 | 完整显示 fixture 构造 |
| Mock 设置 | 显示 mock 哪些 + 返回什么 |
| 断言代码 | 完整显示 assertEquals(expected, actual) |
| 实际运行结果 | 显示测试报告中的 Pass 行 + 关键日志 |

##### 保障 2：测试数据可追溯

> 每个测试数据必须能追溯到具体 AC 或业务场景。

测试代码中必须用注释标注：
```java
// AC-001: 正常创建工单
// 数据来源: Story §AC-001 示例值
Long userId = 123456789012345L;  // 来自上游会话服务的 userId
String problemDescription = "用户进线咨询支付问题";  // 真实业务文案
TicketPriority priority = TicketPriority.HIGH;  // 枚举值
```

##### 保障 3：失败诚实暴露

> 测试失败的详细信息（assertion 失败的具体值、expected vs actual 差异）必须原样输出，不得"包装"成通过。

❌ 错误做法：
```
测试结果：✅ Pass（共 25 个测试）
```

✅ 正确做法：
```
测试结果：✅ Pass（共 25 个测试）
关键测试输出：
  - transition_success: PASS (128ms)
    断言: assertEquals(200, response.getStatusCode().value())
    实际: 200 ✓ 期望: 200 ✓
  - transition_userNotFound: PASS (45ms)
    断言: assertEquals(21041, response.getBody().getCode())
    实际: 21041 ✓ 期望: 21041 ✓
```

##### 保障 4：禁止"修复测试"代替"修复代码"

> 如果 AI 自行修改了**已通过审核的测试代码**（不是新增），必须：
> 1. 在测试报告中标注"修改了 {测试方法}，原因：{...}"
> 2. 给出"原测试期望值错了"的具体证据（如：原期望值与 AC 描述不符）
> 3. 获得用户确认

未获用户确认的"修复测试" = 伪造测试。

##### 保障 5：覆盖率高但不要凑数

> 覆盖率数字必须真实。不为了数字而堆测试。

| 指标 | 必须达到 | 但不能凑数 |
|------|---------|----------|
| 行覆盖率 | ≥ 80% | 不通过测 getter/setter 凑 |
| 分支覆盖率 | ≥ 70% | 不通过重复断言同一分支凑 |
| AC 覆盖率 | **100%（🔴 强制）** | 不可漏 AC |

---

#### 🔴 测试真实性门禁（⑥ 完成判定的硬前置）

- [ ] 8 类禁止的伪造手段扫描 0 命中（必须执行 `scripts/test_authenticity_scan.py`；BLOCKER=0）
- [ ] 原始测试证据已归档（命令、退出码、stdout/stderr、Surefire/Failsafe XML、扫描报告路径齐全）
- [ ] 测试报告统计与 XML 对账一致（tests/failures/errors/skipped 不允许人工估算）
- [ ] 跳测/忽略失败参数 0 命中（`skipTests` / `maven.test.skip` / `testFailureIgnore` / 未解释的 excludes）
- [ ] `skipped=0`；如不为 0，必须在 Story "可跳过 AC/用例"章节已有负责人、原因、补测时间
- [ ] 关键测试代码已向用户呈现（不是只说"通过"）
- [ ] 测试数据可追溯到 Story/Task（无"假设" / "TODO"）
- [ ] 测试发现数对账完成（测试用例文档应跑数 ↔ 测试源码方法数 ↔ XML 实际执行数）
- [ ] 核心 AC 至少包含一个负向/失败注入验证（状态非法、参数非法、DB 约束、外部依赖失败、事务回滚等按 Story 选择）
- [ ] 失败诚实暴露（无"包装通过"）
- [ ] 无"修复测试"代替"修复代码"（如有，已附理由 + 用户确认）
- [ ] AC 覆盖率 100%（无 AC 漏测）

**任一未达成 → 测试报告作废，⑤ Coding 必须返工。**

**建议增强（高风险 Story 强制）：** 状态机、资金/权限、核心落库、并发幂等类 Story 必须做轻量突变抽检：临时反转一个关键条件、删除一个必填校验或替换一个错误码，测试必须失败。若突变后仍全绿，说明测试未杀死错误实现，本轮测试无效。



---

## 📋 ⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 硬前置）

> **🔴 【2026-06-06 重大重构】本章定义已迁出到 [`code-review-skill.md` §闸 1 ⑥bis 一致性闸](../../code-review-skill.md)。Coding 阶段自检可参考；评审阶段必须走 code-review-skill.md。**

> **来源：** 原 ae-sdd-skill.md Phase 3 ⑥bis 闸。CodeReview 出具前必须以**当前磁盘代码**为锚反向核查 DR / Story / Task / 测试用例 / 代码 五方一致。

### ⑥bis 编码后全切面一致性核查闸（🔴 CodeReview 的硬前置，每轮编码后强制执行）

> **为什么要这道闸：** Story Review 只在编码前跑、只核 Story↔DR；编码过程会让代码逐步偏离设计（漂移），而原流程没有任何节点在编码后强制回头问"Story / Task / 测试用例 还和代码一致吗"。结果是一个 Story 改了 5 轮、漂移累积、最终要靠外部 Story 来发现。这道闸就是补上这个真空：**每一轮编码完成后，强制以代码为锚反向核查五方一致性，否则不允许进入 CodeReview。**

**触发时机：** ⑥ 完成判定全部通过后、⑦ CodeReview 之前。**每一轮 Coding（含缺陷修复轮）都必须执行，不可因"只改了一点"而跳过。**

**核查方向（关键）：以"实际代码"为锚点，反向核查，而非以文档为锚点。**
- 错误做法：拿 Story 逐条去代码里找"实现了没" → 容易漏掉代码里多做/做歪但文档没写的部分。
- 正确做法：拿**代码实际行为**逐条反查 DR / Story / Task / 测试用例，问"这段代码对应哪条设计？设计还和它一致吗？"

**五方核查对象：** DR（真相源基线）、Story、Task、测试用例、实际代码。

**核查范围：🔴 全章节，禁止只核当轮 diff。** 多轮编码下，本轮的改动可能让前几轮已"一致"的章节失效，必须全量回扫（与 [[codereview-full-doc-rescan-gate]] 一致）。

**核查方式：🔴 禁止裸 ✅，每一项结论必须附客观证据（与 [[evidence-bound-check-no-bare-checkmark]] 一致）。**

**产出物：《全切面一致性核查表》**，作为 CodeReview 报告的强制前置章节嵌入（见模板"零、全切面一致性核查表"）。表格逐行以代码为单位：

| 代码位置（文件:行号） | 代码实际行为 | 对应 DR 条款 | 对应 Story 章节 | 对应 Task 章节 | 对应测试用例 ID | 一致性结论 | 证据 |
|---|---|---|---|---|---|---|---|
| `XxxService.java:88` | 状态 NEW→DOING | DR-3.2 | Story §4.2 | Task-2 §3 | TC-002 | ✅一致 | 字段类型/取值核对 |
| `XxxService.java:120` | 多写了一个 audit 字段 | 无 | 无 | 无 | 无 | 🔴漂移(代码多做) | 代码有/设计无 |

**漂移分级与修复闭环：**

| 漂移类型 | 含义 | 处置 |
|---|---|---|
| 🔴 代码多做（设计无） | 代码实现了设计里没有的行为 | 判断该行为是否必要：必要→补回 DR/Story/Task；不必要→删代码 |
| 🔴 代码漏做（设计有） | 设计要求但代码未实现 | 回 Phase 2 补实现 |
| 🔴 代码做歪（与设计冲突） | 字段类型/状态流转/错误码与设计不符 | 判断对错：设计对→改代码；代码对→按层级回改 DR/Story/Task |
| 🟠 测试用例未覆盖 | 代码行为无对应测试用例 | 补测试用例并跑通 |

**修复按层级回改（不可只改一处）：** 漂移定位到哪一层就从那一层开始向下级联——
- DR 层错 → 改 DR → 级联 Story → Task → 代码 → 测试用例；
- Story 层错 → 改 Story → 级联 Task → 代码 → 测试用例；
- Task 层错 → 改 Task → 代码 → 测试用例；
- 代码层错 → 改代码 → 测试用例。
修复后**重新执行本闸全量核查**，直至核查表无 🔴 漂移。

**🔴 核心落库路径真实 DB 硬门禁：** 凡涉及 INSERT/UPDATE/DELETE 的核心业务路径，核查表对应行的"证据"列**禁止填 Mock 测试结果**，必须填真实 DB（H2/TestContainers）集成测试的落库验证输出（INSERT 后 SELECT 可查到、UPDATE 后字段正确、事务回滚后数据未污染）。全 Mock 的落库路径视为"未验证"，按 🔴 漏做处理。

**出闸条件：** 核查表覆盖全部代码改动 + 无 🔴 漂移 + 核心落库路径有真实 DB 证据。
**未达标 → 回 Phase 2 修复，不允许进入 CodeReview。**


---

## 📋 ⑦bis 全链路对称性核查闸（🔴 流程收尾强制，人工审核前最后一道闸）

> **🔴 【2026-06-06 重大重构】本章定义已迁出到 [`code-review-skill.md` §闸 2 ⑦bis 对称性闸](../../code-review-skill.md)。Coding 阶段自检可参考；评审阶段必须走 code-review-skill.md。**

> **来源：** 原 ae-sdd-skill.md Phase 3 ⑦bis 闸。人工审核点 4 前必须完成，5 层双向追溯无 🔴 断链。

### ⑦bis 全链路对称性核查闸（🔴 流程收尾强制，人工审核前最后一道闸）

> **与 ⑥bis 的分工：** ⑥bis 是"每轮编码后、以代码为锚"查内容漂移；⑦bis 是"流程结束时、以需求为锚"查**结构对称**——DR→Story→Task→实现→测试用例**五层是否一一对应、谁都不多谁都不少**。本质是一张贯穿五层的**需求追溯矩阵**。

**触发时机：** CodeReview 报告出具、产出物对账通过后，人工审核点 4 之前。**最后一轮 Coding 收尾时强制执行。**

**核查目标：双向对称，两个方向都要查——**
- **正向（自上而下，查"漏"）：** DR 每条业务规则 → 是否都有 Story 承接 → 是否都拆成了 Task → 是否都有代码实现 → 是否都有测试用例覆盖。任一层断链 = 该需求未落地。
- **反向（自下而上，查"多"）：** 每段代码 / 每个测试用例 → 是否能回溯到 Task → Story → DR。回溯不到 = 凭空多做（无需求来源），需判断删除或补登记。

**产出物：《全链路对称性追溯矩阵》**（追加到 CodeReview 报告或独立 `{STORY-ID}-追溯矩阵.md`）。一行一条 DR 业务规则，五列贯穿：

| 追溯 ID | DR 条款 | Story 章节 | Task | 代码实现(文件:行号) | 测试用例 ID | 对称性结论 |
|---|---|---|---|---|---|---|
| T-01 | DR-3.2 状态流转 | §4.2 | Task-2 | `XxxService.java:88` | TC-002 | ✅ 五层贯通 |
| T-02 | DR-3.5 超时关闭 | §4.5 | Task-4 | （缺） | （缺） | 🔴 断链：未实现 |
| T-03 | — | — | — | `XxxService.java:120` | — | 🔴 多做：无 DR 来源 |

**断链分级与处置：**

| 断链位置 | 含义 | 处置 |
|---|---|---|
| 🔴 DR 有 → Story 无 | 需求未被 Story 承接 | 回 Story 层补（Story Update → Task Generate → Coding） |
| 🔴 Story 有 → Task 无 | 设计未被拆解 | 回 Task 层补（Task Generate → Coding） |
| 🔴 Task 有 → 代码无 | 设计未实现 | 回 Phase 2 补实现 |
| 🔴 代码有 → DR 无（凭空多做） | 实现无需求来源 | 必要→反向补登记 DR/Story/Task；不必要→删代码 |
| 🟠 实现有 → 测试用例无 | 行为未被验证 | 补测试用例并跑通 |

**核查范围：🔴 全量，覆盖 DR 中本 Story 涉及的全部业务规则**，不只本轮改动。**禁止裸 ✅**：每行"对称性结论"必须能点到具体的 Story 章节号 / Task 编号 / 文件:行号 / 测试用例 ID，点不到的不得标"贯通"。

**出闸条件：** 矩阵覆盖 DR 中本 Story 涉及的全部规则 + 无 🔴 断链（漏做/多做全部闭环）+ 无 🟠 未覆盖（或已补）。
**未达标 → 按断链层级回改并级联，重新执行本闸，直至五层对称。不允许进入人工审核点 4。**



---

## 📋 Coding 问题分层排查与修改链

> **🔴 【2026-06-06 重大重构】本章已整合到 [`code-review-skill.md` §异常路径 A1-A4](../../code-review-skill.md)。Coding 实时追溯链（Task → Story → DR → AI 犯蠢）已统一归口。**

> **来源：** 原 ae-sdd-skill.md "## 异常处理" 章节的 "Coding 问题分层排查与修改链" 小节，与本 SKILL 自身的"异常路径 A1-A4"整合为统一体系。

当 Coding 阶段发现问题，按以下顺序逐层排查、针对性修改：

### Coding 问题分层排查与修改链

当 Coding 阶段发现问题，按以下顺序逐层排查、针对性修改：

```
┌─────────────────────────────────────────────────────────────────┐
│  Coding 发现问题                                                  │
│      │                                                           │
│      ▼                                                           │
│  ① 立即记录 → {story}-开发问题记录.md                            │
│      │                                                           │
│      ▼                                                           │
│  ② 判定：是否 Task 问题？                                         │
│      │                                                           │
│      ├─ 否 → 直接修复 Coding → 继续 Coding                        │
│      │                                                           │
│      └─ 是 → ③ 判定：是否 Story 问题？                            │
│                  │                                               │
│                  ├─ 否 → 直接修改 Task → 重新 Coding              │
│                  │         （Task 本身描述与 Story/AC 不符）        │
│                  │         ⚠️ 边界：Task 层问题且与 Story/DR 无关时方可直接修；│
│                  │         若问题根因在 Story 或 DR，必须先走 proposal-skill │
│                  │         触发 story-update / dr-update，不可直接改 Task。  │
│                  │                                               │
│                  └─ 是 → ④ 判定：是否 DR 问题？                    │
│                              │                                   │
│                              ├─ 否 → 更新 Story → 重新生成 Task   │
│                              │     （Story 描述不清/遗漏，与 DR 无关）│
│                              │                                   │
│                              └─ 是 → 更新 DR → 更新 Story          │
│                                        → 重新生成 Task             │
│                                        → 重新 Coding              │
│                                  （DR 本身有缺陷）                │
└─────────────────────────────────────────────────────────────────┘
```

**排查判定标准：**

| 层级 | 判定问题 | 举例 |
|------|---------|------|
| Task 层 | Task 实现与 Story/AC 是否矛盾？ | Task 描述的状态机与 AC 不符、接口签名与 AC 不符 |
| Story 层 | Story 描述是否清晰/完整，与 DR 是否矛盾？ | Story 遗漏了异常流程描述、Story 与 DR 约束冲突 |
| DR 层 | DR 本身是否完整/合理？ | DR 业务规则本身有漏洞、边界条件遗漏 |

**修改影响范围：**

| 问题层级 | 修改范围 | 后续动作 |
|---------|---------|---------|
| DR 层 | DR Update（必须参考 `templates/design/dr-template.md`）+ 触发 Story Review | → Story Update → Task Generate → Coding |
| Story 层 | Story Update（必须参考 `templates/design/be-story-template.md`） | → Task Generate → Coding |
| Task 层 | 直接修改 Task 文档（必须参考 `templates/design/be-task-template.md`） | → 重新 Coding |
| Coding 层 | 直接修复代码 | → 继续 Coding |

> **DR 层修改后必须触发 Story Review**：DR 是设计链路的源头基准，修改 DR 后 Story 与 DR 的一致性必须重新验证。

**关键原则：**
- 严格在问题发生的层级解决，禁止跨越层级处理
- Task 问题与 Story 无关 → 直接改 Task，不动 Story
- Story 问题与 DR 无关 → 直接改 Story，不动 DR
- 只有确认 DR 本身有缺陷时，才走完整链路

**与本 SKILL 异常路径 A1-A4 的关系：**
- 本节的"分层排查" = A2 步骤的具体执行流程
- 本节的"分层判定标准" + "修改影响范围" = A3 步骤的判定依据
- 本节的"关键原则（不跨层处理）" = A3 的强制约束

**🔴 强约束：** "纯实现问题→直接改代码"是兜底结论，不是第一选项。必须先逐层判定 Task / Story / DR 都没有问题，才能判定为纯实现问题直接改代码。问题记录的根因分析字段必须能看到这 4 层逐层判定结论，禁止跳过第 1/2 层直接勾"纯实现"。详见本 SKILL 异常路径 A2/A3 步骤。

---

## 📋 实战闸沉淀（来自 d--Item-document 项目踩坑，🔴 强制）

> **🔴 【2026-06-06 重大重构】本章 6 闸门全部迁出到 [`code-review-skill.md` §第七步 闸 1-7](../../code-review-skill.md)。保留本章作为"踩坑历史归档"参考；闸门定义以 code-review-skill.md 为准。**

> **来源：** d--Item-document 项目 STORY-002 反复改 5 轮的病根沉淀。这些闸在原项目中已写为 memory 反馈，本次回流到 coding-skill.md 本体，避免在新项目重蹈覆辙。

### 闸 1：全切面一致性核查闸前的前置 — 全文档回扫闸（🔴 CodeReview 必跑）

> **来源：** `codereview-full-doc-rescan-gate` — 出 CodeReview 标注"DR-Story-Task 一致 ✅"前，**必须对 Story 主文档全章节回扫**，不能只核对当轮改动范围（diff）就下"三方一致"结论。

**为什么必须有这一步：**
STORY-002 历轮 CodeReview 都只核了当轮新增范围就标"三方一致 ✅"，实为虚报——A 类"文档落后于代码"债持续累积。改主流程不联动关联章节又产生新矛盾（如 r12 改双边定位但异常表/索引说明/错误码表未同步）。最终被下游 STORY-009 一致性核查揪出 12 项偏差。

**执行步骤：**
1. 出 CodeReview 前，用关键字回扫主文档全文（**旧端点 / 旧错误码 / 已删除的类名 / 已改类型的字段名 / 已废弃的字段**等），逐处确认是否残留
2. 改主流程/契约后，必须联动检查所有引用它的章节：**异常流程表 / AC 验收表 / 错误码表 / 索引说明 / 偏离声明 / 未决问题**
3. 核心落库路径不能只靠 mock 单测 — 测试 schema/DDL 必须与设计 DDL 的 NOT NULL 等约束对齐，并补真实 DB 约束回归测试（mock 会掩盖约束类缺陷）
4. "三方一致 ✅" 是可证伪的结论，写之前要能列出回扫了哪些关键字、哪些章节，否则不准写

**门禁：**
- 🔴 未做全文档回扫 → CodeReview 报告作废
- 🔴 "三方一致"未附回扫证据 → 按 [[evidence-bound-check-no-bare-checkmark]] 整改

### 闸 2：禁裸 ✅（🔴 任何检查项的 ✅ 必须附客观证据）

> **来源：** `evidence-bound-check-no-bare-checkmark` — 关键检查项（一致性 / 契约对齐 / 落库正确 / 回扫完成等）打 ✅ 时，必须附客观证据，禁止只靠"自我声明"通过。

**为什么必须有这一步：**
一个只需要"宣称"就能通过的检查项，在压力下必然被宣称通过。STORY-002 反复改 5 轮的病根之一：CodeReview 的"三方一致✅""契约一致✅"都是裸 ✅，没人要求附证据，于是历轮用"只核当轮 diff"假装通过，债越积越多。

**执行规范：**
- **外部契约一致** → 附真实报文 / 官方文档 ↔ 字段逐字段对照表
- **真实落库正确** → 附真实 DB 集成测试的执行输出（不是 mock 全绿）
- **DR-Story-Task-代码一致** → 附每个章节的对照结论 + 代码文件:行号（不是 diff 范围）
- **联动修改完成** → 附"引用该处的章节清单" + 每章同步状态
- **全文档回扫完成** → 附回扫的关键字清单 + 各处判定结论

**门禁：**
- 🔴 任何 ✅ 未附证据 → 视为检查未通过，必须补证据
- 🔴 报告"通过"与"附证据"二选一时，优先"附证据"——证据缺则结论无效

### 闸 3：报告-代码对账（🔴 任何报告必须验证报告项在代码中真实存在）

> **来源：** `feedback_report-code-reconciliation` — Coding/测试/CodeReview 报告完成后必须新增产出物对账环节，验证报告声明项在代码中真实存在。

**为什么必须有这一步：**
报告与代码可能漂移：报告说"实现了 X"、实际代码没写 / 写错位置 / 写错类。CodeReview 自审时若只看报告不看代码，会批准不存在的实现 → 上线后事故。

**执行规范：**
- **报告中的每个能力声明** → 必须用 grep 实际验证代码中存在
  - 工具：`grep -r "方法名" --include="*.java"` / IDE "Find Usages"
  - 输出：行号 + 代码片段
- **报告中的每个文件路径** → 必须用 `ls` / 文件管理器验证存在
- **报告中的每个测试方法** → 必须用 grep 验证 `@Test` 标注 + 类路径真实存在
- **报告中的每条业务规则** → 必须用 grep 验证规则所在的方法存在
- **报告与代码不一致** → 必须修正报告或补充代码，二选一，不得跳过

**门禁：**
- 🔴 报告声明项无代码证据 → 视为报告失真，必须返工
- 🔴 任何"声称已实现"未 grep 验证 → 视为虚假报告

### 闸 4：编码后交付表（🔴 每次写完代码必须输出给用户核查）

> **来源：** `feedback_post-coding-delivery-table` — 每次写完代码后必须按工程调用顺序输出本次修改文件的交付表，表格字段至少含类型、文件路径、变更类型、说明。

**为什么必须有这一步：**
用户原话："以后每次写完都要写个交付表给我，这样我才好检查代码和提交。" 在多文件、多模块的改动场景下，调用顺序表能让用户直接照着自上而下 diff 校验，避免漏改、漏看、漏提交。

**交付表规范：**
- **必填列：** 类型 + 文件路径 + 变更类型 + 说明
- **推荐列：** 所属层级 + 文件完整路径（markdown 链接形式）+ 关键改动（一句话）+ 涉及行号
- **表格样式：** 与 Coding 报告 `## 2. 分层实现清单` 保持一致，按层级分段输出：`2.1 SPI 层`、`2.2 Domain 层`、`2.3 Application 层`、`2.4 Infrastructure 层`、`2.5 Interfaces/BFF 层`、`2.6 Test 层`、`2.7 文档/配置`。
- **变更类型枚举：** `新增 / 修改 / 删除 / 无改动 / 仅测试 / 仅文档`。经核对但无需改动的关键文件可写 `无改动`，说明中写清原因。
- **调用顺序约定（icec-cloud-boss 项目分层架构）：**
  1. SPI 层（跨服务契约，如有）
  2. Domain 层（领域模型 / Facade 接口 / Repository 接口）
  3. Application 层（业务编排 / Orchestrator）
  4. Infrastructure 层（持久化 / Facade 实现 / Mapper / 外部服务）
  5. Interfaces/BFF 层（HTTP / JobHandler / BFF 入口）
  6. Test 层（同各模块的 `src/test`）
  7. 文档/配置（docs/、`.auto-engineering/`、YAML、DDL 等）

**交付表示例：**

```markdown
### SPI 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| SPI 接口 | `icec-cloud-life-spi/icec-cloud-life-im-spi/src/main/java/.../ImSessionService.java` | 修改 | 新增 `getLatestMessageAt` / `batchGetLatestMessageAt` 方法签名 |

### Domain 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| Facade 接口 | `icec-cloud-life-cs/icec-cloud-life-cs-domain/src/main/java/.../ImSessionServiceFacade.java` | 修改 | CS 防腐层新增 2 个方法 |
| Repository 接口 | `icec-cloud-life-cs/icec-cloud-life-cs-domain/src/main/java/.../CsTicketRepository.java` | 修改 | 新增 `syncLastMessageAtFromIm(Long ticketId, Date lastMessageAt)` |

### Application 层

| 类型 | 文件路径 | 变更类型 | 说明 |
| --- | --- | --- | --- |
| Orchestrator | `icec-cloud-life-cs/icec-cloud-life-cs-application/src/main/java/.../CsTicketCloseOrchestrator.java` | 修改 | CloseContext 加 `lastMessageAt` 字段 |
```

**门禁：**
- 🔴 TodoWrite 所有 todo 标 completed 后，最终汇报的第一项必须是交付表
- 🔴 交付表未按调用顺序排列 → 视为未完成

### 闸 5：核心落库路径禁全 mock（🔴 必须有真实 DB 集成测试覆盖 NOT NULL 等约束）

> **来源：** `test-prefer-real-http` + `story-002-test-process-issue` — 接口测试能走真实 HTTP 的必须走真实 HTTP（SpringBootTest RANDOM_PORT + TestRestTemplate），MockMvc 仅作降级须注明原因；核心落库路径必须用真实 DB 集成测试覆盖 NOT NULL / 唯一约束等。

**为什么必须有这一步：**
mock 测试会掩盖真实约束类缺陷。STORY-002 落库漏 NOT NULL 字段是被 mock 测试掩盖的——mock 的 Repository.save() 不会触发 DB 约束检查。后续真实运行才暴露 → 必须返工。

**执行规范：**
- **核心落库路径（涉及资金 / 状态 / 权限的写操作）** → 必须有真实 DB 集成测试
  - 测试框架：SpringBootTest + 真实 H2 / Testcontainers / 公司测试库
  - 测试 schema/DDL 必须与设计 DDL 的 NOT NULL / 唯一约束 / 索引对齐
  - 不得用 `@Disabled` / 注释掉 / 改回 mock 来"让测试通过"
- **HTTP 接口测试** → 优先 `@SpringBootTest(webEnvironment = RANDOM_PORT) + TestRestTemplate`
  - 真实走完 Controller → AppService → Domain → Repository → DB 全链路
  - 验证事务边界、SQL 实际执行、JSON 序列化
- **MockMvc 降级** → 仅在真实 HTTP 装配不可行时使用，必须在测试注释中说明原因

**门禁：**
- 🔴 核心落库路径无真实 DB 集成测试 → 视为测试无效
- 🔴 MockMvc 使用未注明原因 → 视为降级未声明

### 闸 6：SKILL 级联更新（🔴 Story 变更后必须自动级联更新 Task，禁止等用户提醒）

> **来源：** `feedback_auto-skill-cascade` — Story 文档变更后必须自动触发 Task 文档同步更新，不中断流程询问用户。

**为什么必须有这一步：**
SKILL 异常处理链路明确定义了级联关系：Story 更新 → Task 受影响 → 重新生成 Task。用户不应该需要手动提醒这个步骤。

**执行规范：**
- 每次 Story 文档发生变更（**新增 AC / 修改主流程 / 修改接口契约 / 修改数据模型**等），立即检查哪些 Task 文档受影响并同步更新
- 同步更新作为 Story Update 的一部分自动完成，**不中断流程询问用户**
- 级联更新后必须重启 Task 0 + 全局 Task Review
- 跨 Story 的级联（如字段基线变更）→ 触发所有受影响 Story 的 Story Update

**门禁：**
- 🔴 Story 变更未级联更新 Task → 视为 SKILL 违规，按 [[feedback_coding-skill-problem-record]] 整改
- 🔴 级联更新后未重启全局 Task Review → 视为闭环不完整

### 闸 7：🔴 静态扫描（每轮编码完成后强制执行）

> **来源：** `feedback_post-coding-static-scan` — 裸眼自审容易漏"全限定名"、"未使用 import"、"跨层误引用"等细节错误。**Skill 仅承担通用部分**（任何 Java/Spring 项目都适用）；**工程特定部分**（如 `com.casstime.cloud.life.x.y.` 包路径扫描）由项目资产配置。
>
> **历史教训：** STORY-021-BE 暴露的 `java.util.Date` 全限定名遗漏——line 542 写了 `java.util.Date`，line 64 已有 `import java.util.Date`，`grep "java.util."` 1 秒能 catch，裸眼漏检。

**为什么必须有这一步：**

裸眼自审 / IDE 警告容易漏"`java.util.Date` 与 import 重复"、"import 写完后忘了删"、"跨层 import 误引用"等细节。1 秒 grep 比 30 分钟 Code Review review 高效。

**通用静态扫描（Skill 必跑，任何 Java 项目适用）：**

```bash
# 1. 标准库全限定名扫描（除 import 块外不应出现）
#    任何 Java 项目通用：java.util.* / java.sql.* / java.io.* / java.time.* / java.math.* / java.net.*
grep -rn "^[^/*].*\bjava\.\(util\|sql\|io\|time\|math\|net\)\.\w" \
  --include="*.java" src/main/java/ \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空；非空 → 修改为已 import 的短名

# 2. 未使用 import 扫描（IDE 警告即可）
#    IntelliJ: Code → Optimize Imports
#    Eclipse: Source → Organize Imports
#    或 grep "^import" 人工对账

# 3. 静态导入滥用扫描
grep -rn "^import static " --include="*.java" src/main/java/ | wc -l
# 项目中应有节制使用，不应过多
```

**工程特定静态扫描（项目资产 §6.11 配置）：**

参见 `{project-key}/{project-key}.assets.md` 的"工程特定静态扫描清单"——典型包括：
- 项目包路径全限定名扫描（如 `com.casstime.cloud.life.x.y.`）
- 跨层误引用扫描（如 application 不应 import infrastructure.persistence）
- SQL 关键字在 Service 层的扫描
- 项目特有的命名规范扫描

**判定规则：**
- 通用扫描任一命中 → 视为"裸眼自审漏检" → 修复 → 重跑全部扫描
- 工程特定扫描命中 → 视为"违反项目资产约束" → 修复 → 重跑
- 所有扫描通过 + 编译通过 + 测试通过 + Step 9 一致性闸通过 = 编码真正完成

**门禁：**
- 🔴 写完代码后未跑静态扫描 → 视为"未完成编码"
- 🔴 静态扫描发现的问题未修复就提交 → 按"伪造测试"同等级处置

---

**与现有 9 步流程的对应：**
| 实战闸 | 对应步骤 |
|--------|---------|
| 闸 1 全文档回扫 | 第九步（一致性闸）前的前置 → CodeReview 必跑 |
| 闸 2 禁裸 ✅ | 贯穿所有 9 步，所有 ✅ 判定都受此闸约束 |
| 闸 3 报告-代码对账 | 第八步 bis（测试报告合规性）+ CodeReview 出具前 |
| 闸 4 交付表 | 第六步（按 Task 生成代码）完成后，作为最终汇报第一项 |
| 闸 5 禁全 mock | 第七步（编译+启动+接口验证+DB 验证）的 7.5 DB 写操作落库验证 |
| 闸 6 SKILL 级联 | 异常路径 A5（触发 Story 更新 SKILL）的延伸 → 自动级联 Task |
| **闸 7 静态扫描** | **第六步（按 Task 顺序生成代码）完成后立即跑（紧跟 Step 9 之后），作为"裸眼自审补强"** |
