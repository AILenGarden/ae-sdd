# 2026-07-02 | java3d-coding-skill v1.3.0 - 新增 BFF 三层落点决策

## Summary

`plugins/java3d-coding-skill/SKILL.md` v1.2.0 只建模了 **Service 类工程**（五模块 DDD，母本 A，覆盖 10 个 service
工程），但 life 项目族还有**第二类工程形态——BFF 三层结构**（母本 B，覆盖 14 个 bff 工程，2c 6 份 + admin 8 份，
全线逐字节一致）完全未建模。本轮以 14 份 BFF README 为权威源，新增 §2.5「BFF 三层落点表」+ §3 BFF 数据策略红线精确化，
让适配器从"只服务 service 类工程"扩展为"覆盖 service + bff 两类工程形态"。老的 spi 双模块 README（母本 C，
与母本 A 冲突）已裁定丢弃不纳入。本次是纯决策知识层的文本增强，不改共有 `coding-skill.md`、不改 loader 机制。

## Changes

| Area | Change |
|---|---|
| SKILL.md §2.5（🆕 新增）| 新增「BFF 三层落点表」章节：声明 BFF 是与 §2.2 service 五模块并列的独立工程形态、**BFF 不设独立 domain 层**（README 原文「application 合并 application 和 domain」）；7 行落点表（RestImpl / Converter / ServiceApp / Facade定义 / FacadeImpl / ServiceClient / Config）+ object 持有约束（仅 VO/DTO，禁 PO/DO）|
| SKILL.md §3 | BFF 数据策略由 1 行「BFF 直接操作 DB / Redis / Kafka」拆为 3 行精确红线：①BFF 操作 DB→禁 ②BFF 操作 Redis/Kafka→禁 ③BFF 使用分布式缓存→禁（可用本地缓存 Caffeine），对齐 README 唯一约定「尽量不存储数据；万不得已可用本地缓存而非分布式缓存/数据库」|
| SKILL.md §9 | 共有→适配器映射表 §3 行补 §2.5 BFF 落点 + §3 BFF 数据红线细化的叠加声明 |
| SKILL.md frontmatter + 注册信息块 | description 补「BFF 三层落点决策」；version 1.2.0 → 1.3.0 |
| registry.yaml | `java3d-coding-skill` 条目 version 1.2.0 → 1.3.0 + description 同步 |

## 触发原因

- 用户要求评估 java3d-coding-skill 还有哪些值得强化的方向；首轮分析时误把"真实代码"当 ground truth（被用户纠正"以 README 为准"），重新校准后系统性读完 30+ 份 README，发现 BFF 类工程（12 份 README 逐字节一致）是适配器完全未覆盖的第二套工程形态
- 适配器 v1.2.0 §2.2 只给 service 五模块落点，BFF 只在 §2.2 有一行占位（`{Resource}RestImpl`）；AI 在 BFF 工程编码时缺乏"类放哪个包""是否设 domain 层""converter 在哪层"的决策依据
- spi 双模块 README（母本 C）与 service 五模块 README（母本 A）在 facade/dao 落点上直接冲突，属已淘汰的老形态，用户裁定"老的不要了"不纳入

## 影响范围

- 纯文档变更（决策知识层文本），不涉及运行时逻辑、门禁行为、CLI 命令
- 不改变已有门禁、子 SKILL 职责边界、文档存放路径
- 仅推进插件自身版本号（1.2.0→1.3.0），**不触发**母版 ae-sdd 版本号（三处一致性 UC-01 不受影响，UC-01 校验母版版本非插件版本）
- `plugins/` 不在 `build_dist.py` 白名单、不在 `update-graph.json` 联动规则，故无需 dev-sync / build-dist / update-check
- 破坏性变更：无（BFF 是新增章节，不改 §2.2 service 已有落点）

## 验证方式

- `ae-sdd plugin validate`（registry.yaml + SKILL.md frontmatter semver 合法）
- 人工核对：§2.5 落点表 7 行 + §3 BFF 数据 3 条红线，与 14 份 BFF README 原文逐条比对一致：
  - 2c 线 6 份：`icec-cloud-life-{auth,content-feed,im,user-journey,vehicle,workticket}-bff/readme.md`
  - admin 线 8 份：`icec-cloud-boss-{agent-workbench,auth,configuration,log,notification,user,vehicle,workticket}-bff/README.md`
- 不涉及母版版本号联动，UC-01~05 不受影响

## Reviewer

待用户确认。
