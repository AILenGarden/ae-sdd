# 2026-07-01 | java3d-coding-skill v1.2.0 - 按 life-user README 校正适配器分层决策

## Summary

`plugins/java3d-coding-skill/SKILL.md` 上线（v1.1.0）后未与真实项目文档逐条核对过，本次以
`D:\Item\life\admin\icec-cloud-boss-user\README.md`（life-user 项目族 DDD 分层约定原始文档）为权威源，
逐条比对适配器内容，修正 1 处直接矛盾（根 pom 存在性）、补齐 2 类完全缺失的分层模式（防腐层 Facade、
集成/领域事件双线）、修正 3 处命名与路径偏差（DataConverter 命名、Mapper/dao 路径、模块粒度），
并补齐技术栈表遗漏项（Guava/RxJava，Kafka 由踩坑脚注升级为正式技术栈行）。本次是纯决策知识层的
文本校正，不改共有 `coding-skill.md`、不改 loader 机制。

## Changes

| Area | Change |
|---|---|
| SKILL.md §2.1 | 工程模块结构由"4 类型（Service 揽 4 层）"改为"5 平级模块（interfaces/application/domain/infrastructure/service 各自独立 pom + 独立打包成 JAR）"，命名模板由 `icec-cloud-life-{module}` 改为 `{project}-{layer}` |
| SKILL.md §2.2 | 落点表去除 application/infrastructure/interfaces 三层的 `{subdomain}` 嵌套（仅 domain 层保留 `{aggregate}` 嵌套）；新增 Facade（domain 定义/infrastructure 实现）、ApplicationEventPublisher/DomainEventPublisher（接口定义于 application/domain，实现于 infrastructure）、EventHandler（interfaces 订阅）、Command/Query vo 落点行；Mapper 落点补 `persistence.dao` 层级 + 独立 `dao.xml` 行 |
| SKILL.md §2.4 | `PersistenceConverter` 命名统一改为 `DataConverter`（判定表 + SOP 描述同步） |
| SKILL.md §1.1 | 技术栈事实表新增 Guava、RxJava 行；消息中间件行改为"底层 Kafka + 上层 messagebus 封装"双层表述 |
| SKILL.md §4 | 骨架展开决策表 `{Resource}PersistenceConverter` 改名 `{Resource}DataConverter`；新增 Facade 调用决策行、集成事件发布决策行 |
| SKILL.md §5 / §8#1 | "无根 pom" 表述改为"根 pom 存在但只做 dependencyManagement+插件+modules 聚合，不直接声明 dependencies"；编译验证从"各模块独立 `mvn -pl` 编译"改为"根目录 `mvn compile` reactor 聚合构建" |
| SKILL.md §9 | 映射表同步补充 Facade/事件发布器落点、根 pom 聚合构建描述 |
| SKILL.md frontmatter + 注册信息块 | description 更新为反映 v1.2.0 校正内容；版本号 1.1.0 → 1.2.0 |
| registry.yaml | `java3d-coding-skill` 条目 version 1.1.0 → 1.2.0 |

## 触发原因

- 用户核对适配器与 life-user 项目族真实 README 文档时发现多处不一致（用户原话："以 Readme 为准，使 Skill 对齐此文档"）
- README 是本项目族 DDD 分层约定的一手来源；适配器 v1.1.0 编写时对 Facade/事件模式、模块粒度、根 pom 存在性等细节未逐条核对源文档，存在编写疏漏

## 影响范围

- 纯文档变更（决策知识层文本），不涉及运行时逻辑、门禁行为、CLI 命令
- 不改变已有门禁、子 SKILL 职责边界、文档存放路径
- 仅推进插件自身版本号（1.1.0→1.2.0），**不触发**母版 ae-sdd 版本号（三处一致性 UC-01 不受影响，因为 UC-01 校验的是母版版本非插件版本）
- 破坏性变更：`{Resource}PersistenceConverter` 命名改为 `{Resource}DataConverter`——若已有项目按旧命名生成代码，新增代码需按新命名；历史代码不强制重命名（README 本身也是新落地项目的约定源，非强制回溯旧代码）
- §1.1bis base package 固化表（v3.6.2 实测值）本次不改，与 README 示例包名（`com.casstime.life.user`，用于说明分层结构的示意名）分属不同性质内容

## 验证方式

- `python tools/bin/ae-sdd update-check` 全绿（本次不改母版版本号/门禁注册/CLI 命令，UC-01~05 不受影响）
- 人工核对：适配器 §2.1/§2.2/§4/§5/§8/§9 与 README 包结构树、DDD 分层约定 1-7 条逐条比对一致
- `ae-sdd plugin validate` 校验 registry.yaml + SKILL.md frontmatter 合法（version semver 格式）

## Reviewer

待用户确认。
