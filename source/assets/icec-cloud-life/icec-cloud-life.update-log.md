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

### v1.1 — 2026-06-24（补全 §F 反向索引 + §G 脚本化）

**变更类型：** 增量更新（索引层补全）

**触发原因：** 用户指出"项目资产索引做得差"，要求对标 ES 思想并脚本化。life 资产 §F 反向索引仅 9 词（门禁要求 ≥20），且位置为粗粒度章节号；§G 读取 API 为"自然语言协议"无脚本实现。

**变更内容：**
- §1 lastAuditedAt：2026-06-17 → 2026-06-24
- §F 反向索引：9 词 → 28 词，位置从粗粒度"§4 / §6.9"精确到"§X.Y + 行号"（新增 AppService / @Transactional / Facade / FeignClient / Converter / CsTicket / ImSession / security_context / AccessUserInfoContext / ApiResult / PagedModels / cellphone / BCrypt / deleted_flag / 双启动模块 / LocalDateTime / job-spring-boot-starter / MapStruct 等 19 词）
- §G 读取 API：从伪代码协议升级为 `ae-sdd assets` 脚本化说明（含场景化 API 表 + 底层 API 表 + 调用示例）
- §10 缺口新增 #11（✅ 已补 2026-06-24）

**配套脚本：** `tools/lib/assets_index.py`（倒排索引 + 分词 + BM25 评分）+ `ae-sdd assets query/outline/section/stats` CLI 子命令

**验证状态：** 已验证（`ae-sdd assets query "AppService" --asset-file .../icec-cloud-life.assets.md` 精确命中 CsTicketAppService / ImSessionAppService，BM25 排序合理）

---

### v1.3 — 2026-06-26（工程级拆分 life-im — schema v3 IM 域 + 融云防腐 + 4 子域范本）

**变更类型：** 增量更新（按 schema v3 §15 工程级拆分 + §1.2 部署信息 + §14 安全提示）

**为什么拆 life-im：**
- 形成"客服 vs IM"两大 Service 域对比（与 life-cs 形成完整 Service 域范式矩阵）
- IM 域展示第三方 IM 云集成（RongCloud）+ 4 业务子域 + 多态消息体（6 类型）+ 签名校验 + STORY-020 内存推导最新值
- 92KB 信息量第二（仅次于 life-cs 110KB）

**变更内容：**
- 🆕 新建 `icec-cloud-life.icec-cloud-life-im.assets.md` 工程级子文件（~50KB）
  - §1.1 部署信息（含融云 rongcloud.app-key/app-secret/api-url/server-name @Value 配置）
  - §1.2 安全提示 8 项（含 **🔴 S-301 融云 app-secret 明文风险**）
  - §3 完整技术栈 21 项（含融云 server-sdk-java + MapStruct 1.5.3.Final + STORY-020 重构相关）
  - §5.2 ImMessageAppService **16 个核心方法 + 19 字段常量**（VENDOR_RONGCLOUD/DEFAULT_VENDOR/CS_USER_FLAG/systemAccountRegistered 等）
  - §5.2 ImUserOnlineStatusAppService 1 个核心方法（融云在线状态回调）
  - §5.3 Domain 层（ImMessageDomainService.fillLatest 三侧锚点推导 + CsUserServiceFacade + UpsertRongyunOnlineStatusDO）
  - §5.4 ImMessageRepositoryImpl **13 个核心方法 + STORY-020 重构**（batchUpsert 仅覆盖内容字段 / findLatestSideMessagesAfterStartAt 标量子查询）
  - §6.1 IM 域 8 条红线（始终验签 / channelType 守卫 / 事务外触发 Hook / batchUpsert 覆盖范围 / CS_USER 守卫 / generateCsUserId 容错 / STORY-020 内存推导）
  - §6.2 6 种消息体类型多态约束（TEXT/IMAGE/FILE/RICH_TEXT/MARKDOWN/COMPOSITE）
  - §7.3 融云回调签名校验流程图（appKey + nonce + timestamp + signature）

- 主体 §2 微服务清单 `icec-cloud-life-im` 行加 `[详见](icec-cloud-life.icec-cloud-life-im.assets.md)` 链接
- 主体 §B 模块索引 `icec-cloud-life-im` 行"文档"列改为 `[工程级子文件]`
- 主体 §15 工程级拆分表 `icec-cloud-life-im` 标记 ✅ 已生成

**待用户确认的缺口（高优先级）：**

| # | 缺口 | 影响范围 |
|---|------|---------|
| 1 | 融云 server-sdk-java 实际依赖引入方式（传递依赖？哪个 starter？） | §3 技术栈 |
| 2 | 融云 app-key / app-secret / api-url / server-name 实际注入位置（@Value 或其他） | §1.1 部署信息 |
| 3 | ImSessionAppService / ImTokenAppService 完整方法列表 | §5.2 AppService |
| 4 | 融云防腐层 4 个核心类（RongCloudConfig/Client/FacadeImpl/ImMessageBodyConverterImpl）方法级 | §5.4 Infrastructure |
| 5 | 错误码（20011 objectName 不支持 / 20020 私聊双边会话不存在 / 20021-20023）| §6.1 IM 域红线 |
| 6 | STORY-020 latest 内存推导在 CS 编排层的具体使用方式 | §5.3 Domain |
| 7 | 父 POM description 占位符 "xxx????" 待填 | §2 子模块结构 |
| 8 | spring-boot-maven-plugin 2.7.5 与 Spring Boot 1.5.7 版本不一致 | §3 技术栈 |

**配套文件：**
- `standards/project-assets/project-assets-schema.md`（v3.5.0）
- `standards/project-assets/project-assets-template.md`（v3.5.0）
- `skills/cross-cutting/project-assets-update-skill.md`（v3.5.0）
- `assets/icec-cloud-life/icec-cloud-life.icec-cloud-life-cs.assets.md`（v1.2 已生成的姊妹范本）

**验证状态：** 已验证（按 schema v3 §15 SOP 全字段填写 + 可信度三态标注 + §A 反向索引 30+ 词 + §1.1 部署信息含融云配置 + §14 安全提示 8 项）

---

### v1.2 — 2026-06-26（schema v3 工程级粒度拆分 + 首个工程级子文件 life-cs）

**变更类型：** 增量更新（按 schema v3 §15 工程级拆分 + §12 横切专题 + §1.2 部署信息 + §14 安全提示）

**触发原因：** 用户 2026-06-26 完成 ae-sdd 项目资产 v3.5.0 重构（CHANGELOG `2026-06-26-project-assets-v3-restructure.md`），要求按新结构拆分 life 域第一个工程。

**为什么先拆 life-cs：**
- 被 STORY-003-BE 直接依赖（同事 `function/登录鉴权-BE-接口逻辑排查.md` 跨 5 工程就含 life-cs），该 Story 后续要产 function/，先把生命之基的 life-cs 工程级子文件做出来
- 110KB 信息量最大（life-cs.md 110KB / life-im 92KB / life-vehicle 103KB）
- 状态机 + 工单业务复杂，能充分演示 schema 各种字段实际填法

**变更内容：**
- 🆕 新建 `icec-cloud-life.icec-cloud-life-cs.assets.md` 工程级子文件（~50KB / ~770 行）
  - §1.1 部署信息 25 个字段（profile / db / redis / feign / xxl-job / panda / 镜像 / 私服 / 加密 全套）
  - §1.2 安全提示 5 项（**🔴 S-101 Redis 密码明文为高危** + S-102 Actuator 外露 + S-103 Hystrix 关闭 + S-104 SPI 注释 + S-105 encrypt.failOnError:false）
  - §3 完整技术栈 16 项（redisson 3.5.7 + spring-statemachine + spring-retry + xxl-job + guava + assertj 3.11.1 等内部 starter）
  - §5 核心类方法级实现 ~70+ 方法（4 个 AppService + Orchestrator + 状态机驱动 + Handler + 2 Facade Impl + Feign + 3 Repository Impl + Notification Gateway）
  - §6.2 客服域特有约束 9 条（状态机驱动 / 超时字段语义 / claimed_at 防御 null / ACTIVE_STATUSES 收窄 / 结单 5 步 / 三端通知异步 / 融云 ONLINE 时间刷新条件 / C25 失败传播 / 坐席乐观锁）
  - §8 数据库表 6 张（含 cs_ticket 字段变更详情 + cs_user_status_rongyun_sync 新表）
- 主体 §2 微服务清单 `icec-cloud-life-cs` 行加 `[详见](icec-cloud-life.icec-cloud-life-cs.assets.md)` 链接
- 主体 §B 模块索引 `icec-cloud-life-cs` 行"文档"列从 `§4` 改为 `[工程级子文件](...)`
- 🆕 主体新增 §12 横切专题文件索引（三类空表占位，等 Story 触发）
- 🆕 主体新增 §15 工程级粒度拆分记录（life-cs ✅ 已生成 + 20 个工程 ⏳ 待拆清单）
- §10 缺口新增 #12（✅ 已补 2026-06-26 life-cs 拆分）

**待用户确认的缺口（高优先级）：**

| # | 缺口 | 影响范围 |
|---|------|---------|
| 1 | `CsTicketConverter` 完整方法列表 | §5.1 Converter 完整性 |
| 2 | `CsTicketAppService` 完整方法列表 | §5.2 AppService 完整性 |
| 3 | `CsRoutingDomainService` 路由算法（基于负载 / 在线状态 / 历史分配）| §5.3 Domain Service |
| 4 | `CsTicketErrorCode` 完整错误码（16000-16999 段位已确认，具体码值待探查）| §6.4 错误码表 |
| 5 | xxl-job 三类 Handler 的 cron 配置（每分钟？每 5 分钟？）| §5.6 定时任务 |
| 6 | ES 索引（CsTicketSearchIndexFacade 详情）| §5.4 Infrastructure |
| 7 | 域事件消费者清单 | §7.3 域间事件 |

**配套文件：**
- `standards/project-assets/project-assets-schema.md`（v3.5.0）
- `standards/project-assets/project-assets-template.md`（v3.5.0）
- `skills/cross-cutting/project-assets-update-skill.md`（v3.5.0）
- `CHANGELOG/2026-06-26-project-assets-v3-restructure.md`

**验证状态：** 已验证（按 schema v3 §15 SOP 全字段填写 + 可信度三态标注 + §A 反向索引 28 词 + §1.2 部署信息 25 字段 + §14 安全提示 5 项）

---

```
### vX.Y — YYYY-MM-DD

**变更类型：** 新建 / 增量更新 / 纠错

**触发原因：** （Story 编号 / 新发现 / 架构调整）

**变更内容：**
- 章节 §X：...

**探查方法：** ...

**验证状态：** 已验证 / 待验证
```
