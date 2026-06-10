# 分层架构规范

## 摘要

本文件定义系统的分层架构和各层职责边界，规定各层能做什么、不能做什么。
适用场景：设计跨层调用、评审模块职责划分时。

---

## 一、整体架构

```
用户侧（2C）                        管理侧（Boss）
cass-webagent（网关）               boss-gateway（网关）
        ↓                                  ↓
life-xxx-bff（用户侧 BFF）          boss-xxx-bff（管理侧 BFF）
        ↓                                  ↓
        └──────────── Service 层 ──────────┘
                     （两侧共用）
                           ↓
                        基础设施层
                  （MySQL / Redis / Kafka / ES）
```

- 2C 和 Boss 分别有独立网关，账号体系不同
- BFF 层按侧区分：`life-xxx-bff`（用户侧）、`boss-xxx-bff`（管理侧）
- Service 层和基础设施层两侧共用

---

## 二、各层职责

### 网关层

**负责：**
- 路由转发
- 统一鉴权入口
- 限流、熔断

**不负责：**
- 任何业务逻辑
- 数据库操作

---

### BFF 层

**负责：**
- 聚合多个 Service 的调用，编排业务流程
- 数据转换（将 Service 返回的数据转换为前端所需格式）
- 操作日志记录
- 异常统一处理

**不负责：**
- 核心业务规则（必须下沉到 Service，BFF 只做数据聚合和格式转换）
- 直接操作数据库
- 直接操作 Redis、Kafka 等中间件

---

### Service 层

Service 层采用 DDD 四层模块结构（Domain / Application / Interfaces / Infrastructure），详见 `project-structure.md`。

> 🔴 **Service 内部分层职责红线（架构腐化高发区）：Domain 写领域逻辑，Application 写业务编排，Repository 只做数据存取，三者不可串味。** 完整的"必须做/禁止做"清单、判定口诀与正反例见 `project-structure.md` 的「分层职责红线」节。每次设计/编码/评审都必须核对。

**负责：**
- 核心业务逻辑
- 数据持久化
- 领域事件发布

**不负责：**
- 直接对前端暴露接口（必须经过 BFF）
- 跨 Service 直连数据库

**Service 间通信：**
- 同步调用：通过 Feign + SPI 接口；Service 可直接调用其他 Service 的 SPI，不需要经过 BFF；超时、重试、熔断由统一的 Feign 配置管理，不在业务代码中手写
- 异步通信：通过领域事件或集成事件，均通过 Kafka（courier 组件）投递
  - 领域事件：同一 Service 内部聚合间解耦，由 domain 层产生，application 层订阅
  - 集成事件：跨 Service 异步通信，由 application 层产生，其他 Service 的 interfaces 层订阅

---

### SPI 层

SPI 不是运行时的层，是 Service 对外暴露的接口契约，详见 `project-structure.md`。

---

### 基础设施层

MySQL、Redis、Kafka、ElasticSearch 等，只允许 Service 层访问，详细模块结构见 `project-structure.md`。

**规则：**
- BFF 层不得直接访问任何基础设施
- 禁止跨 Service 直连数据库

---

## 三、禁止事项

- BFF 层禁止直接操作数据库、Redis、Kafka
- BFF 层禁止包含核心业务规则，只允许数据聚合、格式转换、流程编排
- Service 层禁止直接对前端暴露接口，必须经过 BFF
- BFF 调用 Service 必须通过 SPI 接口
- 禁止跨 Service 直连数据库
- Service 间同步调用必须通过对方 SPI，不得依赖对方内部模块
- 🔴 **Repository（仓储）禁止写任何业务逻辑或领域逻辑**：不做状态流转判断、不做业务规则校验、不做跨聚合编排，仓储方法只能是存取语义（findByXxx/save/update），不能是处理业务（handleXxx/processXxx）
- 🔴 **Application（应用层）禁止写领域规则**：状态能否流转、金额怎么算等业务规则必须下沉到 Domain，Application 只做编排（调谁、顺序、事务边界）
- 🔴 **Domain（领域层）禁止写编排和持久化细节**：不串多个外部服务流程、不出现 SQL/PO/DTO