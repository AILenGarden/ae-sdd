# STORY-{number}-{title}

## 元信息 `必填`

- 文档类型：Story 用户故事
- Story ID：
- 可选来源规格：
- 功能点 ID：
- 优先级：P1 / P2 / P3
- 状态：Draft / Ready / In Progress / Done / Superseded
- 处理人：

> 示例：Story ID: STORY-003-BE，可选来源规格：设计记录 DR-001-02，优先级: P1

---

## 用户故事 `必填`

作为 `{角色}`，我希望 `{完成某个动作或达成某个目标}`，以便 `{获得明确业务价值}`。

> 示例：作为 `运营人员`，我希望 `修改用户账号状态`，以便 `快速停用违规账号或恢复正常账号`。

---

## 业务价值 `选填`

-

> 示例：运营人员可以实时响应违规行为，无需走审批流程即可停用问题账号。

---

## 范围 `必填`

### 包含

-

### 不包含

-

> 示例：
> 包含：修改单个用户状态（启用 / 停用）
> 不包含：批量修改状态、状态变更后的消息通知

---

## 前置条件 `选填（有特殊前置时填写）`

- 数据前置：
- 权限前置：
- 系统前置：

> 示例：
> 数据前置：目标用户已存在
> 权限前置：操作人具有 `user:status:edit` 权限
> 系统前置：无

---

## 涉及工程 `必填`

| 工程 | 父工程 | 类型                      | 变更说明 |
| ---- | ------ | ------------------------- | -------- |
|      |        | BFF / SPI / Service / API |          |

> 工程、父工程和模块注册状态必须以 life 仓库实际证据为准；不要凭模板示例创建模块。
>
> 示例：
>
> | 工程                     | 父工程               | 类型    | 变更说明                                                     |
> | ------------------------ | -------------------- | ------- | ------------------------------------------------------------ |
> | icec-cloud-life-im-bff   | —                    | BFF     | 新增 `PUT /user/{id}/status` Controller 及参数校验，实现对外 API 接口 |
> | icec-cloud-life-im       | —                    | Service | 实现 AppService 编排、DomainService 校验、Repository 更新    |
> | icec-cloud-life-user-spi | icec-cloud-life-spi  | SPI     | 新增 `UpdateUserStatusRequest` / `UserDTO` 及 `UserService` 接口方法 |
> | icec-cloud-life-user-api | icec-cloud-life-api  | API     | 新增对外接口定义（APP 侧）                                   |
> | icec-cloud-boss-user-api | icec-cloud-boss-api  | API     | 新增对外接口定义（Boss 侧）                                  |

---

## 触发入口 `必填（填写适用项，不适用的留空）`

- REST 接口（`/api/{module}/v1/...`）：
- SPI 接口（服务间调用）：
- 消息事件（Kafka topic）：
- 后台任务（job-spring-boot-starter）：
- 外部系统：

> 示例：
> REST 接口：`PUT /user/{id}/status`
> SPI 接口：`UserService.updateUserStatus`
> 消息事件：无
> 后台任务：无

---

## 主流程 `必填`

> 只描述正常路径，假设所有前置条件满足、所有校验通过。异常场景在下方"异常流程"中描述。

1.
2.
3.

> 示例：
> 1. 运营人员提交用户 ID 和目标状态
> 2. 校验用户是否存在
> 3. 校验目标状态与当前状态不同
> 4. 更新用户状态及 last_updated_date
> 5. 返回更新后的用户信息

---

## 异常流程 `必填`

| 场景 | 触发条件 | 系统行为 | 用户提示 / 日志 | AC ID |
| --- | --- | --- | --- | --- |
|  |  |  |  |  |

> 示例：
>

> | 场景 | 触发条件 | 系统行为 | 用户提示 / 日志 | AC ID |
> | --- | --- | --- | --- | --- |
> | 用户不存在 | userId 对应用户不存在 | 抛出业务异常 | "用户不存在" | AC-002 |
> | 状态未变更 | 目标状态与当前状态相同 | 抛出业务异常 | "用户状态未发生变化" | AC-003 |

---

## 接口契约 `必填`

### REST 接口（BFF 层）

**接口：** `{HTTP Method} /{resource}`

**Request**

| 字段 | 类型 | 必填 | 校验规则 | 说明 |
| --- | --- | --- | --- | --- |
|  |  | 是 / 否 |  |  |

**VO**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
|  |  |  |

> 示例：
> 接口：`PUT /user/{id}/status`
>
> Request
>

> | 字段 | 类型 | 必填 | 校验规则 | 说明 |
> | --- | --- | --- | --- | --- |
> | id | Long | 是 | > 0 | 用户 ID（路径参数） |
> | status | String | 是 | 枚举：ACTIVE / INACTIVE | 目标状态 |

>
> VO
>

> | 字段 | 类型 | 说明 |
> | --- | --- | --- |
> | id | Long | 用户 ID |
> | status | String | 更新后状态 |
> | lastUpdatedDate | LocalDateTime | 最后更新时间 |

---

### SPI 接口（Service 层）

**接口：** `XxxDTO methodName(XxxRequest request)`

**Request**

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
|  |  | 是 / 否 |  |

**DTO**

| 字段 | 类型 | 说明 |
| --- | --- | --- |
|  |  |  |

> 示例：
> 接口：`UserDTO updateUserStatus(UpdateUserStatusRequest request)`
>
> Request
>

> | 字段 | 类型 | 必填 | 说明 |
> | --- | --- | --- | --- |
> | userId | Long | 是 | 用户 ID |
> | status | String | 是 | 目标状态 |

>
> DTO
>

> | 字段 | 类型 | 说明 |
> | --- | --- | --- |
> | id | Long | 用户 ID |
> | status | String | 更新后状态 |
> | lastUpdatedDate | LocalDateTime | 最后更新时间 |

---

## 错误码 `必填（有业务错误时）`

| 错误码 | 错误信息 | 触发场景 |
| --- | --- | --- |
|  |  |  |

> 示例：
>

> | 错误码 | 错误信息 | 触发场景 |
> | --- | --- | --- |
> | 11001 | 用户不存在 | userId 对应用户不存在 |
> | 11002 | 用户状态未发生变化 | 目标状态与当前状态相同 |

---

## 数据模型 `选填（有数据库变更时填写）`

> 表操作须遵守：软删除（delete_flag）、更新记录必须同时更新 last_updated_date

### 表变更

| 表名 | 变更类型 | 说明 |
| --- | --- | --- |
|  | 新增 / 修改 |  |

### 字段变更

**`{表名}`**

| 字段名 | 类型 | 约束 | 说明 |
| --- | --- | --- | --- |
|  |  | NOT NULL / DEFAULT ... |  |

### 索引变更

| 表名 | 索引名 | 类型 | 字段 | 说明 |
| --- | --- | --- | --- | --- |
|  |  | 普通 / 唯一 |  |  |

### DDL

```sql
-- 在此粘贴建表或变更 SQL
```

> 示例：
>
> 表变更
>

> | 表名 | 变更类型 | 说明 |
> | --- | --- | --- |
> | boss_user | 修改 | 新增 status 字段 |

>
> 字段变更 `boss_user`
>

> | 字段名 | 类型 | 约束 | 说明 |
> | --- | --- | --- | --- |
> | status | varchar(20) | NOT NULL DEFAULT 'ACTIVE' | 用户状态：ACTIVE / INACTIVE |

>
> 索引变更：无
>
> DDL
>
> ```sql
> ALTER TABLE boss_user
>     ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE' COMMENT '用户状态：ACTIVE / INACTIVE';
>
> CREATE INDEX idx_boss_user_status ON boss_user (status);
> ```

---

## 配置变更 `选填（有配置新增或修改时填写）`

### bootstrap.yml / application.yml

| 配置项 | 所属工程 | 变更类型 | 说明 |
| --- | --- | --- | --- |
|  |  | 新增 / 修改 |  |

### Nacos 配置

| 配置项 | DataId | 变更类型 | 说明 |
| --- | --- | --- | --- |
|  |  | 新增 / 修改 |  |

> 示例：
>
> bootstrap.yml / application.yml
>

> | 配置项 | 所属工程 | 变更类型 | 说明 |
> | --- | --- | --- | --- |
> | feign.client.user.url | icec-cloud-life-im-bff | 新增 | user 服务 Feign 地址 |

>
> Nacos 配置
>

> | 配置项 | DataId | 变更类型 | 说明 |
> | --- | --- | --- | --- |
> | user.status.cache.ttl | icec-cloud-life-im.yaml | 新增 | 用户状态缓存 TTL，默认 300s |

---

## 验收标准 `必填`

| AC ID | Given | When | Then | 测试层级 |
| --- | --- | --- | --- | --- |
| AC-001 |  |  |  | 单元 / 接口 / 集成 |

> **AC 最小覆盖维度（编写时逐项检查）：**
>
> 1. **正向流程** — 主路径走通，输出正确（每个主流程至少一条）
> 2. **业务规则拒绝** — 每条异常流程中的业务规则被违反时的拒绝行为
> 3. **入参边界** — 必填字段缺失、格式非法、长度越界等校验失败场景
> 4. **异常容错** — Story 中声明"失败不阻塞主流程"的依赖，需验证其失败时主流程仍正常返回
> 5. **幂等 / 并发**（如涉及）— 重复请求的预期行为、并发竞争的正确性保证

> 示例：
>

> | AC ID | Given | When | Then | 测试层级 |
> | --- | --- | --- | --- | --- |
> | AC-001 | 用户存在且当前状态为 ACTIVE | 提交 status=INACTIVE | 返回 200，用户状态变为 INACTIVE | 接口 |
> | AC-002 | userId 对应用户不存在 | 提交任意 status | 返回错误码 11001 | 接口 |
> | AC-003 | 用户当前状态为 ACTIVE | 提交 status=ACTIVE | 返回错误码 11002 | 接口 |

---

## 用例设计映射 `必填（可选链接，不构成前置依赖）`

可选地链接 TestCase 的覆盖矩阵；TestCase 文档是独立规格，不是本 Story 的前置条件。

格式：`详见 [TC 文档名](相对路径) 覆盖矩阵章节。`

> 示例：
> 详见 [2c-im-testcase-003-BE.md](../../../design/testcase/be/2c-im-testcase-003-BE.md) 覆盖矩阵章节。

---

## 实现任务映射 `必填（内容清单，不构成 CodingPlan 依赖）`

| Task | 说明 | 涉及工程 / 层 | 状态 |
| --- | --- | --- | --- |
|  |  | {工程名} / BFF · SPI · Service · Domain | Planned |

> 示例：
>

> | Task | 说明 | 涉及工程 / 层 | 状态 |
> | --- | --- | --- | --- |
> | 实现 updateUserStatus SPI 接口 | AppService 编排，DomainService 校验规则，Repository 更新 | icec-cloud-life-cs / Service · Domain | Planned |
> | 实现 PUT /user/{id}/status REST 接口 | Controller 参数校验，AppService 调用 Facade，Facade 调用 Feign | icec-cloud-boss-bff / BFF | Planned |

---

## 偏离声明 `选填（无偏离时可省略）`

> 只写偏离 constraints 默认行为的特殊情况。
> 默认行为：需要鉴权、AppService 层开启本地事务、禁止在事务中调用 Feign。

-

> 示例：
> - 鉴权：此接口加 `@SkipAuth`，无需登录即可访问（公开查询接口）
> - 事务：涉及跨服务数据一致性，需使用分布式事务（Seata AT 模式）

---

## 第三方服务 `选填（有调用外部/第三方服务时填写）`

| 服务名称 | 调用方式 | 开发文档地址 | 负责人 / 联系方式 |
| --- | --- | --- | --- |
|  | REST / SDK / MQ |  |  |

> 示例：
>

> | 服务名称 | 调用方式 | 开发文档地址 | 负责人 / 联系方式 |
> | --- | --- | --- | --- |
> | 极光推送 | REST | https://docs.jiguang.cn/jpush/server/push/rest_api_v3_push | @李四 |
> | 融云 IM  | SDK      | https://docs.rongcloud.cn/platform-chat-api/server-sdk     |                   |

>
> SDK 坐标：`cn.rongcloud.im:server-sdk-java:4.0.2`（[GitHub](https://github.com/rongcloud/server-sdk-java)）

---

## 协作提示与风险 `选填`

- 协作方或外部约束（如有）：
- 受影响的行为或模块：
- 风险：
- 缓解方式：

> 示例：
> 协作方或外部约束：无
> 受影响的行为或模块：用户状态变为 INACTIVE 后，该用户的登录请求将被拒绝
> 风险：并发场景下可能出现重复更新，需确认幂等性要求

---

## 未决问题 `选填`

| 问题 | 负责人 | 截止时间 | 状态 |
| --- | --- | --- | --- |
|  |  |  | Open |

> 示例：停用用户时是否需要同步踢出已登录的 Token？影响：需调用认证服务。负责人：@张三
