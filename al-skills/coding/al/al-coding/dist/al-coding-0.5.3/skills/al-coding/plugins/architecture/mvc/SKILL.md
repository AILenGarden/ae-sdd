# mvc — MVC 三层架构适配器（coding/review 能力）

> **定位：** 补充 `core/coding-core.md` 与 `core/review-core.md` 的 MVC 具体规则。适用于经典 Controller / Service / Dao(Mapper) 三层结构（含其变体：+Manager、+Domain 子包）。版本 0.1，尚未经实战校准；规则冲突按适用范围、来源和项目约束记录裁决。
>
> **与 DDD 的本质差异：** 三层中"业务编排"与"领域逻辑"通常合并在 **Service**；若项目 Service 内部再分 Domain 子包，则该子包按 DDD 插件的 Domain 红线执行。

> **共置规则：** MVC 的 Service 可以同时承载业务编排与业务规则，但这是两个可区分的语义角色，而不是取消职责边界。编排方法负责顺序、事务与跨组件协调；规则方法负责状态、不变量与计算。具体划分以方法职责、依赖方向和测试边界为准。

## §3 分层职责红线（三层实例化）

**Controller 只做协议适配，Service 写业务，Dao 只做存取。**

| Core D4 职责语义 | 三层归属 | 落点 |
|---|---|---|
| 参数格式校验 / 协议适配 / 边界对象转换 | **Controller** | Controller |
| 业务编排 **+** 业务规则（先做A再做B、能不能、算什么） | **Service** | Service（+其 Domain 子包，若有） |
| 数据存取 / 拼查询条件 / 持久化对象转换 | **Dao / Mapper** | Dao/Mapper |
| （扩展）通用多 Service 编排 / 三方调用封装 | **Manager**（可选层，若项目有） | Manager |

**🔴 各层绝对禁止：**
- **Controller**：写业务规则、写 SQL/持久化调用（跳过 Service 直调 Dao）、跨 Controller 互调
- **Service**：写 SQL 拼接细节（SQL 条件组装下沉 Dao）；同类内公开方法无意义互调套娃
- **Dao**：业务判断 if 分支、状态流转逻辑、跨表业务编排；方法名只能是存取语义（`selectByXxx`/`insert`/`updateXxx`）

## §5 CodePlan 章节 1 模板：三层文件级实现顺序

| # | 文件路径（包前缀由项目插件给） | 类型 | 依赖 | 完成后验证（命令见语言插件） |
|---|---------|------|------------------|-------------------|
| 1 | `entity/Xxx`（或持久化对象） | 新增 | — | 按语言适配器编译 |
| 2 | `dao/XxxMapper` | 新增 | #1 | 按语言适配器编译 |
| 3 | `service/XxxService`（接口，若项目有接口惯例） | 新增 | #2 | 按语言适配器编译 |
| 4 | `service/impl/XxxServiceImpl` | 新增 | #3 | 按语言适配器编译 |
| 5 | `controller/XxxController` | 新增 | #4 | 按语言适配器编译 |
| 6 | `test/XxxServiceTest` + `XxxControllerIT` | 新增 | #4/#5 | 按语言适配器执行测试 |

> 🔴 顺序原则：**Entity → Dao → Service → Controller → Test**（被依赖者先行，任何时刻可编译）

## §6 分层归类判定口诀 + 边缘案例（三层）

**判定口诀：**
- 接协议 / 参数校验 / 边界对象转换 → **Controller**
- 业务规则 + 先做A再做B + 事务边界 → **Service**
- 存取数据 / SQL 条件 → **Dao / Mapper**
- 多个 Service 的公共编排 / 三方 SDK 封装 → **Manager**（若有）
- 跨服务契约 → **外部契约适配边界**

**边缘案例判定（🔴 必读）：**
- 状态机校验 → **Service** 内私有方法或独立 `XxxStateMachine` 辅助类（三层无 Domain 层；若项目有 domain 包则下沉）
- "先查缓存再查 DB" → **Service**（业务策略，不是 Dao 职责；Dao 只提供两个存取方法）
- 批量导入的分批处理 → **Service** 编排循环，Dao 只提供单批 insert
- 参数校验中依赖查数据源的校验（如重名校验）→ **Service**（格式校验与业务校验分开）

**判定标准：** 🔴 分层写错（业务规则塞 Dao / SQL 塞 Service / Controller 直调 Dao）= 整 Plan 打回。

## review-core 阶段 B 实例化：三层四红线

| # | 红线 | 判定 | 证据要求 |
|---|------|------|---------|
| B1 | Dao/Mapper 每个方法都是存取语义 | 方法名仅存取类动词，无业务 if | grep Dao 实现 + 列出每个方法 |
| B2 | 业务规则在 Service（或其 Domain 子包） | 状态流转/计算/不变量方法归属 | 类:方法 + 本插件 §3 条目 |
| B3 | Controller 只做适配 | 无业务逻辑、无 Dao 直调 | 文件:行号 |
| B4 | Service 无查询实现细节 | 查询条件和数据源访问位于 Dao/Mapper | Service 文件扫描 + Dao 方法清单 |
