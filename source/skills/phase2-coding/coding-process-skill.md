---
name: coding-process
description: |
  Story/TestCase->CodingPlan->Coding 全流程编排节点（v3.10.0 砍 Task 后）。持有
  CodePlan->Coding->验证->异常追溯全流程编排：加载4上下文 -> 骨架分解 -> 调用 coding-skill
  能力库做 CodeAnalysis -> 出具 CodePlan -> 按 CodePlan 写代码(Execute) -> 编译/测试/异常追溯。
  本 SKILL 是流程节点，调 coding-skill 能力库（如何写对代码的知识），不持有能力本体。
  当 state.phase 走到 coding-process 或 coding 时触发；用户说“开始 Coding/写代码/实现 Story/
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase2-coding/coding-process-skill.full.md
source_fallback_sha256: 8642efbb585819f1fff9233561e931516081b4c61e49cdc2e433a5e79d62955a
source_original_bytes: 32750
source_original_lines: 538
source_semantic_inventory_sha256: 74177d5c4b1ba2f4f068265bab22ccfc300d6e45a507548565901105f201fae4
---

# CodingProcess - CodePlan->Coding 全流程编排节点（流程与能力分离） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase2-coding/coding-process-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase2-coding/coding-process-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase2-coding/coding-process-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase2-coding/coding-process-skill.md`
- fallback: `skill-fallbacks/skills/phase2-coding/coding-process-skill.full.md`
- fallback_sha256: `8642efbb585819f1fff9233561e931516081b4c61e49cdc2e433a5e79d62955a`
- original_lines: 538
- original_bytes: 32750
- semantic_inventory_sha256: `74177d5c4b1ba2f4f068265bab22ccfc300d6e45a507548565901105f201fae4`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: Story/TestCase->CodingPlan->Coding 全流程编排节点（v3.10.0 砍 Task 后）。持有
CodePlan->Coding->验证->异常追溯全流程编排：加载4上下文 -> 骨架分解 -> 调用 coding-skill
能力库做 CodeAnalysis -> 出具 CodePlan -> 按 CodePlan 写代码(Execute) -> 编译/测试/异常追溯。
本 SKILL 是流程节点，调 coding-skill 能力库（如何写对代码的知识），不持有能力本体。
当 state.phase 走到 coding-process 或 coding 时触发；用户说“开始 Coding/写代码/实现 Story/

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:21 📦 文档存放前置调用（🔴 横切依赖）; L2:40 🧠 阶段记忆强制调用（🔴 横切依赖）; L3:137 §A2 调用 coding-skill 能力做 CodeAnalysis; +1 more; keyword_hits: 33 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L1:11 CodingProcess - CodePlan->Coding 全流程编排节点（流程与能力分离）; L2:40 🧠 阶段记忆强制调用（🔴 横切依赖）; L2:93 Phase A：CodeAnalysis（产出 CodePlan）; +3 more; keyword_hits: 74 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:137 §A2 调用 coding-skill 能力做 CodeAnalysis; L3:146 §A3 产出统一版 CodePlan + 跑门禁; L3:177 §B0 第零步：复核 CodingModel（Execute 入口门禁）; +2 more; keyword_hits: 145 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:299 §B4 编译 + 服务启动 + 接口验证 + DB 验证 + 异常验证; keyword_hits: 52 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L2:93 Phase A：CodeAnalysis（产出 CodePlan）; L2:175 Phase B：Execute（按 CodePlan 写代码）; L2:356 Phase C：异常追溯（报错时实时追溯链 A1-A6）; keyword_hits: 44 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L2:21 📦 文档存放前置调用（🔴 横切依赖）; L2:93 Phase A：CodeAnalysis（产出 CodePlan）; L3:146 §A3 产出统一版 CodePlan + 跑门禁; keyword_hits: 75 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 27; refs: ../phase3-review/test-generate-skill.md; ../phase3-review/test-review-skill.md; /ae-sdd 优化这部分实现; +24 more; keyword_hits: 61 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:95 §A1 加载 4 上下文（🔴 强制，缺一停止）; L3:108 §A1.4 流程深度由权威 `EngineeringRoute` 决定，本 SKILL 不自行分流; L3:124 §A1.5 骨架分解（🆕 v3.10.0 从 task-generate-skill 合并）; +14 more; keyword_hits: 149 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | headings: L3:181 §B0.5 Spec 变更确认（🔴 强制，先于工程预检）; keyword_hits: 18 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 11 | CodingProcess - CodePlan->Coding 全流程编排节点（流程与能力分离） |
| 2 | 21 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 40 | 🧠 阶段记忆强制调用（🔴 横切依赖） |
| 2 | 62 | CodingProcess.Run 对外契约 |
| 2 | 93 | Phase A：CodeAnalysis（产出 CodePlan） |
| 3 | 95 | §A1 加载 4 上下文（🔴 强制，缺一停止） |
| 3 | 108 | §A1.4 流程深度由权威 `EngineeringRoute` 决定，本 SKILL 不自行分流 |
| 3 | 124 | §A1.5 骨架分解（🆕 v3.10.0 从 task-generate-skill 合并） |
| 3 | 137 | §A2 调用 coding-skill 能力做 CodeAnalysis |
| 3 | 146 | §A3 产出统一版 CodePlan + 跑门禁 |
| 3 | 160 | §A4 用户审核点 2.5（CodingPlan 评审，🔴 强制） |
| 2 | 175 | Phase B：Execute（按 CodePlan 写代码） |
| 3 | 177 | §B0 第零步：复核 CodingModel（Execute 入口门禁） |
| 3 | 181 | §B0.5 Spec 变更确认（🔴 强制，先于工程预检） |
| 3 | 216 | §B1 工程预检（第一~五步合并） |
| 3 | 231 | §B2 按骨架顺序生成代码（调 coding-skill 骨架展开能力） |
| 3 | 262 | §B3 实现方案确认（强制交互节点） |
| 3 | 299 | §B4 编译 + 服务启动 + 接口验证 + DB 验证 + 异常验证 |
| 3 | 321 | §B5 Test 系列交接 |
| 3 | 332 | §B6 编码后全切面一致性核查闸（CodeReview 硬前置） |
| 3 | 342 | §B7 静态扫描 + 交付表（编码收尾） |
| 2 | 356 | Phase C：异常追溯（报错时实时追溯链 A1-A6） |
| 3 | 394 | §C1 问题记录载体 |
| 3 | 400 | 问题 {序号}：{问题标题} |
| 3 | 414 | §C2 与流程衔接 |
| 2 | 425 | 📖 人工审核主动讲解规范 — Code 节点（审核点 4） |
| 2 | 479 | 执行清单（TodoWrite 1:1 映射，禁止跳过） |
| 2 | 512 | 完成标准 |
| 2 | 528 | 与其他 SKILL 的关系 |

## Inline References

| ref |
| --- |
| ../phase3-review/test-generate-skill.md |
| ../phase3-review/test-review-skill.md |
| /ae-sdd 优化这部分实现 |
| SKILL.md |
| ae-sdd doc resolve |
| ae-sdd doc resolve --intent STORY --story-id {S} |
| ae-sdd doc resolve --intent TESTCASE --work-item {W} --story-id {S?} |
| ae-sdd doc save |
| ae-sdd doc save --intent CODING_ISSUE_LOG --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent CODING_PLAN --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent CODING_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save --intent TEST_REPORT --work-item {W} --story-id {S?} --content-file 草稿.md |
| ae-sdd doc save/resolve |
| ae-sdd state confirm --phase coding-process |
| ae-sdd state confirm --phase spec-change --story {STORY-ID} |
| ae-sdd state write --phase coding |
| ae-sdd-doc/Coding/{WORKITEM-ID}/ |
| be-coding-plan-template.md |
| code-review-skill.md |
| coding-report-skill.md |
| coding-skill.md |
| document-storage-skill.md |
| src/ |
| story-review-skill.md |
| test-review-skill.md |
| testcase-review-skill.md |
| 新增/修改/删除/无改动/仅测试/仅文档 |
