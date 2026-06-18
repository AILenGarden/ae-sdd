# 技术栈

## 摘要

本文件定义项目使用的技术栈清单。所有新功能必须在此范围内选型，引入新技术需显式记录偏离原因。
适用场景：技术选型、评审 DR 的约束承接章节时。

---

## 一、基础框架

| 技术 | 版本 | 说明 |
| --- | --- | --- |
| Java | 8 | |
| Spring Boot | 1.5.7 | |
| Spring Cloud | Dalston.SR4 | |
| Lombok | - | |
| PageHelper | 5.2.1 | MyBatis 分页，统一使用此插件，禁止手写分页 SQL |
| JUnit | 4.12 | 单元测试框架 |
| Mockito | - | Mock 框架，配合 JUnit 使用 |

## 二、数据存储

| 技术 | 版本 | 说明 |
| --- | --- | --- |
| MySQL | 8.0.17 | 主数据库 |
| MyBatis-Plus | 3.3.2 | ORM 框架 |
| ElasticSearch | 7.10.2 | 搜索与日志查询；客户端使用 `elasticsearch-rest-high-level-client` 7.10.2 + `httpclient` 4.5.13 + `httpcore` 4.4.13 |
| Redis | 随 Spring Boot 1.5.7 | 分布式缓存、分布式锁 |
| Caffeine | - | 本地缓存；本地缓存用 Caffeine，分布式缓存用 Redis，不混用 |

## 三、消息与异步

| 技术 | 版本 | 说明 |
| --- | --- | --- |
| Kafka | - | 异步消息，禁止直接使用，必须通过 courier 组件操作 |
| courier-spring-boot-starter | 3.3-SNAPSHOT | 内部消息投递组件，封装 Kafka |

## 四、云服务

| 技术 | 版本 | 说明 |
| --- | --- | --- |
| 华为云 OBS | 3.23.9 | 对象存储 |
| 华为云短信 SDK | 3.1.120 | 短信发送 |
| 阿里云 SDK | 4.5.0 | 按需使用 |

## 五、内部基础组件

| 技术 | 版本 | 说明 |
| --- | --- | --- |
| icec-cloud-commons | b2c.1.0-SNAPSHOT | 公共工具、异常定义、Result 统一响应模型，所有工程必须引入 |
| icec-cloud-spi-common | b2c.1.0-SNAPSHOT | SPI 公共库，SPI 模块必须引入 |
| icec-cloud-base-webapp | - | BFF 基础 Web 应用框架，所有 BFF 工程必须引入 |
| panda-spring-boot-starter | 1.0.9 | 配置中心客户端，统一配置管理入口 |
| casslog-spring-boot-starter | 1.5.0 | 日志组件，禁止直接使用 logback/log4j 配置 |
| cassmetrics-spring-boot-starter | 1.1.0 | 监控指标上报 |
| job-spring-boot-starter | 4.0.5 | 定时任务，禁止使用 @Scheduled |

## 六、禁止事项

- 禁止跨服务直连数据库
- 禁止用 Redis 做持久化存储
- 禁止绕过 courier 直接操作 Kafka（特殊场景需在 DR 中说明偏离原因）
- 禁止使用 @Scheduled，统一使用 job-spring-boot-starter
- 禁止直接配置 logback/log4j，统一使用 casslog 组件