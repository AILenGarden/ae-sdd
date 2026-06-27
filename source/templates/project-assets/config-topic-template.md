---
name: config-topic-template
description: project-assets config/ 环境配置专题 Starter 模板
---

# {环境或场景} 配置

> **适用范围：** {哪些工程 / 哪些环境}
> **最后更新：** {YYYY-MM-DD}
> **可信度：** {已确认 / 据推断}

---

## 1. 工程信息

| 工程 | 路径 | 用途 |
| --- | --- | --- |
| {工程 A} | `{绝对路径}` | {用途} |
| {工程 B} | `{绝对路径}` | {用途} |

---

## 2. {场景 1（如登录获取 Token）}

**接口：** `{METHOD} {URL}`

**说明：** {描述}

**请求体：**

```json
{}
```

**响应字段：** `{accessToken}` 在 `{Cookie}` 里怎么传

**使用方式：** `{Cookie: security_context=<accessToken>}`

---

## 3. 本地测试前置检查

### 3.1 FeignClient 指定本地 URL

测试时需要给 FeignClient 加 `url` 指向本地服务地址，**测试完成后必须还原**。

```java
@FeignClient(name = "{service-name}", url = "http://localhost:{port}")
public interface {Client} extends {SpiService}
```

### 3.2 management port 不能重复

| 工程 | management.port |
| --- | --- |
| {工程 A} | {port} |
| {工程 B} | {port} |

> 如果工程未配置 `management.port`，Spring Boot 可能使用默认端口；本地多服务同时启动时必须显式指定不同端口。

---

## 4. 测试接口 Base URL

### 4.1 本地服务

| 工程 | Base URL | 端口 |
| --- | --- | --- |
| {工程 A} | `http://localhost:{port}/{context}` | {port} |

### 4.2 测试环境

| 工程 | Base URL |
| --- | --- |
| {工程 A} | `https://{host}/{context}` |

---

## 5. 待确认事项

| ID | 问题 | 影响范围 | 优先级 | 待查 |
|----|------|---------|--------|------|
| C-{NNN} | {问题} | {范围} | {🟠/🟡} | {方法} |
