# API 规范

## 摘要

本文件定义接口设计的团队规范，包括 URL 命名、HTTP 方法使用、请求参数、响应结构、错误码等约束。
适用场景：设计或评审任何对外暴露的 HTTP 接口时。

---

## 一、URL 命名规则

- 使用小写字母，单词间用连字符 `-` 分隔，禁止驼峰和下划线
- 路径使用名词，禁止动词（操作语义由 HTTP 方法表达）
- 资源名使用单数，如 `/user`、`/role`、`/vehicle-failure`
- `/pages`、`/list`、`/options` 等是操作后缀，不受资源名单复数规则约束
- 嵌套资源用路径表达从属关系，如 `/vehicle/{id}/maintenance`
- 无需版本前缀
- 无需认证的公开接口统一加 `/public/` 前缀

**正例：**
```
GET    /user/{id}
POST   /user
PUT    /user/{id}
DELETE /user/{id}
POST   /user/pages
```

**反例：**
```
POST   /getUser
POST   /createUser
GET    /user/delete?id=1
```

---

## 二、HTTP 方法使用规范

| 方法 | 使用场景 |
| --- | --- |
| GET | 查询单个资源、简单条件列表查询、获取选项列表 |
| POST | 创建资源、复杂条件分页查询、批量操作 |
| PUT | 更新资源、修改状态、幂等写入操作 |
| DELETE | 删除资源 |

**说明：**
- 分页查询统一使用 POST + RequestBody，原因：查询条件通常包含多个字段、范围查询、嵌套对象，GET + QueryParam 无法优雅表达，且 URL 长度有限制
- 简单查询（参数少且为基础类型）使用 GET + RequestParam
- 状态变更使用 PUT，不使用 POST
- 幂等写入操作使用 PUT，不使用 POST。判断标准：相同参数重复调用结果一致（如"已存在则忽略"的新增操作），此规则同样适用于 SPI 接口

---

## 三、请求参数规范

| 参数类型 | 使用场景 |
| --- | --- |
| PathVariable | 资源唯一标识，如 `/{id}`、`/{code}` |
| RequestParam | 简单查询参数、可选参数、基础类型参数 |
| RequestBody | 复杂对象、分页查询条件、批量操作入参 |

- RequestBody 对象必须加 `@Valid` 注解触发参数校验

**分页入参：** 统一使用 `PageRequest<T>` 泛型包装，T 为具体的查询条件对象。

```java
@PostMapping("/vehicle/failure/pages")
ApiResult<PagedModels<VehicleFailureListVO>> pages(@RequestBody PageRequest<VehicleFailureListRequest> request);
```

`PageRequest` 字段（`com.casstime.cloud.boss.common.api.model.PageRequest`）：

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| page | Integer | 1 | 当前页码，从 1 开始，不能为空 |
| size | Integer | 10 | 每页条目数，不能为空 |
| condition | T | - | 查询条件对象，按需定义 |

---

## 四、响应结构

各工程统一使用对应的返回值包装类，禁止自定义新的包装类（具体类路径见各工程的 api-common 模块）：

| 工程类型 | 包装类 |
| --- | --- |
| 2C BFF | `ApiResult<T>`（life api-common） |
| Boss BFF | `ApiResult<T>`（boss api-common） |
| Service（调用方需按错误码分支时） | `Result<T>`（icec-cloud-commons） |

**响应结构：**

```json
{
  "code": 200,
  "message": null,
  "data": {}
}
```

**data 类型约定：**

| 场景 | 类型 |
| --- | --- |
| 单个对象 | `ApiResult<XxxVO>` |
| 列表 | `ApiResult<List<XxxVO>>` |
| 分页 | `ApiResult<PagedModels<XxxVO>>` |
| 操作成功无数据 | `ApiResult<Void>` |
| 返回 ID 或简单值 | `ApiResult<String>` |

**分页出参：** 统一使用 `com.casstime.commons.model.PagedModels<T>` 包装分页结果。

- Controller 的返回值必须使用对应包装类
- SPI 接口一般直接返回业务对象或抛异常，不使用 `Result<T>` 包装；当调用方需要根据不同业务错误码做分支判断时，使用 `Result<T>` 包装
- Service / AppService 层内部方法不使用包装类，直接返回业务对象或抛异常

---

## 五、错误码规范

- 成功：`200`
- 通用服务错误：`500`
- 业务错误码：5 位整数，集中定义在常量类中（如 `BizCodes`）
- 错误信息使用中文描述

**【推荐】各模块错误码分段管理**，避免不同模块错误码冲突，示例：

| 模块 | 错误码范围 |
| --- | --- |
| 认证 | 10000 - 10999 |
| 用户 | 11000 - 11999 |
| 车辆 | 12000 - 12999 |
| 工单 | 13000 - 13999 |

> 当前各模块错误码范围尚未统一划分，后续补充时按此规则执行。

---

## 六、接口定义规范

- 接口定义在 `*-bff-api` 模块，实现类在 BFF 工程内
- 接口类命名：`{Resource}Rest`
- 实现类命名：`{Resource}RestImpl`
- 请求对象命名：`{Resource}Request`
- 响应对象命名：`{Resource}VO`
- 所有接口必须加 Swagger 注解：
  - 接口类：`@Api`
  - 方法：`@ApiOperation`
  - 参数：`@ApiImplicitParam`
  - 请求/响应对象：`@ApiModel` + `@ApiModelProperty`

---

## 七、全局异常处理

- 使用 `@RestControllerAdvice` 统一处理异常
- HTTP 状态码统一返回 200，错误信息通过响应体的 `code` 和 `message` 字段表达
- 异常分类处理：
  - 业务异常（`BusinessException`）：`log.warn` 记录，返回业务错误码和信息
  - 参数校验异常（`MethodArgumentNotValidException`）：提取字段错误信息返回
  - 系统异常（`Exception`）：`log.error` 记录完整堆栈，返回通用错误码

---

## 八、其他约定

- 禁止在接口 URL 中暴露内部实现细节（如数据库表名、内部服务名）
- 删除操作优先使用逻辑删除，物理删除需在 DR 中说明原因
- 幂等性要求：
  - PUT、DELETE 必须保证幂等
  - POST 创建接口按需设计幂等
  - 涉及外部系统回调的接口（如支付回调、消息回调）必须做幂等处理，防止重复消费
