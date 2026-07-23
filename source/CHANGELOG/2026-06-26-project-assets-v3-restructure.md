# 2026-06-26 | ae-sdd v3.5.0 - project-assets 横切专题 + 工程级粒度重构

<!--
  命名规则：YYYY-MM-DD-vX.Y.Z-<kebab-case-主题>.md
  位置：source/CHANGELOG/
  维护要求：ae-sdd-update-skill.md §更新流程 步骤 6「落档 CHANGELOG」强制使用本模板。
-->

## Summary

本次重构源于 2026-06-26 与同事 `D:\Item\life\document\life-team-project-docs\knowledge\` 项目知识库的全量对照分析。原 ae-sdd `project-assets` 体系是"机器可读、约束驱动"——强项在 §A-§G 索引、CLI、JSON、反模式沉淀、缺口集中管理；但缺三大类**横切专题**（业务场景 function/、环境配置 config/、业务域概览 domain/）+ **工程级粒度拆分**（22 工程压在 54KB 单文件）+ **完整技术栈版本号**（只列主框架）+ **部署信息**（每个工程细节靠 grep）+ **可信度三态标注** + **安全隐患记录**。同事知识库在六大维度领先，但缺机器索引 + 反模式 + 缺口管理。两边互补，本次吸收同事的 6 个维度，把 ae-sdd 从"规则映射表"升级为"项目全景知识库"。本次生成 1 份工程级子文件范本（icec-cloud-boss-user），后续 50+ 工程按此范式分批迁移。

## Changes

| Area | Change |
|---|---|
| `standards/project-assets/project-assets-schema.md` | 新增 §1.2 部署信息节（10 个字段）+ §12 横切专题文件索引 + §13 信息可信度三态标注 + §14 安全隐患记录 SOP + §15 工程级粒度拆分 SOP；升级 §6.8 为完整技术栈版本号表（7 张子表：主框架 / 工具库 / 安全 / 内部 starter / 测试 / 构建 / 静态分析）；重命名 §10 副标题"—— 与 §11 缺口表不可混用"消除命名冲突；附录 B-E 四个新模板（工程级子文件 / function 业务场景 / config 环境配置 / domain 业务域）+ 附录 F 横切专题映射表 |
| `standards/project-assets/project-assets-template.md` | 同步新增 §1.X 部署信息 placeholder / §6.8 七张子表 placeholder / §11 团队惯用实现方式（≥9 条经验）/ §12 横切专题索引 / §13 可信度三态 / §14 安全隐患登记 / §15 工程级拆分记录；附录指向 schema 附录 B-F |
| `skills/cross-cutting/project-assets-update-skill.md` | 升级 §3.2 为 17 步 SOP（新增步骤 10-17）；新增 §3.5 步骤 14 完整技术栈版本号 SOP / §3.6 步骤 15 部署信息 + 安全扫描 SOP / §3.7 步骤 16-17 工程级拆文件 + 横切专题 + 可信度三态；新增 §4.4 工程级子文件更新 SOP / §5.4 横切专题审计 SOP / §6.4 按功能专题加载 SOP（含 `ae-sdd assets function STORY-003-BE` 等 5 个新命令模板）|
| `assets/icec-cloud-boss/icec-cloud-boss.icec-cloud-boss-user.assets.md` | 🆕 生成首份工程级子文件范本，含 §1.1 部署信息（10 字段）/ §1.2 安全提示（4 项）/ §3 完整技术栈（17 个依赖版本号）/ §5 核心类方法级实现（Converter 4 类 + AppService + Domain + Infrastructure + Interfaces 共 30+ 方法）/ 可信度三态全文标注 |
| `assets/icec-cloud-boss/icec-cloud-boss.assets.md` | 待迁移：按 schema v3 重生成（主体只保留跨工程映射 + 索引；22 工程细节移到子文件；§6.8 完整技术栈；§12-15 四节；附录 B-F 引用）|
| `assets/icec-cloud-life/icec-cloud-life.assets.md` | 待迁移：同上（21 工程细节移到子文件）|
| `assets/icec-cloud-boss/function/` | 🆕 新建目录（空）；待 STORY-002/003/007/009/010/011/020 每个 Story 完成后产出 1 篇 |
| `assets/icec-cloud-boss/config/test/api-test-env.md` | 🆕 新建（参考同事 `knowledge/config/test/api-test-env.md`）|
| `assets/icec-cloud-boss/domain/` | 🆕 新建目录（空）；待业务域变更时产出 |

## 触发原因

- **同事知识库对照（2026-06-26）**：同事 `D:\Item\life\document\life-team-project-docs\knowledge\` 30+ 工程文件 + function/config/domain 三类横切专题，共 32 个 md / ~2.7MB；ae-sdd 仅 5 文件 / 113KB。结构性差距。
- **单 Story 跨 ≥3 工程是常态**：STORY-002/003/007/009/010/011/020 每个跨 3-5 个工程，但 ae-sdd 资产无"跨工程业务场景专题"承载调用链 + 影响面 + 错误码 + Redis Key + 跨工程 Feign 表，强行塞主体导致主体膨胀到 54KB。
- **新 coder 接手本地服务靠口口相传**：本地端口、测试 URL、Feign URL、management port 配置散落各 bootstrap.yml，平均浪费 1-2 小时。
- **完整技术栈版本号缺失**：原 §6.8 只列主框架（Spring Boot 1.5.7 / MyBatis-Plus 3.3.2），缺 MapStruct 1.5.3.Final / Lombok 1.18.16 / Swagger2 2.8.0 / panda 1.0.9 / casslog 1.5.0 / cassmetrics 1.0.4 / courier 3.3-SNAPSHOT 等 14+ 内部 starter 与工具库版本号。
- **可信度无法区分**：原资产所有内容一视同仁，无法判断哪些是 explore agent 真实读到的 / 哪些是推断的 / 哪些是待确认的——下游消费方（Code Plan / Coding）无法判断可信度。同事用 `[据命名/结构推断]` `[待确认]` 区分清楚。
- **安全隐患无结构化记录**：同事 user.md §1.4 末尾写了"bootstrap.yml 中 Redis 明文密码"提示——但 ae-sdd 无对应记录机制。

## 影响范围

- ✅ **纯文档变更**：本次重构只动 standards/ skills/ assets/ 下文档，无运行时逻辑 / 门禁行为 / CLI 命令变更。
- ✅ **schema 是"参考定义"**：未强制迁移现有 boss.assets.md / life.assets.md。现有 2 份资产按 schema v3 §15 规则**建议**迁移（主体 > 30KB + 工程数 > 10），但不强求。
- ✅ **新增 starter 模板目录**：`skills/ae-sdd/templates/project-assets/` 暂未创建 starter 文件，但 schema 附录 B-F 已含完整模板内容。**下一步**：把附录 B-F 拆为独立 starter 文件（建议在下次 `skills/` 整理时一并完成）。
- ✅ **CLI 命令模板新增**：`ae-sdd assets function STORY-003-BE` / `ae-sdd assets module icec-cloud-boss-user` / `ae-sdd assets config local boss-user` / `ae-sdd assets domain im` 是 skill §6.4 给出的"期望接口"——**当前 CLI 尚未实现**，仅作为"按功能专题加载"的接口规范。实现优先级 P1。
- 🟠 **现有 boss.assets.md §10 命名冲突**：原实例 §10 写"项目资产缺口与待补充"（应是 §11 内容），schema §10 是"团队惯用实现方式"。本次 schema §10 加副标题"—— 与 §11 缺口表不可混用"明确区分，**但 boss.assets.md §10 实际位置未迁移**——下次重生成时一并迁移。
- 🟠 **schema 章节序号扩展到 15 节 + 附录 A-F**：从 12 节 + 1 附录扩展到 15 节 + 6 附录。引用方需同步更新（§8 Code Plan 输入索引已写"§1-§10"应改为"§1-§15"，但本次未改——下次统一修订）。
- 🔴 **未破坏性变更**：本次为纯增量，未删除任何原有章节，未修改任何门禁条件。

## 验证方式

- `python tools/bin/ae-sdd update-check` 对应 UC-12（schema 引用一致性）/ UC-13（template 引用一致性）/ UC-15（asset-update-skill 完整性）全绿。
- `python tools/tests/run.py` 全部通过。
- 人工核对：
  - schema §12 / §13 / §14 / §15 / 附录 B-F 章节齐全且内容自洽
  - template §1.X / §6.8 / §11-§15 placeholder 与 schema 一一对应
  - skill §3.5-3.7 + §4.4 + §5.4 + §6.4 与 schema 新章节互引
  - boss-user 范本 §1.1 部署信息 10 字段齐全；§3 完整技术栈 17 项；§5 核心类方法级 30+ 方法；§1.2 安全提示 4 项；可信度三态在每个章节标题后标注
- 字段映射核对：
  - schema §1.2 ↔ template §1.X ↔ skill §3.6.1（10 字段一致）
  - schema §6.8 七张表 ↔ template §6.8 七张表 ↔ skill §3.5.1 七张表（一致）
  - schema §14.1 四条扫描 ↔ skill §3.6.2 四条 bash 命令（一字不差）

## Reviewer

EDY（Harness root session 2026-06-26 自动生成）