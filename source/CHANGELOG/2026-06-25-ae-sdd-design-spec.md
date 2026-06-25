# 2026-06-25 | ae-sdd v3.2.5-patch — 新增系统能力设计说明书 ae-sdd-design.md

## Summary

ae-sdd 体系此前缺少一份"系统有哪些能力、每项能力怎么设计实现"的说明文档。
维护者和 LLM Agent 在修改某项能力前，没有地方能快速理解设计意图，容易改错层（
如"review 强制多 Agent"应改 SKILL.md §角色库，而不是各子 reviewSkill 文字）。

本次新建 `source/docs/ae-sdd-design.md`，覆盖 12 个能力模块，每个模块包含：

- 是什么（1-2句定位）
- 设计实现（关键机制、数据结构、CLI 命令）
- 颗粒度与边界（精确粒度、限制、豁免条件）

## Changes

| Area | Change |
| --- | --- |
| `source/docs/ae-sdd-design.md` | **新建**：12 个能力模块，252 行（端到端流程编排 / 智能路由 / 状态持久化 / 多 Agent 编排 / 门禁体系 / 项目资产体系 / 实例化体系 / Harness 适配层 / 记忆层 / Plan-First 编排 / 真实性扫描 / 工具链 CLI） |
| `ae-sdd-update-skill.md §步骤1` | 前置块文案更新：从"变更导航检查"改为"设计意图确认"，引用指向能力说明书 |
| `ae-sdd-update-skill.md §更新依赖图谱` | 前置块文案更新：同上 |
| `ae-sdd-update-skill.md §健康度清单` | v3.2.5 三条检查项更新为能力模块数量（≥12）和两处引用存在 |

## 触发原因

用户要求补充 ae-sdd 的设计说明稿，列出大小设定和实现方式，避免设计不统一、实现不统一。
重点例子：流程编排中包含哪些能力（自动分配 Agent、智能路由、review 多视角）、流程 state 管理的实现方式和颗粒度、实例化脚本如何将 doc 实例化为 harness 等。

## 影响范围

- 纯文档新建，不影响运行时逻辑、门禁行为、CLI 命令
- ae-sdd-update-skill 行为语义不变，只更新前置检查提示文案

## 验证方式

- 人工核对 ae-sdd-design.md 12 个能力模块均有完整的是什么 / 设计实现 / 颗粒度说明
- 人工核对 ae-sdd-update-skill 两处引用已更新为新文档定位
- 健康度清单新增条目可勾选

## Reviewer

陈聪
