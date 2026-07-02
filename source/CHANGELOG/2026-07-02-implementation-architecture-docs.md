# 2026-07-02 实现架构文档分层

## 变更摘要

新增 ae-sdd 实现架构说明书，并将 runtime stats 性能优化方案归档到 plans，避免继续把代码实现细节堆入系统能力说明书。

## 详细变更

- 新增 `source/docs/ae-sdd-implementation-architecture.md`
  - 明确能力设计文档、实现架构文档、plans、CHANGELOG 的职责边界。
  - 梳理 CLI、tools/lib、scripts、state/cache、build/distribution、gate/scanner、runtime stats 的实现分层。
  - 增加"新实现设计写入规则"，防止设计层与实现层继续漂移。
- 新增 `source/docs/plans/2026-07-02-runtime-stats-performance-plan.md`
  - 归档 runtime stats 与性能瓶颈优化方案。
  - 保留当前基线、目标、P0~P3 实施路径和验证方式。
- 更新 `source/docs/ae-sdd-design.md`
  - 将文档定位从"设计+实现全量映射"调整为能力设计入口。
  - 指向新的实现架构说明书。
- 更新 `README.md`
  - 详细文档列表增加系统能力说明书、实现架构说明书和 runtime 编译说明。

## 触发原因

ae-sdd 实现复杂度已从文档型 SKILL 演进为 CLI、门禁、扫描器、hook、runtime compiler、分发器等多子系统协作。继续把实现细节塞进 `ae-sdd-design.md` 会导致能力设计与代码实现互相污染，且更容易出现文档层与实现层漂移。

## 影响范围

- 文档结构调整，不改变运行时代码。
- 不修改 `dist/` 或本地安装副本。

## 验证方式

- 人工核对新增文档链接与职责边界。
- 后续如实现 runtime stats，需要按 `ae-sdd-implementation-architecture.md` 的落点规则同步 tools/tests、update-check 和 CHANGELOG。

## Reviewer

Codex

