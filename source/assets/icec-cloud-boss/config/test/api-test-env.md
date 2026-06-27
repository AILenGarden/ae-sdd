---
name: api-test-env
description: 5 个 BFF/Service 工程的本地 + 测试环境配置（登录/Token/Base URL/management port）
---

# 接口集成测试环境配置

> **适用范围：** 5 个 BFF/Service 工程（icec-cloud-boss-auth-bff / icec-cloud-boss-user / icec-cloud-boss-agent-workbench-bff / icec-cloud-life-cs / icec-cloud-life-user / icec-cloud-life-im）
> **最后更新：** 2026-06-27
> **可信度：** 已确认

> **🆕 v3.5.1.1 来源说明**：本专题由同事知识库 `D:\Item\life\document\life-team-project-docs\knowledge\config\test\api-test-env.md`（2989 字节）迁入并按 `config-topic-template.md` 模板格式化。

---

## 1. 工程信息

| 工程 | 路径 | 用途 |
| --- | --- | --- |
| icec-cloud-boss-auth-bff | `e:\softWare\Project\boss\icec-cloud-boss-auth-bff` | 登录认证 BFF |
| icec-cloud-boss-user | `e:\softWare\Project\boss\icec-cloud-boss-user` | 用户服务 |
| icec-cloud-boss-agent-workbench-bff | `e:\softWare\Project\boss\icec-cloud-boss-agent-workbench-bff` | 客服工作台 BFF |
| icec-cloud-life-cs | `e:\softWare\Project\life\icec-cloud-life-cs` | CS 业务服务 |
| icec-cloud-life-user | `e:\softWare\Project\life\icec-cloud-life-user` | 用户服务（C 端）|

---

## 2. 登录获取 Token

**接口：** `POST https://life-hwbeta.casstime.com/boss-admin/boss-auth-bff/public/auth/login/password`

**说明：** 登录服务已在测试环境运行，所有 boss-bff 工程接口统一使用测试环境前缀 `https://life-hwbeta.casstime.com/boss-admin`。

**请求体：**

```json
{
  "userLoginName": "12312312313",
  "password": "boss-account-121",
  "loginScene": "cs"
}
```

**Token 提取：** 响应中的 `accessToken` 字段即为后续接口调用所需的 JWT Token。

**使用方式：** 请求头添加 `Cookie: security_context=<accessToken>`

---

## 3. 本地测试前置检查

### 3.1 FeignClient 指定本地 URL

测试时需要给 FeignClient 加 `url` 指向本地服务地址，**测试完成后必须还原**。

示例：

```java
@FeignClient(name = "boss-user-service", url = "http://localhost:12004")
public interface BossUserInfoClient extends BossUserInfoService
```

### 3.2 management port 不能重复

检查每个工程的 `bootstrap.yml`，确保 `management.port` 各不相同：

```yaml
management:
  port: 2992
```

各工程的 management port 不能冲突，否则本地多服务同时启动会端口占用报错。

**注意：** 如果工程未配置 `management.port`，Spring Boot 默认使用 30000 端口。多个服务同时启动时必须显式指定不同的 management port，否则第二个服务会因端口冲突启动失败。

当前各工程 management port 分配：

| 工程 | management.port |
| --- | --- |
| icec-cloud-boss-auth-bff | 2992 |
| icec-cloud-boss-agent-workbench-bff | 29934 |
| icec-cloud-life-cs | 30002 |
| icec-cloud-life-im | 30001 |

---

## 4. 测试接口 Base URL

### 4.1 本地服务

| 工程 | Base URL | 端口 |
| --- | --- | --- |
| icec-cloud-boss-agent-workbench-bff | `http://localhost:10097/boss-agent-workbench-bff` | 10097 |
| icec-cloud-life-cs | `http://localhost:20092` | 20092 |
| icec-cloud-life-im | `http://localhost:20093` | 20093 |
| icec-cloud-life-user | `http://localhost:6602` | 6602 |

### 4.2 测试环境

| 工程 | Base URL |
| --- | --- |
| icec-cloud-boss-auth-bff | `https://life-hwbeta.casstime.com/boss-admin/boss-auth-bff` |
| icec-cloud-boss-user | `https://life-hwbeta.casstime.com/boss-admin/boss-user` |
| icec-cloud-life-user | `https://life-hwbeta.casstime.com/life-user` |

---

## 5. 待确认事项

| ID | 问题 | 影响范围 | 优先级 | 待查 |
|----|------|---------|--------|------|
| C-001 | 本地启动 icec-cloud-boss-user 服务的端口（`12004`）是否在所有团队成员机器上统一？团队内是否有端口分配约定表？ | §4.1 | 🟡 P2 | 查团队 wiki 或向架构组确认 |
| C-002 | 测试环境 base URL 前缀 `life-hwbeta.casstime.com` 是否仅用于 boss-bff？life-bff（如 `agent-workbench-bff`）的测试环境 URL 是否相同？ | §4.2 | 🟠 P1 | 查各 BFF 的 `application-test.yml` |
| C-003 | `loginScene=cs` 账号 `12312312313 / boss-account-121` 是否仍有效？测试环境是否定期清理？ | §2 | 🟡 P2 | 查测试环境账号表或联系测试组 |
| C-004 | icec-cloud-life-cs 与 icec-cloud-life-im 的测试环境 Base URL 缺失，是否需要补登？ | §4.2 | 🟡 P2 | 查 `icec-cloud-life-cs` / `icec-cloud-life-im` 的 `application-test.yml` |
