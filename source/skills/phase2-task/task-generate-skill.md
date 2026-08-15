---
name: task-generate
description: 根据 Story 中的 Task 描述和约束文档，生成或更新 Task 实现文档。当 Story Update SKILL 修改了 Task 列表时自动触发，或开发者说"生成 Task"、"写 Task 文档"时触发。生成完成后触发 Coding SKILL。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase2-task/task-generate-skill.full.md
source_fallback_sha256: cbf320e3dbb8cf6b13ded105ad94242cc801e391c4989a5b4564b8a885ebdf97
source_original_bytes: 46092
source_original_lines: 776
source_semantic_inventory_sha256: 8a5c438aaf7b23316905ad7c6903ed3d174fd6e83a903c3df241c98fe37467dc
---

# Task Generate — Task 文档生成 Skill Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase2-task/task-generate-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase2-task/task-generate-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase2-task/task-generate-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase2-task/task-generate-skill.md`
- fallback: `skill-fallbacks/skills/phase2-task/task-generate-skill.full.md`
- fallback_sha256: `cbf320e3dbb8cf6b13ded105ad94242cc801e391c4989a5b4564b8a885ebdf97`
- original_lines: 776
- original_bytes: 46092
- semantic_inventory_sha256: `8a5c438aaf7b23316905ad7c6903ed3d174fd6e83a903c3df241c98fe37467dc`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 根据 Story 中的 Task 描述和约束文档，生成或更新 Task 实现文档。当 Story Update SKILL 修改了 Task 列表时自动触发，或开发者说"生成 Task"、"写 Task 文档"时触发。生成完成后触发 Coding SKILL。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:28 📦 文档存放前置调用（🔴 横切依赖）; L2:63 🧠 阶段记忆强制调用（🔴 横切依赖）; L2:129 触发条件; +3 more; keyword_hits: 52 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:63 🧠 阶段记忆强制调用（🔴 横切依赖）; L2:78 整体流程; L3:482 Task 修复流程; +1 more; keyword_hits: 39 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L2:14 🟠 门禁强度声明（v3.5.11 AA 诚实降级 / v3.9.1 部分硬化）; L2:180 第一步半：前置依赖检查（门禁）; L2:307 第四步 bis：生成后一致性校验（强制门禁）; +2 more; keyword_hits: 127 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 66 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 25 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L1:6 Task Generate — Task 文档生成 Skill; L2:28 📦 文档存放前置调用（🔴 横切依赖）; L3:47 本 SKILL 产出文档 × intent 对照; +9 more; keyword_hits: 157 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 30; refs: .ae-sdd/tmp/{doc-id}-draft.md; SKILL.md §实现方案决策基线; ae-sdd assets query "<name>"; +27 more; keyword_hits: 50 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:372 多 reviewer 视角切分（🆕 2026-06-25 — 落地 §8.4.2 节点专属配置）; L2:496 第六步 bis：输出完整实现方案; keyword_hits: 139 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 26 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Task Generate — Task 文档生成 Skill |
| 2 | 8 | 目标 |
| 2 | 14 | 🟠 门禁强度声明（v3.5.11 AA 诚实降级 / v3.9.1 部分硬化） |
| 2 | 28 | 📦 文档存放前置调用（🔴 横切依赖） |
| 3 | 32 | 写入 SOP（3 步） |
| 3 | 47 | 本 SKILL 产出文档 × intent 对照 |
| 2 | 63 | 🧠 阶段记忆强制调用（🔴 横切依赖） |
| 1 | 69 | 生成 Task / 统一 CodingPlan |
| 2 | 78 | 整体流程 |
| 2 | 129 | 触发条件 |
| 2 | 139 | 第一步：读取输入（🆕 2026-06-10 加"是否有 Story"分支） |
| 3 | 143 | 1.A 有 Story 上级文档（中/大任务场景） |
| 3 | 152 | 1.B 无 Story 上级文档（由权威路线判定，非独立入口） |
| 2 | 180 | 第一步半：前置依赖检查（门禁） |
| 2 | 218 | 第二步：读取约束文档 + Task 模板 |
| 3 | 220 | 2.1 约束文档 |
| 3 | 235 | 2.2 Task 模板 |
| 2 | 241 | 第三步：判断新增/更新 |
| 2 | 252 | 第四步：生成/更新 Task 文档 |
| 3 | 254 | 4.0 存放路径 |
| 3 | 272 | 4.1 生成规则 |
| 3 | 282 | 4.2 骨架生成要求 |
| 3 | 297 | 4.3 检查项生成要求 |
| 2 | 307 | 第四步 bis：生成后一致性校验（强制门禁） |
| 3 | 311 | 校验清单 |
| 2 | 331 | 第五步：生成/更新 Task 0 |
| 2 | 343 | 第五步 bis：全局 Task Review（强制闭环） |
| 3 | 372 | 多 reviewer 视角切分（🆕 2026-06-25 — 落地 §8.4.2 节点专属配置） |
| 3 | 400 | Review 检查清单 |
| 3 | 460 | Review 结论产出 |
| 3 | 482 | Task 修复流程 |
| 2 | 496 | 第六步 bis：输出完整实现方案 |
| 2 | 538 | 第四步 ter（🔴 v3.5.16 编排动作已移交 CodingProcess） |
| 3 | 548 | 调用参数 |
| 3 | 559 | `CodingSkill.Plan(task-level)` 必须返回 |
| 3 | 571 | TaskSkill 职责边界（🔴 强制） |
| 2 | 611 | 第六步：汇总统一版 CodePlan（🔴 v3.5.16 编排动作已移交 CodingProcess） |
| 2 | 664 | 第七步：触发 `CodingSkill.Execute`（⑤ Coding 阶段） |
| 2 | 683 | 禁止事项 |
| 2 | 701 | 执行清单（逐项执行，不可跳过） |
| 2 | 725 | 📖 人工审核主动讲解规范 — Task 节点 |

## Inline References

| ref |
| --- |
| .ae-sdd/tmp/{doc-id}-draft.md |
| SKILL.md §实现方案决策基线 |
| ae-sdd assets query "<name>" |
| ae-sdd assets read task-generate --project <projectKey> |
| ae-sdd doc resolve --intent TASK --work-item BUG-LIFE-001 --doc-id task-1-BossUserQuery |
| ae-sdd doc resolve --intent TASK --work-item {W} --story-id {S?} --doc-id {taskId} |
| ae-sdd doc save |
| ae-sdd doc save --intent CODING_PLAN --work-item {W} --story-id {S?} --content-file 草稿.md --changelog-note "..." |
| ae-sdd doc save --intent CODING_PLAN --work-item {W} --story-id {S?} ... |
| ae-sdd doc save --intent TASK --work-item {W} --story-id {S?} --doc-id task-0-公共依赖说明 ... |
| ae-sdd doc save --intent TASK --work-item {W} --story-id {S?} --doc-id {taskId} ... |
| ae-sdd doc save --intent TASK_IMPL_PLAN --work-item {W} --story-id {S?} ... |
| ae-sdd doc save --intent TASK_REVIEW --work-item {W} --story-id {S?} ... |
| ae-sdd doc save --intent TASK_SUPPLEMENT --work-item {W} --story-id {S?} ... |
| ae-sdd doc save --intent TASK_WRITER_REPORT --work-item {W} --story-id {S?} ... |
| ae-sdd update-check |
| agent-orchestration-skill.md §8.4 多 reviewer 默认编排框架 |
| constraints/ |
| constraints/assets/Story/TestCase |
| get_constraints/get_assets |
| review-loop-skill.md |
| standards/thinking/be-coding-thinking-engine.md |
| task-{N}-0-公共依赖说明.md |
| task-{N}-{X}-*.md |
| templates/design/be-task-implementation-plan-template.md |
| templates/design/be-task-template.md |
| crates/ae-sdd-gates/src/registry.rs |
| {STORY-ID}-CodingPlan.md |
| {STORY-ID}-Task实现方案.md |
| {WORKITEM-ID}-CodingPlan.md |
