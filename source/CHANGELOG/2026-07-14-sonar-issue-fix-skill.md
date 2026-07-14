# 2026-07-14 | Sonar Issue Fix 子 SKILL 与 CodeReview 收尾挂靠

## Summary

新增 Sonar 问题修复子 SKILL，把 issue 归一化、修复来源选择、最小 EditPlan、防陈旧/越界校验和 compile-test-rescan 验证收敛为单一流程。CodeReview 在循环收敛后、最终七道闸前每个评审会话恰好调用一次；Sonar 不可用返回 N/A，修复改动源码则重开受影响验证，但同一会话不递归二次调用。

## Changes

| Area | Change |
| --- | --- |
| `sonar-issue-fix-skill.md` | 新增 `upstream-edit` / `registry` / `reasoned` / `manual` 唯一分类、EditPlan、路径/hash/range/overlap/atomic 防护和验证闭环 |
| `sonar-issue-fix-rules.md` | 新增独立规则注册表；首版只启用 `java:S1128` 单条 unused import 删除配方，包含前置条件、负例、漂移和 license 策略 |
| `code-review-skill.md` | 新增第六步 bis 固定调用点、exactly-once 会话令牌、N/A 语义和源码变化后的验证重开协议 |
| 主入口/更新 SKILL | 新增子 SKILL 索引与职责边界；子 SKILL 计数 28 -> 29 |
| README/设计说明 | 更新 29 个子 SKILL、21 份标准和 Sonar 能力/复用/许可证边界 |
| tests | 新增源级契约测试，覆盖 Story TC-01 至 TC-19 的关键不变量 |

## Reuse And License Decision

- 复用 Sonar 官方 issue/rule/quality gate 输入，以及 SonarLint Core/IntelliJ 公开的 `TextEdit(range, newText)` 协议思想。
- IDEA quick-fix 灯泡或 `quickFix` 标志不是补丁 payload；没有实际 edits 时不得走 `upstream-edit`。
- 不复制或移植 SonarJava analyzer/quick-fix 实现。SonarJava 当前 `Sonar Source-Available License v1.0` 对 AI 摄取/解释源码存在明确边界，因此硬编码注册表只接受独立撰写、可由公开规则行为验证的配方。

## Architecture Impact

`source/docs/ae-sdd-implementation-architecture.md`：N/A。本次只增加 SKILL、规则文档、索引和契约测试，不新增 CLI、tools/lib、gate、state schema、scanner、hook、构建脚本或后台服务。系统能力语义已同步 `source/docs/ae-sdd-design.md`。

Monitor：N/A。没有新增 phase、state/memory/runtime-stats 字段或项目侧路径，Monitor 的只读投影协议不变。

## Verification

- TDD RED：新契约测试在实现前 9/9 失败，原因均为计划产物/集成点不存在。
- TDD GREEN：`python -m unittest tools.tests.test_sonar_issue_fix_skill`，9/9 通过。
- Source slim：本次变更的 root、update、CodeReview、Sonar 四个入口逐项校验通过；slimmer 自身单测 3/3 通过。全仓 `--validate` 仍报告 41 条本次范围外的既有 fallback/hash 漂移，未批量重写无关入口。
- Distribution：`python scripts/build_dist.py` 通过；`ae-sdd runtime verify --path dist/ae-sdd` 通过，只有既有 compact anchor warnings。
- Dependency graph：重建 harness 派生产物（`--no-mount`）后，`ae-sdd update-check` 18/18 通过，保留 4 条既有 warning。
