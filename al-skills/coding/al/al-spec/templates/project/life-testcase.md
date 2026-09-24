# TC-{STORY-ID}-{title}

## 元信息 `必填`

- 文档类型：用例设计
- 用例设计 ID：
- 可选来源规格：
- 覆盖 AC：
- 作者：
- 状态：Draft / Ready / Automated / Executed / Superseded

> 示例：用例设计 ID: TC-STORY-003-BE，可选来源规格：STORY-003-BE，覆盖 AC: AC-001, AC-002, AC-003

---

## 覆盖目标 `必填`

说明本用例设计覆盖哪个用户故事、哪个验收标准、哪个业务风险。

> 示例：覆盖 STORY-003-BE"修改用户状态"的全部验收标准（AC-001 正常更新、AC-002 用户不存在、AC-003 状态未变更），重点验证状态校验逻辑和错误码返回。

---

## 覆盖矩阵 `必填`

| AC ID | 用例 ID | 场景 | 测试层级 | 自动化方式 | 状态 |
| --- | --- | --- | --- | --- | --- |
| AC-001 | TC-001 | 正常流程 | 单元 / Mapper集成 / Controller集成 |  | Planned |

> 示例：
>

> | AC ID | 用例 ID | 场景 | 测试层级 | 自动化方式 | 状态 |
> | --- | --- | --- | --- | --- | --- |
> | AC-001 | TC-001 | 正常更新用户状态 | Controller集成 | MockMvc | Planned |
> | AC-002 | TC-002 | 用户不存在 | 单元 | JUnit + Mockito | Planned |
> | AC-003 | TC-003 | 目标状态与当前状态相同 | 单元 | JUnit + Mockito | Planned |
> | AC-001 | TC-004 | UPDATE SQL 正确性 | Mapper集成 | Spring Boot Test + 开发库 | Planned |

---

## 测试数据 `选填（有需要预置的数据库数据时填写）`

> 集成测试使用 `@Transactional` + `@Rollback`，数据自动回滚，无需手动清理。

| 数据项 | 构造方式 | 约束 | 清理方式 |
| --- | --- | --- | --- |
|  | SQL / 代码构造 |  | 自动回滚 / 手动清理 |

> 示例：
>

> | 数据项 | 构造方式 | 约束 | 清理方式 |
> | --- | --- | --- | --- |
> | 状态为 ACTIVE 的用户 | INSERT INTO boss_user ... | userId=1, status=ACTIVE | @Rollback 自动回滚 |

---

## 用例列表 `必填`

> 编码前填写：场景描述、期望行为、业务断言。
> 编码后补充：自动化入口（实际类名#方法名）和 Mock 配置。

### TC-001 {场景名称}

- 覆盖 AC：
- 测试层级：单元 / Mapper集成 / Controller集成 / 手工
- 前置条件：（描述业务前置状态，不依赖具体类名）
- 场景描述：（用业务语言描述操作，如"坐席向已结束对话发送消息"）
- 期望行为：（用业务语言描述期望结果，如"新工单创建，状态为 IN_PROGRESS"）
- 业务断言（列出需要校验的业务字段及期望值，不写具体 API）：
  - 字段 X = 期望值
  - 字段 Y 不为空
- 自动化入口（**编码后填写**，`src/test/java/{package}/类名#方法名`）：
- Mock 配置（**编码后填写**，单元测试填写；Mapper/Controller 集成测试填"无"）：
- 清理动作：

---

> 示例（接口测试）：
>
> ### TC-001 正常更新用户状态
>
> - 覆盖 AC：AC-001
> - 测试层级：接口
> - 前置条件：userId=1 的用户存在，当前 status=ACTIVE
> - Mock 配置：无（接口测试不 mock）> - 操作步骤：
>   1. 构造请求 `PUT /user/1/status`，body: `{"status": "INACTIVE"}`
>   2. 发送请求
> - 期望结果：HTTP 200，用户状态变为 INACTIVE
> - 断言：
>   - `result.code == 200`
>   - `result.data.id == 1`
>   - `result.data.status == "INACTIVE"`
>   - `result.data.lastUpdatedDate` 不为空
> - 自动化入口：`src/test/java/com/casstime/cloud/boss/interfaces/UserControllerTest#updateUserStatus_success`
> - 清理动作：@Rollback 自动回滚
>
> ### TC-002 用户不存在
>
> - 覆盖 AC：AC-002
> - 测试层级：单元
> - 前置条件：无
> - Mock 配置：
>   - `when(userRepository.findById(99L)).thenReturn(Optional.empty())`
> - 操作步骤：
>   1. 调用 `userAppService.updateUserStatus(UpdateUserStatusCommand(userId=99, status=INACTIVE))`
> - 期望结果：抛出业务异常，错误码 11001
> - 断言：
>   - 抛出 `BossDomainException`
>   - `exception.code == "11001"`
>   - `exception.message == "用户不存在"`
> - 自动化入口：`src/test/java/com/casstime/cloud/boss/application/UserAppServiceTest#updateUserStatus_userNotFound`
> - 清理动作：无

---

## 回归范围 `选填`

> 测试分层策略和覆盖率要求：

- 必跑单元测试：
- 必跑接口测试：
- 可跳过项及原因：

> 示例：
> 必跑单元测试：`UserAppServiceTest`、`UserDomainServiceTest`
> 必跑接口测试：`UserControllerTest#updateUserStatus_*`
> 可跳过项：Mapper 集成测试（本 Story 无新增 SQL）

---

## 执行与报告要求 `必填`

- 测试执行后必须输出测试报告。
- 测试报告必须列出 `行为 ID / AC ID / 用例 ID → 实际测试命令或验证步骤`。
- 若某个 AC 无法自动化，必须说明原因、替代验证方式和剩余风险。

---

## 风险与未覆盖项 `选填`

| 风险 / 未覆盖项 | 原因 | 替代验证 | 后续处理 |
| --- | --- | --- | --- |
|  |  |  |  |

> 示例：并发场景下重复更新未覆盖 | 单元测试难以模拟并发 | 上线后观察日志 | 后续补充并发测试
