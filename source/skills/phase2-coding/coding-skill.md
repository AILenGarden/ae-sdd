---
name: coding
description: 代码生成能力库（v3.6.1 适配器注册加载）。提供"如何写对代码"的知识：代码设计决策方法（11维CodingModel/骨架展开/分层红线）、CodeAnalysis 方法论（④bis SOP）、编码检查清单、禁止事项红线、验证判定标准、静态扫描规则。本文件是能力库，被 coding-process-skill.md 调用，不持有任何流程编排。当需要"如何生成代码/代码规范/代码设计决策/复用判断/分层职责判定"时调用本能力库。🆕 v3.6.1 新增 §13 语言/项目适配器注册加载：按项目技术栈叠加语言/项目特有编码决策（如 Java3D 适配器），纯规则仍归项目 constraints/+assets，本库不复述。
source_slimmed: true
source_slim_schema: ae-sdd-source-slim/v2
source_slim_standard: standards/skill-source-slimming-standard.md
source_slim_template: templates/skill/source-skill-slim-entry-template.md
source_fallback: skill-fallbacks/skills/phase2-coding/coding-skill.full.md
source_fallback_sha256: 675b8388db05683d3aff659648567fbbdb2a3502feb085a45a1b3d61443ab2d8
source_original_bytes: 42695
source_original_lines: 680
source_semantic_inventory_sha256: c52d2e90327792cd5c95a8bae20f45c28d182afbdf88c5c8516c9d03bcfe699a
source_slimmer: slim_source_skills.py@2
---

# CodingSKILL — 代码生成能力库（被调用，非流程节点） Source SKILL Slim Entry

This source SKILL has been slimmed by the standard source-slimming pipeline. The full pre-slim source is preserved at `skill-fallbacks/skills/phase2-coding/coding-skill.full.md` and remains the semantic fallback.

## Load Contract

- Use this slim entry first for routing, scope, semantic inventory, and resource discovery.
- Load `skill-fallbacks/skills/phase2-coding/coding-skill.full.md` before executing any step whose exact wording is not represented in the semantic inventory below.
- Do not run source slimming again when `source_slimmed: true` is present; use `--upgrade` only to re-render from the fallback with a newer schema.
- When compiling, runtime fallback must come from `skill-fallbacks/skills/phase2-coding/coding-skill.full.md`, not this slim entry.

## Summary

- source: `skills/phase2-coding/coding-skill.md`
- fallback: `skill-fallbacks/skills/phase2-coding/coding-skill.full.md`
- fallback_sha256: `675b8388db05683d3aff659648567fbbdb2a3502feb085a45a1b3d61443ab2d8`
- original_lines: 680
- original_bytes: 42695
- semantic_inventory_sha256: `c52d2e90327792cd5c95a8bae20f45c28d182afbdf88c5c8516c9d03bcfe699a`
- standard: `standards/skill-source-slimming-standard.md`
- template: `templates/skill/source-skill-slim-entry-template.md`
- summary: 代码生成能力库（v3.6.1 适配器注册加载）。提供"如何写对代码"的知识：代码设计决策方法（11维CodingModel/骨架展开/分层红线）、CodeAnalysis 方法论（④bis SOP）、编码检查清单、禁止事项红线、验证判定标准、静态扫描规则。本文件是能力库，被 coding-process-skill.md 调用，不持有任何流程编排。当需要"如何生成代码/代码规范/代码设计决策/复用判断/分层职责判定"时调用本能力库。🆕 v3.6.1 新增 §13 语言/项目适配器注册加载：按项目技术栈叠加语言/项目特有编码决策（如 Java3D 适配器），纯规则仍归项目 constraints/+assets，本库不复述。

## Semantic Inventory

| category | evidence | design_refs | fallback_policy |
| --- | --- | --- | --- |
| identity_trigger | frontmatter: name, description; headings: L1:6 CodingSKILL — 代码生成能力库（被调用，非流程节点）; L3:595 §13.1 加载协议 SOP（调用方执行，典型在 CodingProcess §A1 加载上下文时）; L3:656 §13.3 调用方与触发时机; keyword_hits: 29 | source/docs/ae-sdd-design.md §2/§16/§18; source/docs/skill-runtime-compiler.md §2 | Keep frontmatter and summary in the slim entry; full trigger wording stays in fallback. |
| workflow_route | headings: L1:6 CodingSKILL — 代码生成能力库（被调用，非流程节点）; L2:377 §8 验证判定标准（Execute 阶段怎么算验证通过）; L3:411 §8.3 主流程接口测试; keyword_hits: 36 | source/docs/ae-sdd-design.md §2/§16; source/standards/update-graph.json | Index the route/workflow outline; load fallback before executing low-frequency branch detail. |
| gate_constraint | headings: L3:140 §5.0 风险预判（必须先于 7 章节执行）; L3:164 §5.1 CodePlan 必须包含的 7 个章节; L3:256 §5.2 CodePlan 门禁（未通过禁止进入 Execute）; +6 more; keyword_hits: 120 | source/docs/ae-sdd-design.md §5; tools/lib/gates.py:GATE_REGISTRY | Preserve gate identifiers in index; CLI gate output remains higher authority than prose. |
| tool_command | headings: L3:411 §8.3 主流程接口测试; L1:575 Eclipse: Source → Organize Imports; keyword_hits: 40 | source/docs/ae-sdd-implementation-architecture.md §4/§5; source/docs/ae-sdd-design.md §13 | Index command/API references; full invocation contracts stay in fallback or implementation docs. |
| state_data | headings: L4:206 章节 3：数据结构 / DO 字段; keyword_hits: 34 | source/docs/ae-sdd-design.md §3/§15/§19; tools/lib/state.py | Index state/config vocabulary; use tools/lib state output as execution truth. |
| output_doc_contract | headings: L2:136 §5 CodeAnalysis ④bis：CodePlan 输出（CodeAnalysis 能力本体）; L4:328 要素 5：输出类骨架; L1:571 期望输出为空；非空 → 修改为已 import 的短名; keyword_hits: 46 | source/docs/ae-sdd-design.md §7; source/templates/** | Index document/output obligations; load fallback before generating exact long-form artifacts. |
| resource_reference | inline_refs: 29; refs: ../../standards/thinking/be-coding-thinking-engine.md §1.4 风险预判·11维度; ../../templates/coding/be-coding-plan-template.md; ae-sdd assets query "<name>"; +26 more; keyword_hits: 32 | source/standards/**; source/templates/**; source/skills/** | Preserve referenced paths in the slim entry; copied fallback remains the semantic anchor. |
| design_alignment | headings: L2:21 §0 能力总览（本库提供的能力清单）; L2:41 §1 CodingModel 决策方法（11 维）; L2:72 §2 约束文件引用（9 项关键规则）; +34 more; keyword_hits: 181 | source/docs/ae-sdd-design.md; source/docs/ae-sdd-implementation-architecture.md; source/docs/skill-runtime-compiler.md | Index the alignment surface; update design docs before changing behavior. |
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
| 1 | 6 | CodingSKILL — 代码生成能力库（被调用，非流程节点） |
| 2 | 21 | §0 能力总览（本库提供的能力清单） |
| 2 | 41 | §1 CodingModel 决策方法（11 维） |
| 2 | 72 | §2 约束文件引用（9 项关键规则） |
| 2 | 90 | §3 分层职责红线（写代码时反复对照，违反即阻断） |
| 2 | 109 | §4 骨架展开规则（Task 伪代码 → 完整方法体） |
| 2 | 136 | §5 CodeAnalysis ④bis：CodePlan 输出（CodeAnalysis 能力本体） |
| 3 | 140 | §5.0 风险预判（必须先于 7 章节执行） |
| 3 | 164 | §5.1 CodePlan 必须包含的 7 个章节 |
| 4 | 166 | 章节 1：文件级实现顺序 |
| 4 | 181 | 章节 2：关键类骨架 |
| 4 | 206 | 章节 3：数据结构 / DO 字段 |
| 4 | 214 | 章节 4：Mapper / Repository 关键 SQL |
| 4 | 223 | 章节 5：测试用例的对应实现 |
| 4 | 234 | 章节 6：编译与测试验证点 |
| 4 | 246 | 章节 7：调试与回滚策略 |
| 3 | 256 | §5.2 CodePlan 门禁（未通过禁止进入 Execute） |
| 2 | 275 | §6 CodeAnalysis ④bis 实战 SOP：分层拆分 + 项目资产映射方法论 |
| 3 | 284 | 核心设计哲学 |
| 3 | 290 | 方法论要素（CodeAnalysis 必须覆盖） |
| 4 | 292 | 要素 1：读取项目资产 |
| 4 | 298 | 要素 2：Task 执行顺序编排（不重写实现） |
| 4 | 304 | 要素 3：按抽象 4 层对每个 Task 做分层归类（🔴 核心） |
| 4 | 322 | 要素 4：把每个类按项目分层映射到确切包路径 |
| 4 | 328 | 要素 5：输出类骨架 |
| 3 | 332 | 7 条禁令（🔴 任何一条违反 = 整 Plan 打回） |
| 2 | 346 | §7 G-CODEPLAN-SRC 源码核对判定标准（🆕 v3.4.0） |
| 2 | 377 | §8 验证判定标准（Execute 阶段怎么算验证通过） |
| 3 | 381 | §8.1 编译验证 |
| 3 | 390 | §8.2 服务启动验证 |
| 3 | 411 | §8.3 主流程接口测试 |
| 3 | 420 | §8.4 错误码映射验证 |
| 3 | 429 | §8.5 DB 写操作落库验证 |
| 3 | 436 | §8.6 事务边界验证 |
| 2 | 442 | §9 编码后漂移核查 + 假修复识别 |
| 3 | 444 | §9.1 全切面一致性核查（漂移判定） |
| 3 | 469 | §9.2 假修复识别规则（测试造假判定） |
| 2 | 485 | §10 异常根因 4 层分类判定 |
| 2 | 505 | §11 经验检查清单 + 禁止事项红线 |
| 3 | 507 | §11.1 经验检查清单（通用，每次生成代码前逐项确认） |
| 3 | 529 | §11.2 禁止事项红线 |
| 3 | 550 | §11.3 基准过滤器自检（7 项，参考 be-coding-thinking-engine） |
| 2 | 562 | §12 静态扫描规则（通用 grep，Execute 编码后必跑） |
| 1 | 567 | 1. 标准库全限定名扫描（除 import 块外不应出现） |
| 1 | 571 | 期望输出为空；非空 → 修改为已 import 的短名 |
| 1 | 573 | 2. 未使用 import 扫描（IDE 警告即可） |
| 1 | 574 | IntelliJ: Code → Optimize Imports |
| 1 | 575 | Eclipse: Source → Organize Imports |
| 1 | 577 | 3. 静态导入滥用扫描 |
| 1 | 579 | 项目中应有节制使用，不应过多 |
| 2 | 589 | §13 语言/项目适配器注册加载（🆕 v3.6.1） |
| 3 | 595 | §13.1 加载协议 SOP（调用方执行，典型在 CodingProcess §A1 加载上下文时） |
| 3 | 623 | §13.1bis 叠加视图速查表（🆕 v3.6.2 — 降低 AI 脑内合并负担） |
| 3 | 646 | §13.2 适配器契约（注册进注册表的语言/项目 SKILL 必须满足） |
| 3 | 656 | §13.3 调用方与触发时机 |
| 3 | 663 | §13.4 零破坏声明 |

## Inline References

| ref |
| --- |
| ../../standards/thinking/be-coding-thinking-engine.md §1.4 风险预判·11维度 |
| ../../templates/coding/be-coding-plan-template.md |
| ae-sdd assets query "<name>" |
| ae-sdd assets read coding --project <projectKey> |
| ae-sdd assets section §4 --project <projectKey> |
| ae-sdd gates check --only G-CODEPLAN-SRC |
| ae-sdd-plugin-loader-skill.md |
| application/XxxAppService.java |
| appservice/{Resource}AppService |
| be-coding-plan-template.md |
| be-coding-thinking-engine.md |
| code-review-skill.md |
| coding-process-skill.md |
| curl localhost:{port}/actuator/beans \| grep {本Story新增的Bean名} |
| curl localhost:{port}/actuator/health |
| domain/.../entity/ |
| domain/.../service/{Resource}DomainService |
| domain/entity/XxxAggregate.java |
| domain/repository/XxxRepository.java |
| entity/{Resource}DO.transition() |
| infrastructure/persistence/XxxMapper.java |
| infrastructure/persistence/XxxRepositoryIT.java |
| infrastructure/persistence/XxxRepositoryImpl.java |
| interfaces/XxxController.java |
| lessons-learned.md |
| plugins/java3d-coding-skill/SKILL.md |
| plugins/registry.yaml |
| standards/thinking/be-coding-thinking-engine.md |
| {STORY-ID}-CodingPlan.md |
