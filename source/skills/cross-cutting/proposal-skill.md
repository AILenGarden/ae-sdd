---
name: proposal
description: 建议书（Proposal）SKILL — 统一所有"问题描述 + 解决方案"载体。覆盖多渠道（Code Review / Story Review / Coding 异常 / Project Assets 漂移 / 用户反馈 / 生产事故 / Test 发现），用统一的 4 段结构（原本是怎么样/要做什么/怎么做/涉及范围）走流程：改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test。🆕 2026-06-06 新建，解决"问题描述模式可以很多种但最终执行流程不变"的核心洞察。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/cross-cutting/proposal-skill.full.md
source_fallback_sha256: a14ce895782b9d13da399471f6a0cb52bf5227f26c95f80a4f746b5b4d2a37c0
source_original_bytes: 30302
source_original_lines: 671
source_semantic_inventory_sha256: 2f5e4b7204030d69893bedc68d6e69254caabfcf9908ca722f6f112768bbe105
---

# Proposal — 建议书 SKILL（统一问题描述 + 解决方案载体） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/cross-cutting/proposal-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/cross-cutting/proposal-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/cross-cutting/proposal-skill.full.md`, not this slim entry.

## Summary

- source: `skills/cross-cutting/proposal-skill.md`
- fallback: `skill-fallbacks/skills/cross-cutting/proposal-skill.full.md`
- fallback_sha256: `a14ce895782b9d13da399471f6a0cb52bf5227f26c95f80a4f746b5b4d2a37c0`
- original_lines: 671
- original_bytes: 30302
- semantic_inventory_sha256: `2f5e4b7204030d69893bedc68d6e69254caabfcf9908ca722f6f112768bbe105`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 建议书（Proposal）SKILL — 统一所有"问题描述 + 解决方案"载体。覆盖多渠道（Code Review / Story Review / Coding 异常 / Project Assets 漂移 / 用户反馈 / 生产事故 / Test 发现），用统一的 4 段结构（原本是怎么样/要做什么/怎么做/涉及范围）走流程：改 Story → 改 TestCase → 改 Task → 改 Coding → 改 Test。🆕 2026-06-06 新建，解决"问题描述模式可以很多种但最终执行流程不变"的核心洞察。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L2:26 📦 文档存放前置调用（🔴 横切依赖）; L2:126 触发条件; L3:498 渠道 3：Coding 异常追溯链 → 触发 Proposal; +1 more; keyword_hits: 52 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L2:88 整体流程; L2:314 第五步：用 Proposal 走流程（🔴 核心）; L3:318 5.1 5 步流程总览; keyword_hits: 33 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:75 标尺 3：可执行性（🔴 §3 必须有具体步骤）; L3:80 标尺 4：影响范围明示（🔴 §4 必须列下游动作）; L2:623 禁止事项; keyword_hits: 34 | source/docs/ae-sdd-design.md §5; crates/ae-sdd-gates/src/registry.rs:GateRegistry | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | keyword_hits: 21 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | keyword_hits: 10 | source/docs/ae-sdd-design.md §3/§15/§19; crates/ae-sdd-store/src (StateAuthority) | Index state/config vocabulary; use CLI state output as execution truth. |
| output_doc_contract | headings: L2:26 📦 文档存放前置调用（🔴 横切依赖）; L2:280 第四步：写入 Proposal 文档; L3:509 追溯层 1 命中：Task 文档缺陷; +1 more; keyword_hits: 49 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 47; refs: GET /boss-user-bff/api/v1/users/list?roleId=0; SKILL.md; STORY-001-BE-CodeReview-v1-r1.md; +44 more; keyword_hits: 101 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L3:75 标尺 3：可执行性（🔴 §3 必须有具体步骤）; L3:80 标尺 4：影响范围明示（🔴 §4 必须列下游动作）; L3:166 §1 原本是怎么样（现状）; +4 more; keyword_hits: 160 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
| fallback_only_detail | keyword_hits: 11 | source/skill-fallbacks/**; source/CHANGELOG/** | Do not summarize aggressively; keep only the location signal and rely on fallback for exact detail. |

## Source Slimming SOP

1. Read the full source or the recorded fallback as the only semantic input.
2. Identify semantic categories before slimming: identity/trigger, workflow/route, gates/constraints, tools/API, state/data, output contracts, resources, design alignment, and fallback-only detail.
3. Render this entry from `templates/skill/source-skill-slim-entry-template.md`; do not hand-edit generated slim sections.
4. Validate `source_fallback_sha256`, required sections, and `source_semantic_inventory_sha256`.
5. Rebuild compiled runtime and run runtime verification after any source SKILL slimming change.

## Headings

| level | line | title |
| --- | --- | --- |
| 1 | 6 | Proposal — 建议书 SKILL（统一问题描述 + 解决方案载体） |
| 2 | 26 | 📦 文档存放前置调用（🔴 横切依赖） |
| 2 | 48 | 0. 目标 |
| 2 | 58 | Proposal 总则（🔴 贯穿全 SKILL，违反 = 结论无效） |
| 3 | 60 | 标尺 1：完整性（🔴 4 段必填） |
| 3 | 69 | 标尺 2：单一权威源（🔴 一次写完，多处引用） |
| 3 | 75 | 标尺 3：可执行性（🔴 §3 必须有具体步骤） |
| 3 | 80 | 标尺 4：影响范围明示（🔴 §4 必须列下游动作） |
| 2 | 88 | 整体流程 |
| 2 | 126 | 触发条件 |
| 2 | 141 | 第一步：识别渠道 |
| 2 | 162 | 第二步：填写 Proposal 4 段 |
| 3 | 166 | §1 原本是怎么样（现状） |
| 3 | 185 | §2 要做什么（目标/新需求） |
| 3 | 208 | §3 怎么做（方案/影响分析/步骤拆解） |
| 3 | 252 | §4 涉及范围（下游 5 类动作） |
| 2 | 269 | 第三步：合理性自检 |
| 2 | 280 | 第四步：写入 Proposal 文档 |
| 2 | 314 | 第五步：用 Proposal 走流程（🔴 核心） |
| 3 | 318 | 5.1 5 步流程总览 |
| 3 | 338 | 5.2 各下游 SKILL 接收 Proposal 的方式 |
| 3 | 348 | 5.3 Proposal 与下游 UpdatePlan 的关系 |
| 3 | 363 | 5.4 下游 SKILL 的"携带 Proposal"机制 |
| 3 | 376 | 5.5 Proposal 多源合并（可选） |
| 2 | 393 | 第六步：循环判定 |
| 2 | 414 | 第七步：Proposal 闸门全集 |
| 2 | 430 | 第八步：Proposal 生命周期 |
| 2 | 451 | 多渠道接入设计（🆕 2026-06-06 关键设计） |
| 3 | 453 | 渠道 1：Code Review 评审发现 → 自动生成 Proposal |
| 3 | 476 | 渠道 2：Story Review 评审发现 → 自动生成 Proposal |
| 3 | 498 | 渠道 3：Coding 异常追溯链 → 触发 Proposal |
| 3 | 509 | 追溯层 1 命中：Task 文档缺陷 |
| 3 | 515 | 追溯层 1 命中：Task 文档缺陷 |
| 3 | 530 | 渠道 4：Project Assets 漂移 → 触发 Proposal |
| 4 | 542 | 步骤 3：增量更新项目资产对应章节 |
| 4 | 548 | 步骤 3：🔴 先生成 Proposal（渠道 4：Project Assets 漂移） |
| 3 | 557 | 渠道 5：用户反馈 → 手动写 Proposal |
| 3 | 566 | 渠道 6：生产事故 → 手动写 Proposal |
| 3 | 578 | 渠道 7：Test 发现 → 自动生成 Proposal |
| 2 | 606 | 与其他 SKILL 的衔接 |
| 2 | 623 | 禁止事项 |
| 2 | 638 | 执行清单 |
| 2 | 657 | 维护 |

## Inline References

| ref |
| --- |
| GET /boss-user-bff/api/v1/users/list?roleId=0 |
| SKILL.md |
| STORY-001-BE-CodeReview-v1-r1.md |
| STORY-001-BE-CodingReport-v1-r1.md |
| STORY-001-BE-Report-v1-r1.md |
| STORY-001-BE-testcase.md |
| STORY-001-BE.md |
| STORY-001-BE.md §AC 验收标准 |
| ae-sdd doc save --intent PROPOSAL --story-id STORY-001-BE --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL --story-id cross-story --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL --story-id project --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL --story-id {STORY-ID} --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL --story-id {S} --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL ... |
| ae-sdd doc save --intent PROPOSAL_ARCHIVE --story-id cross-story --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL_ARCHIVE --story-id project --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL_ARCHIVE --story-id {STORY-ID or 类别} --content-file 草稿.md |
| ae-sdd doc save --intent PROPOSAL_ARCHIVE --story-id {STORY-ID} --content-file 草稿.md |
| ae-sdd-update-skill.md |
| appservice/BossUserAppService.java |
| appservice/BossUserAppService.java:88 |
| appservice/BossUserAppService.java:88-110 |
| code-review-skill.md |
| code-review-skill.md §第四步 bis |
| coding-report-skill.md |
| coding-report-skill.md §四 |
| coding-report-skill.md §四 测试结果 |
| coding-skill.md |
| coding-skill.md §异常路径 |
| coding-skill.md §异常路径：Coding 实时追溯链 |
| design/proposal/ |
| document-storage-skill.md |
| document-storage-skill.md §1.3 路径模板 |
| if (query.getRoleId() != null && query.getRoleId() == 0) { /* 不过滤 */ } else if (query.getRoleId() != null && query.getRoleId() > 0) { /* 过滤 */ } else if (query.getRoleId() != null && query.getRoleId() < 0) { throw new BusinessException(10001); } |
| project-assets-update-skill.md |
| project-assets-update-skill.md §4 动作 2 |
| project-assets-update-skill.md §4.2 步骤 3 |
| proposal-skill.md |
| proposal-skill.md §第二步 |
| proposal-skill.md §第五步 |
| review-loop-skill.md |
| story-review-skill.md |
| story-update-skill.md |
| task-5-BossUserAppServiceList.md |
| task-generate-skill.md |
| templates/proposal/proposal-template.md |
| testcase-generate-skill.md |
