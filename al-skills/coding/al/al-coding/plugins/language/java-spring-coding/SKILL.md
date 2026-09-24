# java-spring — Java + Spring Boot 编码适配器（coding 能力，版本/构建工具矩阵）

> **定位：** 补充 `core/coding-core.md` 的 Java/Spring 具体规则。只写语言和框架特有内容，不复述 Core 方法论、不写架构分层与项目规范（分别归架构/项目插件）。冲突按适用范围、来源和项目约束记录裁决。
>
> **版本边界：** 本插件覆盖 Spring Boot 1.x、2.x、3.x 的共同决策原则，但**不把三代当作同一套可直接执行的命令/API**。调用前必须从 `pom.xml` 或 `build.gradle(.kts)` 解析 `framework_version` 与 `build_tool`；任一未知就暂停并请求确认。项目插件只负责锁定本项目事实，本插件负责按下方矩阵选择对应做法。

## §4 骨架展开规则（Java + Spring 具体展开）

对应 CodingModel 的 D1-D7 代码设计成型步骤与 R01-R11 风险判定，具体 Java/Spring 语法如下：

| 伪代码动词 | Java + Spring 展开 | 示例 |
|---------|---------|------|
| **校验** xxx | 入参使用项目实际 Bean Validation 包中的 `@Valid` / `@NotBlank`（Boot 1.x/2.x 常见 `javax.validation`，Boot 3.x 为 `jakarta.validation`，以依赖和现有代码为准）；业务规则校验调用上游设计定义的领域接口；查不到结果用 `Optional.orElseThrow(...)` 显式处理 | `result = gateway.findById(id).orElseThrow(...)` |
| **查询** xxx | 按上游数据访问契约调用存取抽象；结果 Optional 显式处理，禁止裸调 `.get()` | `gateway.findByXxx(param)` |
| **调用** 外部服务 | 按上游外部依赖表的超时/重试/降级填入；非幂等操作禁止自动重试，必须有幂等键。容错注解随版本选型（⚠ Netflix 栈 `@HystrixCommand` 仅 Boot 1.x/早期 2.x；新栈用 Resilience4j/Sentinel，按项目插件） | Feign Client + fallback |
| **转换** | 使用项目选定的 Converter/映射器（`XxxConverter.toXxx(source)`），禁止在业务调用方手工逐字段 set | `XxxConverter.toResult(source)` |
| **返回** | 遵循项目声明的结果类型；不得返回 null，空集合使用 Java 惯用的空集合 | `return resultOf(value)` |
| **抛异常** | 使用项目声明的业务异常类型；禁止直接抛出裸 `RuntimeException` | `throw new BusinessException(code)` |
| **组装** | 使用当前项目和依赖版本支持的构造方式；先列全必填字段来源 | `Result.builder().field(value).build()` |
| **发送** MQ | 使用当前 Spring/Messaging 依赖提供的消息客户端；事务边界、可靠投递和幂等策略由架构/项目适配器定义 | `messageClient.send(message)` |

## §5 CodePlan 章节 1 / 6 的构建命令（按构建工具选择）

先确认项目实际使用的构建工具；**不得因为识别到 Java/Spring 就默认执行 Maven**。

| 构建工具 | 单模块/增量编译 | 指定测试 | 集成测试 |
|---------|----------------|---------|---------|
| Maven | `mvn -pl {module} -am compile` | `mvn -pl {module} -am test -Dtest={XxxTest}` | `mvn -pl {module} -am verify -Dit.test={XxxIT}`（项目启用 Failsafe 时） |
| Gradle | `./gradlew :{module}:compileJava`（或项目定义的 compile 任务） | `./gradlew :{module}:test --tests '{XxxTest}'` | 仅当项目存在 `integrationTest`/等价任务时执行 `./gradlew :{module}:integrationTest` |

编码收口仍需在**父工程根目录**执行该工具对应的全量构建；多模块项目不能用单模块结果冒充全量验证。

## §8 验证判定标准（Java + Spring Boot 版本/工具矩阵）

### §8.0 先决条件：解析版本与构建工具

| 事实 | 解析来源 | 解析失败时 |
|------|---------|-----------|
| Spring Boot 版本 | Maven parent/BOM/property，或 Gradle plugin/dependency management/property | 标记 `{待确认: Spring Boot 版本}`，禁止继续选择版本敏感 API |
| 构建工具 | `pom.xml`、`mvnw`、`build.gradle`、`build.gradle.kts`、`gradlew` | 标记 `{待确认: 构建工具}`，禁止执行 Maven/Gradle 猜测命令 |
| 管理端点路径与暴露范围 | `application*.yml/properties`、安全配置、Actuator 依赖 | 以项目实际配置为准；没有 Actuator 时改用应用上下文/Smoke Test，不伪造端点证据 |

### §8.1 Spring Boot 版本矩阵

| Boot 代际 | 命名空间/版本敏感点 | 健康与 Bean 验证 |
|----------|---------------------|------------------|
| 1.x（含 1.5） | 可能使用 `javax.*`；管理端点常见为 `/health`、`/beans`，也可能受 `management.context-path` 等配置影响 | 读取项目配置后调用实际健康端点；`/beans` 只有在项目启用并暴露时才可用，否则用 `ApplicationContext` 测试验证 Bean |
| 2.x | 可能使用 `javax.*`；Web 管理端点通常以 `/actuator` 为前缀，但暴露由 `management.endpoints.web.exposure.include` 决定 | 健康端点通常为配置后的 `/actuator/health`；`/actuator/beans` **不是默认必有**，仅在显式暴露时调用，否则用上下文测试 |
| 3.x | 使用 `jakarta.*`；同样受 management base path、exposure 与安全配置约束 | 健康端点通常为配置后的 `/actuator/health`；Bean 验证优先使用上下文测试，只有显式暴露 `/actuator/beans` 才做 HTTP 检查 |

**统一通过条件：** 不是“固定 URL 返回 200”，而是“按项目配置解析出的健康检查可达且状态正常；新增 Bean 通过上下文或已明确暴露的 Actuator 证据确认；启动日志无致命异常”。

### §8.2 编译验证

> 必须在**父工程根目录**执行，不允许只编译子模块（漏跨模块依赖问题）。

```text
Maven:  cd {parent-project-root} && mvn compile
Gradle: cd {parent-project-root} && ./gradlew build（或项目定义的全量验证任务）
```
**通过：** 所选工具的全量任务成功，无 error，所有受影响模块及其依赖全部通过。

### §8.3 服务启动验证

```text
Maven:  cd {parent-project-root} && mvn spring-boot:run
Gradle: cd {parent-project-root} && ./gradlew bootRun
```
**通过标准（三项全部满足）：**
- 按 §8.1 和项目 management 配置解析出的健康检查可达并返回正常状态；不能把 `/actuator/health` 当作所有版本/项目的固定路径。
- 新增 Bean 通过 `ApplicationContext` 测试，或通过项目明确暴露且已鉴权的 Actuator Bean 端点确认；`/actuator/beans` 不作为跨版本硬门槛。
- 启动日志含应用启动完成信号，无 `BeanCreationException` 等致命异常。

**启动失败处理（必须定位根因，禁止绕过）：**

| 失败现象 | 正确处理 | 禁止做法 |
|---------|---------|---------|
| `Port already in use` | 读启动日志查占用进程、修复配置 | ❌ 直接 kill 占端口进程 |
| `BeanCreationException` | 检查 @Autowired 依赖、Bean 扫描路径 | ❌ 注释掉报错 Bean |
| `DataSource connection failed` | 检查数据库连接配置 | ❌ 改用内存 DB |
| `BeanNotOfRequiredTypeException` | 检查接口实现类匹配 | ❌ 强制类型转换 |

> 启动失败属 🔴 阻断型，必须修复后重新验证。

### §8.4 主流程接口测试

> 🔴 能走真实 HTTP 的必须走真实 HTTP：默认 `@SpringBootTest(webEnvironment = RANDOM_PORT)` + `TestRestTemplate`；MockMvc 仅在框架过老时降级并注明原因。

```text
Maven:  cd {service-root} && mvn test -Dtest=*ApiIT,*ControllerIT
Gradle: cd {service-root} && ./gradlew test --tests '*ApiIT' --tests '*ControllerIT'
```
**通过：** 真实 HTTP Pass + HTTP 200 + 响应结构与契约一致。

### §8.5 错误码映射验证

| 场景 | 预期结果 |
|------|---------|
| 参数为空 | HTTP 400 + 契约错误码 |
| 状态非法流转 | HTTP 400 + 契约错误码 |
| 未登录访问 | HTTP 401 或契约错误码 |
| 服务内部异常 | HTTP 500 或兜底错误码 |

### §8.6 DB 写操作落库验证

```text
Maven:  cd {service-root} && mvn test -Dtest=*IntegrationTest
Gradle: cd {service-root} && ./gradlew test --tests '*IntegrationTest'
```
**验证点：** 按项目持久化适配器声明的真实数据源验证写入、更新、删除和事务可见性；核心路径必须有真实数据源证据，全 Mock 只能作为单元测试，不能替代落库验证。

### §8.7 事务边界验证

**验证点：** 事务内失败 → DB 无污染；事务外操作不在事务内；`@Transactional` 调用链与设计一致。

## §11 Java/Spring 经验检查清单（叠加于 Core 的代码设计成型与完成判定，每次生成代码前逐项确认）

| # | 检查项 | 说明 |
|---|--------|------|
| 1 | 构建依赖是否被注释 | 新工程模板中直接依赖可能被注释，需按实际构建图确认 |
| 2 | lombok 显式声明 | scope=provided 不传递，每个模块单独声明 |
| 3 | 第三方 SDK 实际包路径 | 从 jar 解压确认，不凭记忆 |
| 4 | 校验注解来源包 | ⚠ 随 Boot 版本变化（Boot2=javax.*，Boot3=jakarta.*），从当前项目确认 |
| 5 | 结果与异常类型 | 与项目声明的返回/异常契约匹配，不自行发明公共类型 |
| 6 | 字段类型与上游契约一致 | 以需求、接口或项目约束中的类型为准 |
| 7 | 新模块注册到构建图 | Maven/Gradle 的模块声明与依赖关系完整 |
| 8 | 注解与客户端版本 | 按当前 Spring/依赖版本确认注解包与客户端 API |
| 9 | 工具类返回类型 | 以现有项目接口为准，避免盲目类型转换 |
| 10 | 事务提交后动作 | 优先使用 `@TransactionalEventListener(phase = TransactionPhase.AFTER_COMMIT)`；需要编程式注册时使用 `TransactionSynchronizationManager.registerSynchronization(...)` 的 `afterCommit()` 回调 |

> 版本、返回/异常类型、模块结构和事务约束等项目事实，以项目插件/约束文档为准。

## §12 静态扫描（Java 具体 grep，编码后必跑）

```bash
# 1. 标准库全限定名扫描（除 import 块外不应出现）
grep -rn "^[^/*].*\bjava\.\(util\|sql\|io\|time\|math\|net\)\.\w" \
  --include="*.java" src/main/java/ \
  | grep -v ":import " | grep -v ":package "
# 期望输出为空；非空 → 修改为已 import 的短名

# 2. 未使用 import 扫描（IDE 自动化即可）
#    IntelliJ: Code → Optimize Imports
#    Eclipse: Source → Organize Imports

# 3. 静态导入滥用扫描
grep -rn "^import static " --include="*.java" src/main/java/ | wc -l
# 有节制使用，不应过多
```

**判定规则见 CodingModel 的设计完成判定与验证要求**（任一命中 → 修复 → 重跑全部扫描）。
