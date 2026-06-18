# TC-STORY-{number}-BE：{标题}

## 元信息

- 文档类型：用例设计
- 用例设计 ID：TC-STORY-{number}-BE
- 来源 Story：[{story文件名}]({story相对路径})
- 覆盖 AC：AC-001, AC-002, ...
- 状态：Draft / Ready / Executed

---

## 覆盖目标

{一句话描述测试覆盖目标和重点验证内容}

---

## 覆盖矩阵

| AC ID | 用例 ID | 场景 | 测试层级 | 自动化方式 | 状态 |
| --- | --- | --- | --- | --- | --- |
| AC-001 | TC-001 | | 单元 / 接口 / 集成 | JUnit + Mockito / SpringBootTest(RANDOM_PORT)+TestRestTemplate / SpringBootTest+H2 | Planned |

> 🔴 接口（L2）测试默认走真实 HTTP（SpringBootTest RANDOM_PORT + TestRestTemplate），MockMvc 仅在框架过老无法启动嵌入式容器时降级，须在备注注明原因。

---

## 测试数据

| 数据项 | 构造方式 | 约束 | 清理方式 |
| --- | --- | --- | --- |
| | INSERT / Builder / Mock | | @Rollback / 手动清理 |

---

## 用例列表

### TC-001 {场景描述}

- 覆盖 AC：AC-001
- 测试层级：单元 / 接口 / 集成
- 前置条件：
- Mock 配置：
  - `when(...).thenReturn(...)`
- 操作步骤：
  1. 
- 期望结果：
- 断言：
  - 
- 自动化入口：`src/test/java/{package}/{TestClass}#{method}`
- 清理动作：无 / @Rollback

---

## 回归范围

- 必跑单元测试：
- 必跑接口测试：
- 可跳过项：

---

## 风险与未覆盖项

| 风险 / 未覆盖项 | 原因 | 替代验证 | 后续处理 |
| --- | --- | --- | --- |
| | | | |
