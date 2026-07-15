---
name: coding
description: 代码生成能力库（v3.6.1 适配器注册加载）。提供"如何写对代码"的知识：代码设计决策方法（11维CodingModel/骨架展开/分层红线）、CodeAnalysis 方法论（④bis SOP）、编码检查清单、禁止事项红线、验证判定标准、静态扫描规则。本文件是能力库，被 coding-process-skill.md 调用，不持有任何流程编排。当需要"如何生成代码/代码规范/代码设计决策/复用判断/分层职责判定"时调用本能力库。🆕 v3.6.1 新增 §13 语言/项目适配器注册加载：按项目技术栈叠加语言/项目特有编码决策（如 Java3D 适配器），纯规则仍归项目 constraints/+assets，本库不复述。
---

# CodingSKILL — 代码生成能力库（被调用，非流程节点）

> G-CODE-1 work-item 扫描只接受与当前 Story 绑定、经 plan/evidence/artifact hash 校验的 `VerificationPlan.changedPaths`；只扫描其中生产代码。缺失或空 scope 保持全仓严格扫描，测试/文档-only scope 阻断，scoped 路径不读取或创建 baseline。

> **🔴 v3.5.17 定位（能力库化）：** 本文件是**纯能力库**，提供"如何写对代码"的知识：
> - **决策方法**：11维 CodingModel、骨架展开规则、分层职责红线、复用判断
> - **CodeAnalysis 方法论**：④bis SOP（分层归类/骨架输出）、CodePlan 模板/门禁定义、G-CODEPLAN-SRC 判定
> - **编码检查清单**：经验检查清单、禁止事项红线、基准过滤器
> - **验证判定标准**：编译/启动/接口/DB/事务/漂移核查/假修复识别/静态扫描
>
> **本文件不持有任何流程编排**（步骤串联/异常路径触发/审核点/交付表/讲解规范归 [`coding-process-skill.md`](coding-process-skill.md)）。
> **调用方**：CodingProcess 在 CodeAnalysis 阶段（产出 CodePlan）和 Execute 阶段（按 CodePlan 写代码）都调用本能力库。

---

## §0 能力总览（本库提供的能力清单）

| 能力块 | 章节 | 调用阶段 |
|--------|------|---------|
| 11维 CodingModel 决策方法 | §1 | CodeAnalysis 产出 + Execute 复核 |
| 约束文件引用（9项关键规则） | §2 | 双阶段 |
| 分层职责红线 + 各层绝对禁止 | §3 | 双阶段 |
| 骨架展开规则（伪代码→代码） | §4 | Execute |
| CodeAnalysis ④bis 全套（风险预判/7章节/门禁） | §5 | CodeAnalysis |
| CodeAnalysis ④bis SOP（分层归类/骨架输出方法论） | §6 | CodeAnalysis |
| G-CODEPLAN-SRC 源码核对判定 | §7 | CodeAnalysis |
| 验证判定标准（编译/启动/接口/DB/事务） | §8 | Execute |
| 编码后漂移核查 + 假修复识别 | §9 | Execute |
| 异常根因 4 层分类判定 | §10 | 双阶段（报错追溯时） |
| 经验检查清单 + 禁止事项红线 | §11 | 双阶段 |
| 静态扫描 grep 规则 | §12 | Execute |
| 语言/项目适配器注册加载（🆕 v3.6.1） | §13 | CodeAnalysis + Execute（加载上下文时触发） |

---

## §1 CodingModel 决策方法（11 维）

> **能力定位：** 这是"代码设计时的风险决策方法"。CodeAnalysis 阶段用它产出决策记录，Execute 阶段用它复核（不重新产出）。
>
> **加载路径：** `standards/thinking/be-coding-thinking-engine.md`

**11 维 CodingModel 决策记录表：**

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

**判定标准：** 任一维度结论为空或"不知道" → 决策不完整，须向上游（Story / TestCase / DR）追溯补充信息。

> **🆕 v3.6.2 证据缺失降级规则（测试反馈补）：** 上表"证据列"（文件:行号 / Story AC / TestCase ID）在纯场景描述、尚未产出上游文档时可能空缺。证据缺失时的处理：
> - 🔴 **禁止**：用占位内容（如"假设 userId=1L"）凑证据后继续推进——按共有「禁止猜测/禁止杜撰」红线，无来源不得编造。
> - ✅ **正确降级**：① 若该维度结论可从项目资产/constraints 派生 → 引用资产作为证据；② 若需上游 Story/Task/DR 提供 → 在 CodeAnalysis 产出中标 `{待确认:需 Story §X 提供}` 并**列入 CodePlan 待核实清单**（与 §7 G-CODEPLAN-SRC 同机制）；③ 待核实清单非空 → CodePlan 视为草案，**禁止进 Execute**，直到证据补齐。
> - 即：证据缺失**不得跳过决策**，也不得**编造证据**，而是**诚实标记 + 列待核实 + 阻断 Execute**。

---

## §2 约束文件引用（9 项关键规则）

> **能力定位：** 编码时必须对照的判断标准。通过 `document-storage-skill.get_constraints(projectKey)` 加载。

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
| **`be-coding-thinking-engine`** | **设计→实现→测试全链路思考框架（通过 `get_thinking_engine(projectKey)` 加载）** |

---

## §3 分层职责红线（写代码时反复对照，违反即阻断）

> **能力定位：** 判断"这段代码属于哪层"的决策标准。完整清单见 `get_constraints(projectKey)["project-structure"]` 的「分层职责红线」节。

**Domain 写领域逻辑，Application 写业务编排，Repository 只做数据存取。**

| 这段代码是… | 归属层 | 落点 |
|---|---|---|
| 业务规则 / 能不能 / 算什么（状态能否流转、金额怎么算、不变量校验） | **Domain** | 实体充血方法 / DomainService |
| 先做A再做B / 协调谁调谁 / 事务从哪到哪 / 转 DTO | **Application** | AppService |
| 把数据存进去 / 取出来 / 转 PO↔DO 格式 / 拼查询条件 | **Repository** | RepositoryImpl |
| 参数格式校验（@Valid） | **Interfaces** | Controller/Impl |

**🔴 Repository 绝对禁止：** 状态流转判断、业务规则校验、跨聚合编排、存取方法里塞 if-业务分支。仓储方法名只能是 `findByXxx`/`save`/`updateStatus` 这类存取语义；一旦出现 `handleXxx`/`processXxx`/`checkXxx业务` 就是放错层。
**🔴 Application 绝对禁止：** 写领域规则（下沉到 Domain）、写 SQL/持久化细节。
**🔴 Domain 绝对禁止：** 串多个外部服务的编排、出现 PO/DTO/SQL。

---

## §4 骨架展开规则（Task 伪代码 → 完整方法体）

> **能力定位：** Execute 阶段把伪代码翻译成代码的展开规则。Task 文档的"方法级逻辑"表格中，每个逻辑步骤以动词开头，按以下规则翻译。

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

---

## §5 CodeAnalysis ④bis：CodePlan 输出（CodeAnalysis 能力本体）

> **能力定位：** CodeAnalysis 阶段产出 CodePlan 的方法论。CodingProcess 调用本能力产出 `{WORKITEM-ID}-CodingPlan.md`，Story 仅是可选上游设计关系。

### §5.0 风险预判（必须先于 7 章节执行）

> **为什么前置：** 7 章节是"怎么写"，但动笔前必须先想清楚"这个 Story 有哪些风险，方案是否已覆盖"。漏掉风险预判，章节4的 SQL 可能缺乐观锁，章节2的类骨架可能缺幂等设计。

完整任务按 `../../standards/thinking/be-coding-thinking-engine.md §1.4 风险预判·11维度` 逐维过一遍。微任务按同文档 §0.5 裁剪为原子性、并发安全、幂等性、外部依赖容错、安全 5 维；发现架构/契约/数据模型/跨服务影响时立即升级完整 profile。

> **📍 维度清单与 §1 CodingModel 决策表的 11 维完全一致**（① 原子性 … ⑪ 可运维性），第4维统一命名"同步/异步解耦"。

| 维度（见 §1 表） | 本 Story 是否涉及 | 方案 / 有意不做的理由 |
|---|---|---|
| ① 原子性 | 是 / 否 | |
| ② 并发安全 | 是 / 否 | |
| ③ 幂等性 | 是 / 否 | |
| ④ 同步/异步解耦 | 是 / 否 | |
| ⑤ 数据一致性 | 是 / 否 | |
| ⑥ 外部依赖容错 | 是 / 否 | |
| ⑦ 性能瓶颈 | 是 / 否 | |
| ⑧ 资源隔离 | 是 / 否 | |
| ⑨ 安全 | 是 / 否 | |
| ⑩ 可观测性 | 是 / 否 | |
| ⑪ 可运维性 | 是 / 否 | |

**判定标准：** 风险预判表未完成 → 禁止进入 7 章节。每个「是」必须在对应章节中体现（如：①原子性=是 → 章节4 SQL 必须写事务边界）。每个「否」必须写明理由，不允许空列。

### §5.1 CodePlan 必须包含的 7 个章节

**规模分支：** 大/中/小任务执行下列 7 章节。微任务使用 Tier 1 轻量 profile，只要求：①变更范围与文件级实现顺序；②风险、停止条件与回滚；③编译/测试验证点；④需要新增或重塑类骨架时才填写类骨架并触发 G-CODEPLAN-SRC。禁止为了过门禁伪造 Story、AC、Mapper SQL 或混合压测章节。

#### 章节 1：文件级实现顺序

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
> 🔴 每个文件完成后必须有"验证列"——不允许写完一堆文件再统一编译

#### 章节 2：关键类骨架

每个核心类必须包含：类签名（注解 + 类名 + 继承/实现）、核心字段（含类型 + 一句话说明）、核心方法签名（方法名 + 入参 + 返回值 + 关键注解如 `@Transactional`）、关键方法体的伪代码（10-30 行）。

**示例：**
```java
public class Ticket {
    private Long id;
    private TicketStatus status;
    private Long version;

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

| 表名 | 字段名 | 类型 | 约束 | 对应 DO 字段 | 对应 DTO 字段 | 备注 |
|------|--------|------|------|-------------|-------------|------|
| im_ticket | id | BIGINT | PK | id | ticketId | 雪花 ID |
| im_ticket | status | VARCHAR(32) | NOT NULL | status | status | 枚举字符串 |
| im_ticket | version | BIGINT | NOT NULL DEFAULT 0 | version | — | 乐观锁 |

#### 章节 4：Mapper / Repository 关键 SQL

| # | 操作 | Mapper 方法 | 关键 SQL/条件 | 乐观锁 | 备注 |
|---|------|------------|-------------|--------|------|
| 1 | INSERT | `TicketMapper.insert(po)` | 标准 insert | 否 | 业务端生成 ID |
| 2 | UPDATE | `TicketMapper.updateStatus(po)` | `WHERE id=#{id} AND version=#{version} AND status=#{expectedStatus}` | 是 | 状态前置 + 乐观锁 |

> 🔴 关键 UPDATE 的 WHERE 条件必须明确写出（状态前置、乐观锁），不允许"看情况"。

#### 章节 5：测试用例的对应实现

| AC ID | 测试类 | 测试方法 | 测试数据来源 | Mock 范围 | 真实 DB | 真实 HTTP |
|-------|--------|---------|------------|---------|---------|----------|
| AC-001 | `TicketAppServiceIT` | `transition_success` | `Story §AC-001 示例值` | Mock: 无 | ✅ 走 H2 | ✅ 走 SpringBootTest |
| AC-002 | `TicketAppServiceIT` | `transition_userNotFound` | `Story §AC-002 示例值` | Mock: 无 | ✅ 走 H2 | ✅ 走 SpringBootTest |

> 🔴 测试数据来源必须从 Story/Task 中可追溯，禁止"假设用户 ID=1L 就能跑"。
> 🔴 核心落库路径必须用真实 DB（H2/TestContainers），禁止全 Mock。
> 🔴 核心接口必须用真实 HTTP（SpringBootTest RANDOM_PORT），禁止 MockMvc 代替。

#### 章节 6：编译与测试验证点

| 阶段 | 触发时机 | 验证命令 | 通过标准 |
|------|---------|---------|---------|
| 单文件完成 | 每写完一个文件 | `mvn -pl {module} compile` | BUILD SUCCESS |
| 单层完成 | 每层完成 | `mvn -pl {module} -am compile` | BUILD SUCCESS |
| 单 Task 完成 | 一个 Task 的所有文件完成 | `mvn -pl {module} -am test -Dtest={XxxTest}` | 全部 Pass |
| 全量完成 | Coding 全部完成 | `mvn clean verify` | 全部 Pass + 无 `Tests in error` |
| 真实 HTTP 验证 | 涉及 Controller 的 Task 完成后 | `mvn -pl {interfaces} -am verify -Dtest={XxxIT}` | SpringBootTest 起服务 + 真实 HTTP 200 |

> 🔴 不允许"写完所有代码再统一编译"——任何文件写完必须能独立编译。

#### 章节 7：调试与回滚策略

| 失败类型 | 定位方法 | 回滚策略 |
|---------|---------|---------|
| 编译失败 | IDE 跳转 + 错误栈 | git stash 当前未提交改动 → 回到上一个 commit |
| 单测失败 | 测试报告 + assertion 失败信息 | 先看是不是测试期望值错了（详见 §9 假修复识别） |
| 集成测试失败 | H2/TestContainers 日志 + SQL 输出 | 检查 DB 初始化脚本 + 数据准备 |
| 真实 HTTP 失败 | 服务启动日志 + curl 输出 | 检查 Bean 注入 + Controller 路径 |
| 性能问题 | JProfiler / Arthas | 先定位慢 SQL（EXPLAIN）→ 再定位慢方法 |

### §5.2 CodePlan 门禁（未通过禁止进入 Execute）

- **完整 profile（大/中/小）**：执行下列门禁及模板 §15 自检。
- **微任务 profile**：G-07 确认 Work Item Plan 存在；G-08 校验范围/实现顺序/风险回滚/验证四维且无 `TODO/TBD/待确认/❌`；无 Story 时 G-14 记录 N/A；无类骨架时 G-CODEPLAN-SRC 显式跳过。

- [ ] 7 个章节全部填写
- [ ] 文件级实现顺序满足"每步可独立编译"
- [ ] 关键类骨架覆盖所有层核心类
- [ ] 数据结构 / DO 字段与 Story 数据模型完全一致
- [ ] Mapper 关键 SQL 的 WHERE 条件、乐观锁、状态前置明确写出
- [ ] 测试用例对应实现的测试数据可追溯到 Story/Task
- [ ] 核心落库路径标记"真实 DB"
- [ ] 核心接口标记"真实 HTTP"
- [ ] 编译与测试验证点覆盖到每个文件/每层/每个 Task
- [ ] 调试与回滚策略完整（至少 5 种失败类型）
- [ ] 🆕 v3.4.0 **G-CODEPLAN-SRC 源码核对**：关键类骨架每个类附【已读源码：】或【待核实源码】标记，待核实清单为空
- [ ] 🆕 v3.4.0 **G-14 CodingPlan-Story 一致性**：Plan 引用 Story 文档 + AC ID 对齐 + 偏离项有 Proposal

> **📍 完整 15 条门禁自检表（含判定 SOP）在 [`be-coding-plan-template.md` §15](../../templates/coding/be-coding-plan-template.md)。**

---

## §6 CodeAnalysis ④bis 实战 SOP：分层拆分 + 项目资产映射方法论

> **能力定位：** CodeAnalysis 的核心方法论——如何把 Task 拆成类骨架、如何分层归类、如何映射包路径。

> **配套资产：**
> - 项目资产：`ae-sdd assets read coding --project <projectKey>` — 返回 §4 DDD 内部分层落点 + §5 命名约定 + §6 工程约束
> - 精准查询：`ae-sdd assets query "<name>"`（module/component/table 通用）
> - Code Plan 模板：`../../templates/coding/be-coding-plan-template.md`

### 核心设计哲学

1. **抽象分层规则不变**（4 类 + 2 可选）：请求处理 / 业务编排 / 领域逻辑 / 基础能力 + 跨模块 SPI / BFF 入口
2. **项目资产是跨项目复用的关键**：每项目一份项目资产；无资产 → 走项目资产 §9 探查 SOP 构建 → 再做映射
3. **Code Plan 不重写实现**：实现细节在 Task 里，Code Plan 只做 Task 编排 + 类骨架 + 方法级逻辑 + 目录对应

### 方法论要素（CodeAnalysis 必须覆盖）

#### 要素 1：读取项目资产

调用 `ae-sdd assets read coding --project <projectKey>`，返回 §4 + §5 + §6。
- 资产不存在 → 停止，先运行 `project-assets-update-skill §3 生成动作`
- 资产过期（`lastAuditedAt > 90 天`）→ 停止，先运行 `project-assets-update-skill §5 审计`

#### 要素 2：Task 执行顺序编排（不重写实现）

按依赖关系画执行顺序图：Domain → Infrastructure → Application → Interfaces → BFF → Test。
- **不写实现细节**（具体方法体在 Task 文档里）
- **不重写 Task 文档**

#### 要素 3：按抽象 4 层对每个 Task 做分层归类（🔴 核心）

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

**判定标准：** 🔴 分层写错（业务规则塞到 Repository/状态机计算塞到 Controller/编排逻辑塞到 Domain）= 整 Plan 打回。

#### 要素 4：把每个类按项目分层映射到确切包路径

调用 `ae-sdd assets section §4 --project <projectKey>` 匹配每个类对应的精确包路径。
- **禁止**写"包路径待定/TBD/按项目惯例"
- **禁止**复述项目资产内容（避免双源不一致）

#### 要素 5：输出类骨架

按 §5.1 章节 2 的类骨架要素输出（类签名/字段/方法签名/伪代码 ≤30 行）。

### 7 条禁令（🔴 任何一条违反 = 整 Plan 打回）

| # | 禁令 | 反例 |
|---|------|------|
| 1 | 禁止在 Code Plan 写完整实现代码 | ❌ 贴 30 行方法体 / 完整 SQL / import 块 |
| 2 | 禁止跳过项目资产直接写 Code Plan | ❌ 无项目资产就开始 Task 编排 |
| 3 | 禁止把分层写错 | ❌ 业务规则塞到 Repository / 状态机塞到 Controller / 编排塞到 Domain |
| 4 | 禁止把分层映射表里的"包路径"写成"待定/TBD" | ❌ "包路径按项目惯例" |
| 5 | 禁止重写 Task 文档 | ❌ 复制 Task 已写的方法体到 Code Plan |
| 6 | 禁止省略 7 个强制章节 | ❌ 任何章节"无则填 N/A"必须有说明 |
| 7 | 禁止用 default value 凑测试数据 | ❌ "假设 userId=1L 就能跑"（必须可追溯到 Story/Task 章节） |

---

## §7 G-CODEPLAN-SRC 源码核对判定标准（🆕 v3.4.0）

> **能力定位：** CodeAnalysis 时校验"类骨架是否核对过现有同类源码"的判定标准。CLI：`ae-sdd gates check --only G-CODEPLAN-SRC`。

**核心教训：** CodingPlan 凭推测设计类骨架会改错文件、重复造轮子、与现有建模范式不符（案例复盘见 [`lessons-learned.md` §3.1](../../standards/lessons-learned.md)）。

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

**门禁规则：**
- 类骨架章节**无任何标记** → 🔴 阻断（每个新增/修改类须附标记）
- 标【已读源码：】但**文件不存在** → 🔴 阻断（防伪造标记）
- **待核实清单非空** → 🔴 阻断（须补读后改为【已读源码：】才进 Execute）
- CodingPlan **无关键类骨架章节**（微任务场景）→ 跳过，不阻断

---

## §8 验证判定标准（Execute 阶段怎么算验证通过）

> **能力定位：** Execute 阶段编译/启动/接口/DB/事务验证的"通过标准"。CodingProcess 编排执行顺序时调用本判定标准。

### §8.1 编译验证

> **判定标准：必须在父工程根目录执行 `mvn compile`，不允许只编译子模块。** 分模块编译会漏掉跨模块依赖问题。

```bash
cd {parent-project-root} && mvn compile
```
**通过：** BUILD SUCCESS，无 error，所有子模块全部编译通过。

### §8.2 服务启动验证

```bash
cd {parent-project-root} && mvn spring-boot:run
```
**通过标准（三项全部满足）：**
- `curl localhost:{port}/actuator/health` 返回含 `"status":"UP"`
- `curl localhost:{port}/actuator/beans | grep {本Story新增的Bean名}` 确认 Bean 已注册
- 启动日志含 `Started XxxApplication`，无 `BeanCreationException`

**启动失败处理原则（必须定位根因，禁止绕过）：**

| 失败现象 | 正确处理 | 禁止做法 |
|---------|---------|---------|
| `Port already in use` | 读启动日志查占用进程、修复配置 | ❌ 直接 kill 占端口进程 |
| `BeanCreationException` | 检查 @Autowired 依赖、Bean 扫描路径 | ❌ 注释掉报错 Bean |
| `DataSource connection failed` | 检查数据库连接配置 | ❌ 改用内存 DB |
| `BeanNotOfRequiredTypeException` | 检查接口实现类匹配 | ❌ 强制类型转换 |

> 启动失败属于 🔴 阻断型，必须修复后重新验证。

### §8.3 主流程接口测试

> 🔴 **能走真实 HTTP 的接口测试必须走真实 HTTP。** L2 接口测试默认 `@SpringBootTest(webEnvironment = RANDOM_PORT)` + `TestRestTemplate`；MockMvc 仅在框架过老时降级，须注明原因。

```bash
cd {service-root} && mvn test -Dtest=*ApiIT,*ControllerIT
```
**通过：** 主流程接口经真实 HTTP 测试 Pass + HTTP 200 + 响应结构与 Story 契约一致。

### §8.4 错误码映射验证

| 场景 | 预期结果 |
|------|---------|
| 参数为空 | HTTP 400 + Story 定义的错误码 |
| 状态非法流转 | HTTP 400 + 2104X（根据 Story） |
| 未登录访问 | HTTP 401 或 Story 定义的错误码 |
| 服务内部异常 | HTTP 500 或兜底错误码 |

### §8.5 DB 写操作落库验证

```bash
cd {service-root} && mvn test -Dtest=*IntegrationTest
```
**验证点：** INSERT 后 SELECT 能查到 / UPDATE 后字段值正确（乐观锁 version 递增）/ 逻辑删除生效 / 事务提交后数据可见。

### §8.6 事务边界验证

**验证点：** 事务内操作失败 → 数据库无污染 / 事务外操作（通知、消息）不在事务内 / 事务方法调用链与 Story 一致。

---

## §9 编码后漂移核查 + 假修复识别

### §9.1 全切面一致性核查（漂移判定）

> **能力定位：** 编码后以代码为锚反向核查五方（DR/Story/Task/测试/代码）一致的判定标准。

**核查方向：以实际代码为锚点反向核查**（不是拿文档去代码里找）。
**核查范围：🔴 全章节全文件，禁止只核当轮 diff。**

**《全切面一致性核查表》结构：**

| 代码位置(文件:行号) | 代码实际行为 | DR | Story 章节 | Task | 测试用例 ID | 一致性结论 | 证据 |
|---|---|---|---|---|---|---|---|

**漂移 4 级判定表：**

| 类型 | 处置 |
|---|---|
| 🔴 代码多做（设计无） | 必要→补回 DR/Story/Task；不必要→删代码 |
| 🔴 代码漏做（设计有） | 回补实现 |
| 🔴 代码做歪（与设计冲突，如 ID 类型不符） | 设计对→改代码；代码对→按层级回改 DR/Story/Task |
| 🟠 测试用例未覆盖 | 补测试用例并跑通 |

**🔴 核心落库路径真实 DB 硬门禁：** 凡 INSERT/UPDATE/DELETE 的核心路径，核查表"证据"列禁止填 Mock 结果，必须填真实 DB 落库验证输出。全 Mock 视为"未验证"，按 🔴 漏做处理。

**出闸条件：** 核查表覆盖全部代码改动 + 无 🔴 漂移 + 核心落库路径有真实 DB 证据。

### §9.2 假修复识别规则（测试造假判定）

> **能力定位：** 判断"测试通过但实际代码有问题"的识别标准。

- 缺原始日志或 XML，只在 Markdown 中写"通过" → 虚报成功
- 测试命令带跳测参数（`-DskipTests`/`-Dmaven.test.skip`/`testFailureIgnore`），或 POM 配置跳过 → 虚报成功
- XML 实际执行数小于测试用例应跑数，且无明确解释 → 假修复风险
- `test_authenticity_scan.py` 出现 BLOCKER → 测试无效
- 测试用例覆盖不完整 → 假修复风险
- 某测试层级完全缺失（如只有 L1，没有 L2/L3）→ 假修复风险
- 失败用例被标记"跳过"而非真正修复 → 假修复
- 通过率异常高（如 100%）但 L3/L4 缺失 → 假修复风险
- 核心 AC 只有 happy path，没有失败注入/负向断言 → 假修复风险

---

## §10 异常根因 4 层分类判定

> **能力定位：** 报错时判断"根因在哪层"的分类标准。CodingProcess 异常追溯流程调用本判定。

**4 层根因分类：**

| 追溯层 | 判定内容 | 命中后处置 |
|--------|---------|-----------|
| 1️⃣ Task 文档 | 核心代码/方法签名/字段类型/依赖/包路径 是否写错/写漏？是否与 Story/AC 矛盾？ | 修 Task 文档（=修 CodePlan）→ 重新 CodeAnalysis → 重新 Coding |
| 2️⃣ Story 文档 | 接口契约/数据模型/字段类型/异常流程 是否写错/写漏？AC 是否与 DR 矛盾？ | 写 Supplement → Story Update → Task Generate 重新生成 → 重新 Coding |
| 3️⃣ DR 文档 | 业务规则本身是否有漏洞/边界条件遗漏？是否与 PRD/上游约束矛盾？ | 写 DR 补充说明 → DR Update → 通知受影响 Story → 重新 Review/Generate/Coding |
| 4️⃣ AI 犯蠢 | 前提：层 1/2/3 全部判定"无误"。类型：typo / 漏 import / 笔误 / API 误用 | 写问题记录 → 直接修代码 → 继续 Coding |

**判定标准：**
- 🔴 **禁止跳过层 1/2/3 直接判定"AI 犯蠢"**——历史上反复多轮返工的根源正是跳层判定
- 🔴 每层判定都要写入问题记录的"根因分析"字段，不允许"自我声明无误"
- 🔴 命中层 1/2/3 必须先改文档、再改代码（顺序不可颠倒）

---

## §11 经验检查清单 + 禁止事项红线

### §11.1 经验检查清单（通用，每次生成代码前逐项确认）

| # | 检查项 | 说明 |
|---|--------|------|
| 1 | pom 依赖是否被注释 | 新工程模板中 SPI 依赖常被注释，需取消注释 |
| 2 | lombok 是否显式声明 | scope=provided 不传递，每个模块需单独声明 |
| 3 | 第三方 SDK 实际包路径 | 从 jar 中确认，不要凭记忆猜测 |
| 4 | 校验注解来源包 | 按 Spring Boot 版本确认 @NotBlank 等注解的来源包 |
| 5 | Result.code 类型 | 确认是 Integer 还是 String，错误码枚举类型要匹配 |
| 6 | 字段类型与 Task 一致 | 特别注意 ID 字段是 Long 还是 String（varchar） |
| 7 | ApiResult 完整 import | 不同工程的 ApiResult 包路径不同，从现有代码 grep 确认 |
| 8 | 新模块注册到父 pom | 创建子模块后必须在父 pom 的 modules 中添加 |
| 9 | BFF Controller 实现 Rest 接口 | 不要自己加 @Api/@GetMapping，从 Rest 接口继承 |
| 10 | Feign 注解版本 | 按 Spring Cloud 版本确认 FeignClient 注解包 |
| 11 | 工具类返回类型 | 确认工具类返回类型，避免盲目类型转换 |
| 12 | VO 和 DTO 分离 | bff-api 定义 VO，SPI 定义 DTO，Controller 中做转换 |
| 13 | Task 0 必读 | 公共包路径、DO 定义、接口定义在 Task 0 中 |
| 14 | 审计字段自动填充 | 需要 MetaObjectHandler 配置，否则 FieldFill 不生效 |
| 15 | 事务外执行 | 使用 TransactionSynchronizationManager.afterCommit() |

> 📍 第 3/4/7/10/11 项的项目特定事实已下沉到 [`lessons-learned.md` §4](../../standards/lessons-learned.md)。项目特定经验检查清单通过 `ae-sdd assets read coding --project <projectKey>` 加载（§6.10）。

### §11.2 禁止事项红线

| 禁止 | 应该 |
|------|------|
| 凭记忆猜测第三方 SDK 包路径 | 从本地 jar 中解压确认 |
| 跳过编译验证直接报告完成 | mvn compile + 服务启动成功 + 接口测试通过才算完成 |
| 一次性生成所有代码再验证 | 按 Task 顺序生成，关键节点中间验证 |
| 自行命名包路径 | 使用 Task 0 和 Task 文档中定义的固定包路径 |
| 忽略 Task 文档中的核心代码 | 核心代码是模板，直接使用 |
| 跳过 Task 0 直接开始实现 | Task 0 是公共依赖说明，必须先读 |
| 修改 Task 文档中的方法签名 | 方法签名已确定，不可自行修改 |
| **发现问题直接修复，不记录** | **必须先写入开发问题记录，再分析根因，再修复** |
| **Task/Story 文档有缺陷时直接改代码** | **必须先更新文档，再按更新后的文档修复代码** |
| **启动失败时 kill 进程或绕过** | **启动失败必须读日志定位根因，属于 🔴 阻断型** |
| **只编译子模块就认为编译通过** | **必须在父工程根目录执行 mvn compile** |
| **核查/Review 用裸 ✅ 自我声明通过** | **✅ 必须附客观证据（类型核对/文件:行号/真实 DB 输出）** |
| **核心落库路径用 Mock 测试充当落库验证** | **核心路径必须用真实 DB（H2/TestContainers）验证** |
| **在 Repository 里写业务/领域逻辑** | **Repository 只做数据存取；状态流转、业务规则校验属 Domain/Application** |
| **在 Application 里写领域规则** | **业务规则下沉到 Domain；Application 只做编排** |
| **在 Domain 里写编排或持久化** | **Domain 只写领域逻辑，不串外部服务、不出现 SQL/PO/DTO** |

### §11.3 基准过滤器自检（7 项，参考 be-coding-thinking-engine）

- [ ] ①可用性：方案能正常完成业务目标？
- [ ] ②正确性：骨架逻辑步骤覆盖所有 TestCase 场景（含异常分支）？
- [ ] ③高效性：无循环IO、无慢SQL、无N+1？
- [ ] ④可维护性：分层清晰、每方法 ≤50 行？
- [ ] ⑤健壮性：CodingModel ①②③⑥维度方案已体现在代码中？
- [ ] ⑥可读性：命名见名知意、无魔法值？
- [ ] ⑦可演进性：无跨层调用、无强耦合？

---

## §12 静态扫描规则（通用 grep，Execute 编码后必跑）

> **能力定位：** 通用静态扫描命令（任何 Java 项目适用）。工程特定扫描由项目资产 §6.11 配置。

```bash
# 1. 标准库全限定名扫描（除 import 块外不应出现）
grep -rn "^[^/*].*\bjava\.\(util\|sql\|io\|time\|math\|net\)\.\w" \
  --include="*.java" src/main/java/ \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空；非空 → 修改为已 import 的短名

# 2. 未使用 import 扫描（IDE 警告即可）
#    IntelliJ: Code → Optimize Imports
#    Eclipse: Source → Organize Imports

# 3. 静态导入滥用扫描
grep -rn "^import static " --include="*.java" src/main/java/ | wc -l
# 项目中应有节制使用，不应过多
```

**判定规则：**
- 通用扫描任一命中 → 视为"裸眼自审漏检" → 修复 → 重跑全部扫描
- 工程特定扫描命中 → 视为"违反项目资产约束" → 修复 → 重跑
- 所有扫描通过 + 编译通过 + 测试通过 + 一致性核查通过 = 编码真正完成

---

## §13 语言/项目适配器注册加载（🆕 v3.6.1）

> **能力定位：** 本库 §1-§12 是**语言/项目无关的共有编码决策知识**。实际编码时还需"语言特有/项目特有"的决策知识（如 Java/Spring 的注解选型、icec 项目的 messagebus 选型）。这部分不写进本库（避免 DRY 违规与双源漂移），而是通过**注册表加载适配器**，在运行时**叠加**到本库的相应章节之上。
>
> **为什么用注册表而非合并引擎：** ae-sdd 注册表是路径解析系统（plugin_loader.py 的 resolve_skill），把一个 key 解析成一份文件。本节利用现有 `skill-new` 注册机制（已实现、已测试），由调用方在运行时**读两份文件叠加**，不依赖未实现的 skill-extends 章节合并。零 loader 代码改动。

### §13.1 加载协议 SOP（调用方执行，典型在 CodingProcess §A1 加载上下文时）

```
1. 读项目技术栈
   调 ae-sdd assets read coding --project <projectKey>（或 get_constraints(projectKey)["technology-stack"]）
   取关键判据：语言（Java/Go/...）、框架（Spring Boot/...）、项目族（icec/...）

2. 解析目标适配器 key
   按技术栈映射到适配器 provides key：
     Java + icec（casstime/life/boss） → coding-adapter-java
     其他语言/项目族                  → coding-adapter-{lang}（按需扩展，命中即叠加）
   无对应适配器（未知语言/纯共有场景）→ 跳到步骤 5（仅共有能力）

3. 解析适配器路径
   调 plugin_loader.resolve_skill("<adapter key>", ade_sdd, master)
   按三层优先级（L1 项目层 > L2 全局层 > L3 仓库根层 > L0 无适配器）解析：
     ├─ 命中 → result.resolved_path 指向某层适配器 SKILL
     └─ 未命中（resolved_path=None）→ 跳到步骤 5

4. 叠加应用（命中时）
   AI 同时读本库（§1-§12 共有）+ 适配器文件，按适配器 §9「与共有章节的映射」
   把适配器的特化决策叠加到本库对应章节。生效优先级：适配器 > 共有（冲突时以适配器为准）。
   被适配器覆盖的本库章节：§3 分层红线、§4 骨架展开、§8 验证判定、§11 经验清单、§12 静态扫描。

5. 仅共有能力（未命中或未知技术栈时）
   本库 §1-§12 独立生效，行为同 v3.5.17。零破坏。
```

### §13.1bis 叠加视图速查表（🆕 v3.6.2 — 降低 AI 脑内合并负担）

> **🔴 决策定位：** 测试反馈——§13.1 步骤 4 说"AI 同时读两份叠加"，但**没给合并后视图**，AI 要脑内对位合并共有与适配器，容易遗漏某条特化。本表是**叠加后的"决策查表"骨架**：列出本库哪些章节会被适配器覆盖、合并后怎么查。
>
> **使用方式：** AI 命中适配器后，按下表"合并后查法"一次性定位决策，不用反复在两份文件间跳。具体特化内容由适配器 §9「与共有章节的映射」声明。

| 本库章节 | 共有（始终生效）| 适配器是否覆盖 | 叠加后合并查法（命中适配器时）|
|---------|---------------|--------------|---------------------------|
| §1 CodingModel 决策 | 11 维决策表 + 证据缺失降级 | **补充**（不取消）| 先查共有§1 决策表 + 适配器§1 锁技术栈前提（版本/框架/消息选型）|
| §2 约束文件引用 | 9 项约束抽象清单 | **补充** | 共有§2 列约束 name；适配器§1 给具体 Java+icec 事实 |
| §3 分层职责红线 | 抽象判定口诀 | **覆盖**（冲突以适配器为准）| 适配器§2 落点表 + §2.4 DO/PO 判定线 + §3 特化红线 > 共有§3 口诀 |
| §4 骨架展开规则 | 伪代码动词展开 | **覆盖** | 适配器§4 注解选型/Converter/事务/契约 > 共有§4 通用展开 |
| §5 CodeAnalysis ④bis | 7 章节 + 风险预判 | 不覆盖 | 共有§5 独立（方法论语言无关）|
| §6 ④bis SOP | 分层归类方法论 | 不覆盖 | 共有§6 独立 |
| §7 G-CODEPLAN-SRC | 源码核对判定 | 不覆盖 | 共有§7 独立（含适配器包路径作"同类源码"对照范围）|
| §8 验证判定标准 | 编译/启动/接口/DB/事务 | **覆盖** | 适配器§5 验证姿态（JUnit4/dev-DB/no-root-pom）> 共有§8（如§8.5 H2 被覆盖）|
| §9 漂移核查 + 假修复 | 全切面核查表 + 8 类造假 | 不覆盖 | 共有§9 独立 |
| §10 异常根因 4 层 | 4 层分类 + 修复顺序 | 不覆盖（配合适配器§2.4）| 共有§10 独立；适配器§2.4 给"DO/PO 合法差异 vs 建模错误"辅助层1/4 判定 |
| §11 经验清单 + 红线 | 通用检查清单 | **覆盖** | 适配器§6 命名/错误码 + §8 踩坑库 > 共有§11.1（部分项被取代，见适配器§9）|
| §12 静态扫描 | 通用 grep | **覆盖** | 共有§12 通用扫描 + 适配器§7 工程特化扫描（叠加，都跑）|

> **叠加优先级铁律：** 适配器 > 共有（冲突时以适配器为准）；适配器"不覆盖"的章节（§5/§6/§7/§9/§10/§13 本身）共有独立生效。**未命中适配器**时，上表所有"覆盖/补充"行退化为仅共有，行为同 v3.5.17。

### §13.2 适配器契约（注册进注册表的语言/项目 SKILL 必须满足）

| 契约项 | 要求 |
|--------|------|
| 注册类型 | `type: skill-new`（现有机制，不用 skill-extends 未实现类型）|
| provides key | 形如 `coding-adapter-{lang}`（如 `coding-adapter-java`），本库 §13.1 步骤 2 按此 key 解析 |
| 注册层 | 默认母版 L3（`plugins/registry.yaml`）；项目可 L1、个人可 L2 覆盖 |
| 内容边界 | **承载"编码决策知识层"**（技术栈锁/框架选型决策/包路径落点/验证姿态特化/踩坑决策库），**不复述项目 constraints/ + assets 的纯规则**（指针引用，DRY）|
| 必含章节 | §9「与共有章节的映射」——显式声明叠加覆盖本库哪些 §，供 AI 叠加应用 |

### §13.3 调用方与触发时机

| 调用方 | 触发节点 | 行为 |
|--------|---------|------|
| [`coding-process-skill.md` §A1](coding-process-skill.md) | CodeAnalysis 加载上下文时 | 跑 §13.1 SOP，命中则在 §A2 做分层归类/骨架输出时叠加适配器决策 |
| [`coding-process-skill.md` §B2](coding-process-skill.md) | Execute 按骨架展开代码时 | 叠加适配器对 §4 骨架展开、§8 验证、§12 静态扫描的特化 |

### §13.4 零破坏声明

- 无任何适配器注册（三层全无）→ §13.1 SOP 返回 fallback → 本库 §1-§12 独立生效，与 v3.5.17 行为一致。
- 注册表/适配器加载失败 → 同 plugin_loader 现有降级策略（try/except 返回 None，不阻断主流程）。
- 本库 §1-§12 的通用规则在叠加前后都有效（适配器只"加特化"，不"取消共有"）。

---



| SKILL | 关系 |
|-------|------|
| `coding-process-skill.md` | **调用方**：在 CodeAnalysis 阶段（产出 CodePlan）和 Execute 阶段（写代码）调用本能力库 |
| `be-coding-thinking-engine.md` | **能力来源**：本库的 11 维 CodingModel / 基准过滤器源自 thinking-engine |
| `be-coding-plan-template.md` | **配套模板**：CodePlan 16 节模板 + 15 条门禁自检表 |
| `code-review-skill.md` | **下游**：测试真实性 8 类禁止 / ⑥bis 一致性闸 / ⑦bis 对称性闸 等评审规则 |
| **语言/项目适配器**（如母版 L3 的 `plugins/java3d-coding-skill/SKILL.md`）| **叠加层**：通过 §13 注册加载机制叠加到本库 §3/§4/§8/§11/§12，提供语言/项目特有编码决策。注册机制见 [`ae-sdd-plugin-loader-skill.md`](../cross-cutting/ae-sdd-plugin-loader-skill.md) |
