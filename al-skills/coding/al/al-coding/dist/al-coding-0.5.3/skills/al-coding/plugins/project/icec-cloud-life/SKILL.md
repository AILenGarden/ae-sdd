---
name: icec-cloud-life
description: icec-cloud-life 项目适配器；仅在项目身份精确匹配时加载 Life 的技术栈、工程结构、分层、安全、接口、数据库、测试和代码风格约束。
metadata:
  display_name: icec-cloud-life
  project_id: icec-cloud-life
  version: 0.1.0
  role: project-constraints
---

# icec-cloud-life 项目适配器

## 项目身份

- 项目根：`D:\Item\life`
- 项目身份：`icec-cloud-life`
- 根目录不是 Git 仓库；每个子工程独立维护 Git。
- 代码与构建范围以目标子工程及其实际代码为准，不把整个根目录当成单一 Maven 工程。

## 技术与架构事实

- Java 8
- Spring Boot 1.5.7
- Spring Cloud Dalston
- Maven 多模块子工程；根目录没有 root pom
- 服务内采用 DDD 分层：`interfaces → application → domain → infrastructure`
- 系统调用链通常为：`api → bff → spi → service`

## 权威约束文件

任务命中本项目后，先读取 `D:\Item\life\AGENTS.md`，再按任务范围读取以下约束；这些文件是事实源，本 SKILL 不复制其正文：

| 约束 | 权威文件 |
|---|---|
| 技术栈 | `D:\Item\life\constraints\technology-stack.md` |
| 工程结构 | `D:\Item\life\constraints\project-structure.md` |
| 分层与依赖 | `D:\Item\life\constraints\layered-arch.md` |
| 代码风格 | `D:\Item\life\constraints\code-style.md` |
| 接口契约 | `D:\Item\life\constraints\api.md` |
| 数据库 | `D:\Item\life\constraints\database.md` |
| 安全 | `D:\Item\life\constraints\security.md` |
| 测试 | `D:\Item\life\constraints\testing.md` |
| 隐含约束 | `D:\Item\life\constraints\implicit-constraints.md` |

根据目标子工程和变更面选择约束：接口或 SPI 变更读接口、结构和分层；数据变更读数据库；认证或敏感数据变更读安全；测试变更读测试；无法排除影响时保留待确认并补读。

## 项目规则

- 项目身份必须精确匹配 `icec-cloud-life`；其他 life、boss 或相似目录不自动套用本适配器。
- Story 权威副本位于 `D:\Item\life\document\life-team-project-docs`；`ae-sdd-doc` 不能替代该目录。
- 修改代码前进入目标子工程执行 `git status`、`git diff` 和历史查询；禁止在 `D:\Item\life` 根目录执行 Git 操作。
- 构建和测试使用目标子工程自己的 Maven 入口；无 root pom 时不能假设存在根级 Maven 命令。
- 本项目适配器只提供项目事实和约束指针；Java、DDD、命名和通用质量规则分别由对应适配器与 Core 提供。
