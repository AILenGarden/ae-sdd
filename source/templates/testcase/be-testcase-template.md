# TC-STORY-{number}-BE：{标题}

<a id="metadata"></a>
<!-- ae-sdd:testcase-section id=metadata layer=secondary -->
## 1. 元信息

| 字段 | 内容 |
| --- | --- |
| 文档类型 | 用例设计 |
| 用例设计 ID | TC-STORY-{number}-BE |
| 来源 PRD | {PRD 文档引用} |
| 来源 DR | {DR 文档引用} |
| 来源 Story | [{story文件名}]({story相对路径}) |
| 覆盖 AC | AC-001, AC-002, ... |
| 作者 | {作者} |
| 状态 | Draft / Ready / Automated / Executed / Superseded |

> 示例：用例设计 ID: TC-STORY-003-BE，来源 PRD: PRD-001，来源 DR: DR-001-02，来源 Story: STORY-003-BE，覆盖 AC: AC-001, AC-002, AC-003

---

> **适用场景：** 后端测试用例设计模板，配合 `testcase-generate.md` 使用。
>
> **精简规则：** 先建立有限风险登记，再按行为等价类选择覆盖 AC 与已准入风险的最小充分用例组合；达到停止条件后不得继续扩张。

---

## 填写声明表

| § | 章节 | 填写义务 | 适用条件 |
| --- | --- | --- | --- |
| 1 | 元信息 | 🔴 必填 | 全部 |
| 1.1 | 能力与场景推导 | 🔴 必填 | 全部 |
| 2 | 覆盖目标 | 🔴 必填 | 全部 |
| 3 | 覆盖矩阵 | 🔴 必填 | 全部 |
| 4 | 有限风险登记与缺陷假设 | 🔴 必填 | 全部 |
| 5 | 测试数据 | 🔴 必填 | 全部；无预置数据时说明原因 |
| 6 | 用例列表 | 🔴 必填 | 全部 |
| 7 | 回归范围 | 🔴 必填 | 全部 |
| 8 | 执行与报告要求 | 🔴 必填 | 全部 |
| 9 | 有界性、风险与未覆盖项 | 🔴 必填 | 全部 |
| 9.2 | 预算例外 | 🟡 选填（条件） | 超出局部用例预算时 |

> 标题保持纯语义；填写义务以本表为准。无预置数据或无可执行回归项时明确填写“无”及原因，不省略对应章节。

---

## 1.1 能力与场景推导

| 字段 | 内容 |
| --- | --- |
| CapabilityModel | command/query/state-machine/batch/async/file/auth/idempotent/concurrent（按契约选择） |
| 前态/后态 | 可达前态、合法后态、禁止后态 |
| 独立观察面 | 公开查询、列表、任务状态、事件、下载或只读持久化观察；说明为何独立 |
| 变化维度/不变量 | 执行后必须变化和必须保持的字段、关系、守恒或单调性 |
| 扰动轴 | field/identity/order/replay/concurrency/time/boundary/dependency-failure |
| 失败机制 | 本场景能检出的具体缺陷 |

> CRUD 场景不是固定清单。只有 CapabilityModel 命中时，才生成 create→read 或 full-update→read 等具体步骤。

---

## 2. 覆盖目标

{一句话描述测试覆盖目标和重点验证内容}

说明本用例设计覆盖哪个用户故事、哪个验收标准、哪个业务风险。

> 示例：覆盖 STORY-003-BE“修改用户状态”的全部验收标准（AC-001 正常更新、AC-002 用户不存在、AC-003 状态未变更），重点验证状态校验逻辑和错误码返回。

---

## 3. 覆盖矩阵

| AC ID | 风险/假设 ID | 用例 ID | 场景 | 测试层级（最低充分层级） | 验证边界 | HTTP 阶段 | 内部 Mock | 独立验证价值 | 自动化方式 | 状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AC-001 | — | TC-001 | 正常更新用户状态 | L2 / Controller集成 | http | local → test-env | false | 真实 HTTP 协议、响应与持久化结果 | RANDOM_PORT + HTTP client | Planned |
| AC-002 | — | TC-002 | 用户不存在 | L1 / 单元 | unit | — | true | 业务错误码和未写入不变量 | JUnit + Mockito | Planned |
| AC-003 | — | TC-003 | 目标状态与当前状态相同 | L1 / 单元 | unit | — | true | 业务规则拒绝和状态不变 | JUnit + Mockito | Planned |
| AC-001 | — | TC-004 | UPDATE SQL 正确性 | L3 / Mapper集成 | integration | — | false | SQL 与数据库约束 | Spring Boot Test + 开发库 | Planned |

> 🔴 接口（L2）AC 固定 `boundary=http`、`stages=[local,test-env]`、`internalMocksAllowed=false`。本地 `RANDOM_PORT + HTTP client` 必须走 Controller→Service→Repository/Mapper→测试 DB，随后以同一 buildId 跑非 loopback 测试环境。MockMvc、直接 Controller 调用、内部 MockBean/SpyBean 都不能关闭接口 AC。

> 基线示例曾使用 MockMvc 表示 Controller 集成测试；按当前 `testing.md`，MockMvc 不能关闭接口 AC。上表保留基线的 AC ID、用例 ID、场景、测试层级、自动化方式和状态语义，并将接口用例升级为真实 HTTP 证据。

---

## 4. 有限风险登记与缺陷假设

> 所有候选先登记再选择。只有 `keep` 候选成为 H-{N} 并要求用例覆盖；`merge` 必须填写“合并至”；`exclude/defer` 必须说明理由。

| 候选/假设 ID | 类型 | 风险等级 | 证据来源 | 行为分区 | 独立失败机制 | 最低充分层级 | 选择决策 | 合并至 / 覆盖用例 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| H-CONC-1 | 并发 | high | Story 写路径 + 通用库 | 乐观锁冲突 | 丢失更新 | L3 | keep | TC-001 |
| C-BND-1 | 边界 | low | 同一 validator | 参数错误 | 无新增机制 | L2 | merge | H-VALID-1 / TC-002 |
| | | | | | | | | |

**类型枚举：** 并发 / 事务 / 边界 / 状态机 / 集成 / 安全 / 时序资源

**证据来源枚举：** AC / 接口契约 / 业务不变量 / 改动分支 / 历史缺陷 / 项目坑:{具体出处} / 高影响风险

**选择决策枚举：** keep / merge / exclude / defer

---

## 5. 测试数据

| 数据项 | 构造方式 | 约束 | 清理方式 |
| --- | --- | --- | --- |
| | INSERT / SQL / 代码构造 / Builder / Mock | | 自动回滚 / @Rollback / 手动清理 |

> 集成测试使用 `@Transactional` + `@Rollback`，数据自动回滚，无需手动清理。

> 示例：
>
> | 数据项 | 构造方式 | 约束 | 清理方式 |
> | --- | --- | --- | --- |
> | 状态为 ACTIVE 的用户 | `INSERT INTO boss_user ...` | `userId=1, status=ACTIVE` | `@Rollback` 自动回滚 |

---

## 6. 用例列表

### TC-001 {场景描述}

- 用例类型：风险证伪 / AC 代表 / 回归防护
- 覆盖的缺陷假设（风险证伪类必填，多对多用逗号分隔）：H-CONC-1, H-TX-1
- 缺陷假设分类：并发 / 事务 / 边界 / 状态机 / 集成 / 安全 / 时序资源
- 风险等级：high / medium / low
- 证据来源：AC / 契约 / 分支 / 历史缺陷 / 项目坑 / 高影响风险
- 行为分区：{控制流 / 错误码 / 存储约束 / 外部协议 / 副作用}
- 独立失败机制：{现有用例无法暴露的缺陷；无则合并或排除}
- 选择决策：keep
- 覆盖 AC：AC-001
- 测试层级：L1 / L2 / L3 / L4（最低充分层级）
- 验证边界：unit / http / integration
- HTTP 阶段（接口必填）：local → test-env
- internalMocksAllowed（接口必填）：false
- 前置条件：
- 场景描述：{用业务语言描述操作}
- 期望行为 / 期望结果：{用业务语言描述期望结果}
- 业务断言 / 断言：
  - {字段} = {期望值}
  - {字段} 不为空
- Test double 配置 / Mock 配置（仅非接口单测或外部 supplemental 故障注入填写）：
  - 外部边界：`when(...).thenReturn(...)`
- 操作步骤：
  1. 
- 自动化入口：`src/test/java/{package}/{TestClass}#{method}`
- 清理动作：无 / @Rollback

> 用例必须同时记录场景描述、期望行为、业务断言、操作步骤、Mock 配置、自动化入口和清理动作；接口用例不得使用内部 Mock 替代 HTTP 边界。

> 示例：TC-001 正常更新用户状态：前置条件为 `userId=1` 且 `status=ACTIVE`，发送 `PUT /user/1/status` 后断言 HTTP 200、`result.code == 200`、状态变为 `INACTIVE`、`lastUpdatedDate` 不为空；自动化入口为 `UserControllerTest#updateUserStatus_success`，清理动作 `@Rollback`。

> 示例（异常单元测试）：
>
> ### TC-002 用户不存在
>
> - 用例类型：AC 代表
> - 覆盖 AC：AC-002
> - 测试层级：L1
> - 验证边界：unit
> - 前置条件：无
> - 场景描述：更新不存在用户的状态
> - 期望行为 / 期望结果：抛出业务异常，错误码 11001
> - 业务断言 / 断言：`exception.code == "11001"` 且消息为“用户不存在”
> - Test double 配置 / Mock 配置：`when(userRepository.findById(99L)).thenReturn(Optional.empty())`
> - 操作步骤：调用 `updateUserStatus(userId=99)`
> - 自动化入口：`src/test/java/{package}/UserAppServiceTest#updateUserStatus_userNotFound`
> - 清理动作：无

---

## 7. 回归范围

> 参照 `testing.md` 中的测试分层策略和覆盖率要求。

- 必跑单元测试：
- 必跑接口测试：
- 必跑本地 HTTP：
- 必跑测试环境 HTTP（同 buildId）：
- 可跳过项及原因：

> 示例：必跑单元测试 `UserAppServiceTest`、`UserDomainServiceTest`；必跑接口测试 `UserControllerTest#updateUserStatus_*`；可跳过 Mapper 集成测试（本 Story 无新增 SQL）。

---

## 8. 执行与报告要求

- 测试执行后必须输出测试报告。
- 测试报告必须列出 `Story ID / AC ID / 用例 ID → 实际测试命令或验证步骤`。
- 若某个 AC 无法自动化，必须说明原因、替代验证方式和剩余风险。

---

## 9. 有界性、风险与未覆盖项

### 9.1 停止条件证据

- AC 已覆盖：是 / 否（缺口：）
- `keep` 风险已映射：是 / 否（缺口：）
- 改动分支与历史回归已覆盖：是 / 否（缺口：）
- 剩余候选是否增加新失败机制、控制流、契约、协议、断言或层级证据：否 / 是（说明：）
- 结论：停止生成 / 继续补充（理由：）

### 9.2 预算例外

| 超出局部上限的候选 | 新增价值 | 执行成本 | 维护成本 | 不可合并原因 | 确认人 |
| --- | --- | --- | --- | --- | --- |
| 无 / {候选 ID} | | | | | |

### 9.3 风险与未覆盖项

| 风险 / 未覆盖项 | 原因 | 替代验证 | 后续处理 |
| --- | --- | --- | --- |
| | | | |

> 示例：
>
> | 风险 / 未覆盖项 | 原因 | 替代验证 | 后续处理 |
> | --- | --- | --- | --- |
> | 并发场景下重复更新未覆盖 | 单元测试难以模拟并发 | 上线后观察日志 | 后续补充并发测试 |
