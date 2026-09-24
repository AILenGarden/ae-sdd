# ddd — DDD 分层架构适配器（coding/review 能力）

> **定位：** 补充 `core/coding-core.md` 与 `core/review-core.md` 的 DDD 具体规则。只定义领域驱动设计的层职责、依赖方向、边界对象和评审判据；不写语言语法、构建命令、具体客户端、项目包根或版本事实。规则冲突按适用范围、来源和项目约束记录裁决。

## §3 分层职责红线（DDD 实例化）

**Domain 写领域逻辑，Application 写业务编排，Repository 只做数据存取。**

| Core D4 职责语义 | DDD 归属层 | 落点 |
|---|---|---|
| 业务规则 / 能不能 / 算什么（状态流转、不变量、金额计算） | **Domain** | 实体充血方法 / DomainService |
| 先做 A 再做 B / 协调谁调谁 / 事务从哪到哪 / 转换边界对象 | **Application** | 应用服务 |
| 数据存取 / 持久化对象转换 / 拼查询条件 | **Repository**（实现于 Infrastructure） | Repository 实现 |
| 参数格式校验 / 协议适配 | **Interfaces** | 入口适配实现 |

**🔴 各层绝对禁止：**

- **Repository**：状态流转判断、业务规则校验、跨聚合编排、存取方法里塞业务 if 分支。方法名只能表达存取语义。
- **Application**：写领域规则、写持久化细节；只能协调用例、事务和跨边界调用。
- **Domain**：串多个外部服务编排、出现持久化对象/传输对象/具体框架实现。

## §5 CodePlan 章节 1 模板：DDD 文件级实现顺序

| # | 文件路径（包前缀由项目插件给） | 类型 | 依赖 | 完成后验证（命令见语言插件） |
|---|---------|------|------------------|-------------------|
| 1 | `domain/entity/XxxAggregate` | 新增 | — | 按语言适配器编译领域组件 |
| 2 | `domain/repository/XxxRepository` | 新增 | #1 | 按语言适配器编译领域组件 |
| 3 | `infrastructure/persistence/XxxMapper` | 新增 | #1 | 按语言适配器编译基础设施组件 |
| 4 | `infrastructure/persistence/XxxRepositoryImpl` | 新增 | #2, #3 | 按语言适配器编译基础设施组件 |
| 5 | `application/XxxAppService` | 新增 | #1, #2 | 按语言适配器编译应用组件 |
| 6 | `interfaces/XxxController` | 新增 | #5 | 按语言适配器编译入口组件 |
| 7 | `infrastructure/persistence/XxxRepositoryIT` | 新增 | #4 | 按语言适配器执行集成测试 |

> 🔴 顺序原则：**Domain → Infrastructure → Application → Interfaces → Test**（依赖倒置，先被依赖者后依赖者）。具体文件后缀、模块名和验证命令由语言/项目插件确定。

## §6 分层归类判定口诀 + 边缘案例（DDD）

**判定口诀：**

- 业务规则（状态机/不变量/聚合一致性）→ **Domain**
- 协调谁调谁（事务/顺序/跨域）→ **Application**
- 存取数据（find/save/update 等存取语义）→ **Repository / Infrastructure**
- 接协议 / 参数格式校验 → **Interfaces**
- 跨边界能力调用 → **Domain 中的 ACL/端口 + Infrastructure 适配实现**
- 面向外部消费者的聚合 → **入口适配边界**

**边缘案例判定（🔴 必读）：**

- 状态机（业务规则核心）→ **Domain**（领域服务或聚合实体的状态转换行为）
- 跨聚合事务（协调多聚合）→ **Application**（应用服务的事务边界；具体声明方式由语言适配器提供）
- 缓存读（带业务策略如“先查缓存再查数据源”）→ **Application**；缓存读写实现 → **Infrastructure**
- 全局唯一性校验（需查数据源）→ **Domain**（通过领域服务依赖抽象端口）

**判定标准：** 🔴 分层写错（业务规则塞 Repository / 状态机塞 Interfaces / 编排塞 Domain）= 整 Plan 打回。

## §7 DDD 边界补充

| 边界问题 | DDD 归属 | 约束 |
|---|---|---|
| 跨模块能力调用 | Domain 定义 ACL/端口，Infrastructure 提供适配 | Domain 不感知传输协议和具体客户端 |
| 缓存、搜索、消息等技术实现 | Infrastructure | 使用策略仍由 Application/Domain 按职责承载 |
| 外部消息入口 | Interfaces | 入口只适配协议并调用 Application，不直接修改聚合状态 |
| 跨聚合事务与应用事件 | Application | 事务边界由 Application 定义，外部发布方式由语言/项目适配器定义 |
| 领域对象与传输/持久化对象转换 | 对应边界层 | 转换不得反向污染 Domain 模型 |

## review-core 阶段 B 实例化：DDD 四红线

| # | 红线 | 判定 | 证据要求 |
|---|---|---|---|
| B1 | Repository 实现每个方法都是存取语义 | 方法名仅表达存取语义，无业务 if | 实现文件 + 方法清单 |
| B2 | 领域逻辑在 Domain | 状态流转/不变量/业务计算方法归属 | 类:方法 + 本插件 §3 条目 |
| B3 | Application 只做编排 | 事务边界 / 调 Domain 顺序；具体声明方式以语言适配器为准 | 类:方法 + 文件:行号 |
| B4 | Domain 无具体技术对象 | 领域层不依赖持久化对象、传输对象或具体框架实现 | 文件:行号 + 类型/import 清单 |
| B5 | 层间依赖单向 | Interfaces 不反向依赖 Infrastructure；Domain 只依赖抽象 | 模块依赖图 + 反向依赖扫描 |
| B6 | 对象归属正确 | Domain/Application/Interfaces/Infrastructure 对象各在允许边界 | 文件路径、类型和依赖清单 |
| B7 | 事件边界正确 | Domain 事件不直接做外部调用；外部消息经 Interfaces → Application | 事件定义、发布器和调用链 |
