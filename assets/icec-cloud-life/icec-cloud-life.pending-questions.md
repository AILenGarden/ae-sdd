# icec-cloud-life 项目资产 — 待确认问题清单

> 本文件记录探查过程中发现的、**无法自行确认的问题**。
> - **问题解决后：直接删除对应条目**（不需要归档，消掉就好）
> - **不要把"待确认问题"写进 assets.md 正文**，保持资产主体只含已确认的事实
> - 解决时机：涉及该问题的 Story 生成前 / 偶然发现了答案 / 专项排查时

---

## 待确认问题

| ID | 问题描述 | 影响章节 | 严重度 | 发现时间 | 解决线索 |
|----|---------|---------|--------|---------|---------|
| Q-001 | `icec-cloud-life-obs-service` 的 bootstrap.yml 中 `server.port: 6602` 与 `life-user-service` 相同，疑似拷贝遗留 bug。obs-service 的真实端口是多少？ | §2 微服务清单 | 🟡 一般 | 2026-06-17 | 查 `icec-cloud-life-obs/icec-cloud-life-obs-service/src/main/resources/bootstrap.yml`，spring.application.name 字段缺失也可能是遗留 |
| Q-002 | `ServiceProviderConstants` 中 captcha / notification / user / vehicle / touchpoint / content-feed / workticket / ops-notification / configuration 共 9 个 SPI 服务名未从代码逐一核实（只确认了 im 和 cs） | §7 跨服务契约入口 | 🟠 严重 | 2026-06-17 | 扫各 spi 模块的 ServiceProviderConstants.java，或看对应 Feign Client 的 @FeignClient(name = "xxx") |
| Q-003 | `icec-cloud-life-auth-bff` 认证流程细节未探查：Token 颁发逻辑是否复用 boss-common 的 JwtEncoderUtil？Cookie `security_context` 写入在 auth-bff 还是 gateway？与 boss 体系是否共享 JWT 密钥？ | §6.6 安全规范 | 🟠 严重 | 2026-06-17 | 扫 `D:\Item\life\2c\icec-cloud-life-auth-bff\` 的核心认证类 |
| Q-004 | `life-user` / `life-vehicle` / `life-workticket` / `life-notification` / `life-touchpoint` 等域的 DDD 包路径（interfaces/application/domain/infrastructure）未逐一验证，当前 §4 只精确验证了 cs + im 两个域 | §4 DDD 内部分层落点 | 🟢 建议 | 2026-06-17 | 涉及对应域的 Story 生成时按需补充验证 |

---

> 上次检查：2026-06-17  /  当前待确认数：4（🔴0 🟠2 🟡1 🟢1）
