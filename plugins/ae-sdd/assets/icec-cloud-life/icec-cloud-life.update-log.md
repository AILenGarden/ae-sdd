# icec-cloud-life 项目资产 — 更新日志

> 文件路径：`D:\Item\ae-sdd\assets\icec-cloud-life\icec-cloud-life.update-log.md`
> 对应资产：`icec-cloud-life.assets.md`

---

## 变更记录

### v1.0 — 2026-06-17（首版）

**变更类型：** 新建（首次探查）

**探查方法：** 多 Agent 并行 Explore（5 个 Agent 同时扫描不同服务）

**覆盖范围：**

| 维度 | 覆盖情况 |
|------|---------|
| 微服务清单 | 21 个（全量，含端口/ContextPath/类型）|
| DDD 分层落点 | 精确包路径：cs + im 两个冰山模块（其余域元信息级别）|
| 命名约定 | 14 类对象（来自 cs + im + im-bff 真实代码）|
| 工程约束 | §6.1-§6.11（继承 constraints/ + 项目特化补缺）|
| 跨服务契约 | 11 个 SPI 子模块清单 + 关键 Feign 消费关系 |
| 索引层 | §A-§G 全量（大纲/模块/字段/组件/API/关键词/读取API）|

**关键探查发现：**

1. **工程根目录结构特殊**：`D:\Item\life` 下顶层代码在 `2c/` 子目录，boss 相关模块不在此工程（在 `d:\Item\icec-cloud-boss`）
2. **cs 服务 Converter 命名**：application 层 Converter 使用 `@UtilityClass`（已验证 `CsTicketConverter`）；infrastructure 层使用 `XxxPersistenceConverter`
3. **im 服务 Converter 命名差异**：im Converter 使用 `public final class` + 私有构造（代码注释明确说明"不使用 @UtilityClass"）
4. **Controller 命名双轨制**：cs 服务 interfaces 层实现 SPI 接口，命名 `XxxServiceImpl`；BFF 层实现 api 接口，命名 `XxxRestImpl`
5. **im-bff 是单模块工程**（无子模块分层）；包路径 `com.casstime.cloud.life.bff.im`（与多模块 cs/im 不同）
6. **SPI 父工程含 11 个子模块**，包括跨产品线的 `icec-cloud-boss-abnormal-spi`
7. **life-user 服务有 6 个子模块**（含 web-api + web 双 Web 层），比 cs/im 的标准四层多两个模块
8. **端口疑似冲突**：life-obs-service 与 life-user-service 均配置了 6602，需核实

**待用户确认的缺口（高优先级）：**

| # | 缺口 | 影响范围 |
|---|------|---------|
| 1 | life-obs 端口 6602 与 life-user-service 是否确实冲突 | §2 微服务清单准确性 |
| 2 | ServiceProviderConstants 各字段值（11 个 SPI）需代码核实（部分为推测）| §7.1 准确性 |
| 3 | life-auth-bff 的 Token 颁发/Cookie 写入细节（与 boss-auth-bff 是否共享认证逻辑）| §6.6 安全规范 |
| 4 | life-user / life-vehicle / life-workticket 的 DDD 包路径（目前只有 cs + im 两个冰山模块）| §4 完整性 |

---

## 变更记录模板

```
### vX.Y — YYYY-MM-DD

**变更类型：** 新建 / 增量更新 / 纠错

**触发原因：** （Story 编号 / 新发现 / 架构调整）

**变更内容：**
- 章节 §X：...

**探查方法：** ...

**验证状态：** 已验证 / 待验证
```
