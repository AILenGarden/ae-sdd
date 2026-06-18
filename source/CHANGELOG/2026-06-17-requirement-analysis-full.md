# 2026-06-17 ae-sdd 需求分析体系重大重构

## 概述

本次重构是 ae-sdd 体系自 2026-06-06 以来**最重大的一次架构升级**，新增需求分析 SKILL、补充 DR 链路、建立统一的文档系统、增强项目资产索引化、强化智能路由 4 维判定。

**核心问题**：
- 流程从 DR 开始，DR 之前的过程（需求分析、PRD 拆解）没有 SKILL 管理
- 文档存放分散（`design/`、`.ae-task/`、`.ae-plan/`、`.trae/documents/`），无统一入口
- 项目资产全文加载太重，关联性分析效率低
- 智能路由只有 4 类需求判定，细粒度不够

## 关键变化

### 1. 新增独立 SKILL：requirement-analysis-skill（核心）

**位置**：`D:\Item\ae-sdd\skills\phase1-design\requirement-analysis-skill.md`
**规模**：~1056 行
**职责**：从 PRD/Issue/对话需求生成 RA 文档 + 5 维规模裁定 + 路由决策

**核心能力**：
- 8 维度并行挖掘（角色/场景/流程/数据/规则/设计方向/AC/假设）
- 5 问自检（证据/反例/边界/冲突/缺口）
- 缺口 4 级分级（🔴 阻断 / 🟠 严重 / 🟡 一般 / 🟢 建议）
- 5 维规模裁定（服务范围/接口变更/架构决策/数据变更/测试层级）
- 6 类规模 → 6 个下游 SKILL 路由

### 2. 补充 DR 链路：dr-generate + dr-review

- `dr-generate-skill.md`（A5）：从 RA 文档生成 DR 草稿
- `dr-review-skill.md`（A6）：对 DR 草稿进行 5 阶段评审

**完整 DR 链路**：requirement-analysis → **dr-generate** → **dr-review** → dr-update → story-generate

### 3. 文档系统统一：ae-sdd-doc/

- 8 类流程目录：PRD / RA / DR / Story / Task / Coding / Test / CR
- 迭代目录：`ae-sdd-doc/iterations/{YYYY-MM-DD}/`
- 版本号：`v{major}.{minor}`（旧版保留）
- ChangeLog：独立文件，按 doc-id 索引
- 关联性分析：业务 0/1 + 逻辑 0/1 双维判定
- gitignore：自动生成，幂等追加

### 4. 项目资产索引化升级

- §A-G 7 层索引（大纲/模块/字段/组件/API/反向/读取 API）
- 关联性分析通过索引快速完成
- 增量更新支持

### 5. 智能路由 4 维增强

**融合策略**：保留旧的 4 类需求判定作为 fallback，新增 4 维判定（来源 × 规模 × 现有产物 × 项目类型）为优先入口

| 入口 | 旧路径 | 新路径 |
|------|-------|-------|
| 有 PRD | 直接套 Story 7 区模板 | → requirement-analysis → dr-generate / story-generate |
| 有 Issue | 直接套 Story 7 区模板 | → requirement-analysis（轻量）→ 下游 |
| 对话需求 | 直接套 Story 7 区模板 | → requirement-analysis（多轮对话）→ 下游 |
| BUG 类 | 跳到 coding | → 直接 coding-skill |
| 配置类 | 跳到 coding | → 直接 coding-skill |

### 6. 现有 SKILL 迁移（9 个）

- 路径从硬编码（`design/`、`.ae-task/`、`.ae-plan/`）改为 `documentStorage.resolve_path()` API
- 核心功能（7 阶段挖掘、8 道闸、追溯链）**未触动**

### 7. 存量迁移工具（准备就绪）

- `scripts/migrate-docs.mjs`（444 行，Node.js 18+ 跨平台）
- 默认 DRY-RUN，必须 `--execute` 才执行
- 旧文件**永不删除**，仅复制
- 自动生成 ChangeLog

## 新增文件清单

| # | 路径 | 行数/字节 | 来源 |
|---|------|---------|------|
| 1 | skills/cross-cutting/document-storage-skill.md | 54KB（重构）| A1 |
| 2 | skills/cross-cutting/project-assets-update-skill.md | 45KB（升级）| A2 |
| 3 | templates/design/prd-template.md | ~100 行 | A3 |
| 4 | templates/design/issue-template.md | ~60 行 | A3 |
| 5 | templates/design/ra-template.md | ~150 行 | A3 |
| 6 | skills/phase1-design/requirement-analysis-skill.md | 1056 行 | A4 |
| 7 | skills/phase1-design/dr-generate-skill.md | 1056 行 | A5 |
| 8 | skills/phase1-design/dr-review-skill.md | 1082 行 | A6 |
| 9 | skills/orchestration/ae-sdd-skill.md | +86 行（路由增强）| A7 |
| 10-18 | 9 个 SKILL 改造 | - | A8 |
| 19 | scripts/migrate-docs.mjs | 444 行 / 14.7KB | A8 |
| 20 | docs/migration-guide.md | 287 行 / 10.3KB | A8 |

**总计**：~+5000 行 / 9 个新文件 / 9 个文件改造

## 关键设计决策

1. **三阶段流程独立化**：requirement-analysis / dr-generate / dr-review 各自独立 SKILL（不合并到 prd-story-skill）
2. **关联性分析不加权**：业务 0/1 + 逻辑 0/1，命中即关联
3. **规模裁定一票否决制**：架构决策=4 或数据变更=4 直接升"大"
4. **8 维度并行挖掘**：不容许杜撰、不容许有歧义
5. **4 维判定融合而非替换**：保留旧 4 类需求为 fallback
6. **存量迁移准备就绪**：默认 DRY-RUN，由用户决定时机

## 兼容性保证

| 维度 | 保证 |
|------|------|
| 流程/章节/标准/门禁 | ✅ 所有 SKILL 核心功能未触动 |
| 旧路径引用 | ✅ 9 个 SKILL 已改为 API 调用 |
| 关联性算法 | ✅ 与 document-storage §6 完全同步 |
| 项目资产索引 | ✅ 与 project-assets-update §A-G 完全同步 |
| 旧文件 | ✅ 迁移工具永不删除旧文件 |

## 用户后续动作

1. **运行迁移**（按用户决定时机）：`node scripts/migrate-docs.mjs --target . --execute`
2. **同步到插件副本**：`D:\Item\ae-sdd\scripts\sync-to-plugin.sh`（自动）
3. **CHANGELOG 维护**：本变更按 ae-sdd-update-skill §步骤 4.5 登记

## 后续规划

- 进一步增强 requirement-analysis 多轮对话能力
- 完善关联性分析（接入项目资产生成的关键词）
- 在 ae-sdd-update-skill 中加 4 维判定步骤

## Reviewer

用户
