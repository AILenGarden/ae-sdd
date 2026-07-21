# 测试规范

## 摘要

本文件定义测试分层策略、覆盖率要求、测试数据管理和自动化约束。
适用场景：制定测试策略、编写测试用例、评审测试覆盖时。

---

## 一、测试框架

| 用途 | 框架 / 工具 |
| --- | --- |
| 单元测试 | JUnit 4.12 + Mockito + AssertJ |
| 集成测试 | Spring Boot Test + 开发库（@Transactional + @Rollback） |
| Controller / REST 验收 | 🔴 真实 HTTP：本地 SpringBootTest(RANDOM_PORT) + TestRestTemplate/RestAssured/标准 HTTP client；随后以同一 buildId 请求测试环境 |
| 代码质量 | Checkstyle + SpotBugs |

---

## 二、测试分层策略

```
┌──────────────────────────────┐
│     测试环境接口验收（L4）    │  ← 🔴 同一 buildId 请求非 loopback 测试环境
├──────────────────────────────┤
│     本地接口验收（L2）        │  ← 🔴 RANDOM_PORT + HTTP client，完整内部链
├──────────────────────────────┤
│     Mapper 集成测试           │  ← 开发库，验证 SQL 正确性
├──────────────────────────────┤
│     Service 单元测试          │  ← Mockito，验证业务逻辑
└──────────────────────────────┘
```

| 层级 | 测试类型 | 工具 | 说明 |
| --- | --- | --- | --- |
| Service | 单元测试 | JUnit + Mockito | 纯逻辑，不依赖数据库，Mock 所有外部依赖 |
| Mapper | 集成测试 | Spring Boot Test + 开发库 | 验证自定义 SQL 和 XML 映射正确性，使用 @Transactional + @Rollback 回滚数据 |
| Controller / REST | 接口验收 | 🔴 真实 HTTP：SpringBootTest(RANDOM_PORT) + TestRestTemplate/RestAssured/标准 HTTP client | 本地真实端口走 Controller→Service→Repository/Mapper→测试 DB 完整内部链；禁止内部 @MockBean/@SpyBean/@Mock/@Spy/Mockito.mock；同一 buildId 必须再跑测试环境 |

**单元测试 Mock 规则：**
- Service 单元测试必须 mock 所有外部依赖（Repository、Facade、外部服务调用），禁止在单元测试中真实调用数据库或远程服务
- 每个测试用例必须明确说明 mock 的依赖及其返回值，不同场景的 mock 返回值应在用例中独立配置，禁止跨用例共享 mock 状态
- 单元测试和 test double 只提供快速反馈，不能作为接口 AC 的验收证据；外部系统 sandbox/stub 只能作为 supplemental 故障注入

**接口 executionPlan 合同：**
- 接口 AC 的 verification 必须声明 `boundary=http`、`stages=[local,test-env]`、`internalMocksAllowed=false` 和非空命令
- 两阶段 evidence 必须覆盖同一 AC 集合、使用同一 buildId，且本地阶段完成时间不得晚于测试环境阶段开始时间
- 缺测试环境、buildId 不同、内部 mock、非真实 HTTP URL 或 supplemental 证据冒充任一必需阶段时，接口 AC 保持 BLOCKED

---

## 三、覆盖率要求

| 层级 | 最低覆盖率 | 说明 |
| --- | --- | --- |
| 整体项目 | ≥ 60% | 通过 JaCoCo 统计 |
| Service 核心业务逻辑 | ≥ 70% | |
| Mapper 自定义 SQL | ≥ 60% | 仅针对 XML 中手写的方法，MyBatis-Plus 自动生成的不计入 |
| Controller 接口 | ≥ 50% | 核心接口必须覆盖正常路径 + 主要异常路径 |

**断言要求：**
- 禁止只断言 HTTP 状态码（如只检查 200），必须校验响应体中的业务字段
- 异常路径必须断言错误码和错误信息
- 断言必须覆盖 Story 接口契约中定义的关键业务字段，不能只断言操作成功
- 正例：
  ```java
  // ❌ 反例
  assertThat(result.getStatusCode()).isEqualTo(200);

  // ✅ 正例
  assertThat(result.getCode()).isEqualTo(200);
  assertThat(result.getData().getId()).isNotNull();
  assertThat(result.getData().getStatus()).isEqualTo("ACTIVE");
  ```

---

## 四、测试数据管理

- 集成测试：在开发库上运行，使用 `@Transactional` + `@Rollback` 保证测试数据自动回滚，不污染数据库
- 禁止测试用例之间共享可变状态

---

## 五、测试目录约定

```
src/test/java/
├── {package}/interfaces/     # Controller 集成测试，命名：*ControllerTest.java
├── {package}/application/    # Service 单元测试，命名：*AppServiceTest.java
└── {package}/infrastructure/ # Mapper 集成测试，命名：*MapperTest.java

src/test/resources/
└── test-data/
    ├── init-*.sql            # 测试数据初始化
    └── cleanup-*.sql         # 测试数据清理
```

---

## 六、静态分析

- **Checkstyle**：代码风格检查（命名、缩进、括号等），CI 中 P0 问题阻断构建
- **SpotBugs**：静态分析（NPE、资源泄漏、逻辑错误等），CI 中 P0 问题阻断构建

---

## 七、验收流程

```
编码完成
    ↓
Checkstyle / SpotBugs 静态检查
    ↓
Service 单元测试（JUnit + Mockito）
    ↓
Mapper 集成测试（开发库）
    ↓
Controller 集成测试（真实 HTTP：RANDOM_PORT + TestRestTemplate）
    ↓
同一 buildId 部署并请求测试环境 HTTP
    ↓
覆盖率达标检查
    ↓
✅ 验收通过
```

---

## 八、禁止事项

- 🔴 禁止用 MockMvc、@WebMvcTest、application-context/controller-bound WebTestClient 或直接 Controller 调用关闭接口 AC
- 🔴 禁止在真实端口接口验收中用 @MockBean/@SpyBean/@Mock/@Spy/Mockito.mock 替换内部 Service、Repository、Mapper、DAO、UseCase、Application 或 Controller
- 🔴 禁止只有本地 HTTP 结果就关闭接口 AC；必须以同一 buildId 完成测试环境 HTTP 阶段
- 🔴 禁止用 external supplemental stub/sandbox evidence 代替 `http-local` 或 `http-test-env`
- 🔴 禁止用全 Mock 替代核心落库路径验证——INSERT/UPDATE/DELETE 核心路径必须用真实 DB（H2/TestContainers）验证落库
- 禁止使用 `Thread.sleep()` 等待异步结果，使用 `Awaitility` 或 Mock 替代
- 禁止在测试中使用生产数据库
- 禁止测试方法名使用无意义命名（如 `test1()`、`testA()`）
- 禁止空 catch 块吞掉测试异常

---

## 九、跨 Story 集成测试

当 Story 声明了系统前置依赖（`系统前置：STORY-XXX-BE 已完成`），且前置 Story 均为 Done 时，当前 Story 完成者须编写跨 Story 集成测试。

- 验证范围：当前 Story 与直接前置 Story 之间的数据消费和接口调用链路
- 实现方式：`@SpringBootTest` 启动完整上下文，仅 Mock 外部系统，不 Mock 内部 Service
- 数据管理：`@Transactional` + `@Rollback`，通过 SQL 预置上游数据
- 目录约定：`src/test/java/{package}/integration/*IntegrationTest.java`
- 豁免：前置 Story 仅提供基础数据表且 Mapper 集成测试已验证读取正确性时可豁免，需在 TestCase 文档中说明
