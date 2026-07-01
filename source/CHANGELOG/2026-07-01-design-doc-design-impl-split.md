# 2026-07-01 | ae-sdd v3.7.3 - 设计文档「设计/实现」分节重构 + 事实核对纠偏

## Summary

用户要求 `source/docs/ae-sdd-design.md` 按"设计"与"实现"分节重写，实现部分需用表格标注每个设计点对应的具体实现方式（文件:行号/函数名/CLI 命令/gate ID）。重写过程中系统性核对了文档全部量化声明与代码现状的一致性，发现并纠正多处文档滞后：门禁数量（26→29）、扫描器数量（3→6 扫描器+2 对齐工具）、CLI 子命令分类数（14→29）、状态机链长度（11/10/8/4→14/13/12/7）、harness 转换脚本（已从 `.ps1` 迁移到 `build_harness.py`）、以及"主流程监管器/流程偏移检测"两节被误标"🆕 v3.6 规划中"实为已完整实现并接入 UserPromptSubmit hook。

## Changes

| Area | Change |
| --- | --- |
| `source/docs/ae-sdd-design.md` | 全文重写：16 个无编号章节（标题+混排叙述）→ 17 个编号章节，每章统一拆为 `### 设计`（叙述）+ `### 实现`（表格：设计点/需求点 → 具体实现方式）+ `**颗粒度与边界**` 三段式 |
| §5 门禁体系 | 门禁总数 "26 个" 纠正为实际 29 个，补全此前遗漏的 G-09B、G-REVIEW-LOOP |
| §7 文档存取层（document-storage） | 新增整节（原文档零覆盖），涵盖 `resolve_path`/`save_doc`/`finalize_doc`/`get_constraints`/`get_assets` 等 API；同时标注 `get_thinking_engine(projectKey)` 被 3 处 SKILL 文档引用但 `document_storage.py` 无对应实现的已知缺口 |
| §9 Harness 适配层 | 脚本引用从已不存在的 `convert-ae-sdd-to-harness.ps1` 纠正为 `scripts/build_harness.py`（521 行，Python 重写） |
| §12 真实性扫描 | 覆盖范围从"3 个静态扫描器"纠正为 6 个扫描器（test/coding/ra_authenticity_scan + flow_violation_scan + ra_depth_scan + ra_implementation_scan + plugin_content_scan）+ 2 个对齐工具（alignment_audit/iteration_check） |
| §13 工具链 CLI | 分类数从"14 大类子命令"纠正为 29 个顶层子命令组；移除文档中虚构的 `route/sync-tools/run/quick/proposal` 命令（代码中不存在） |
| §16 主流程监管器 / §17 流程偏移检测与矫正 | 标题从"🆕 v3.6 规划中"纠正为"（v3.6，已实现）"，补充修正说明：`flow_monitor.py`（326 行）+ `prompt_inject.py:_run_flow_monitor()` + `state.py` 暂停/恢复/矫正计数 API 均已实现并接入 hook |
| §4 多 Agent 编排 | 补充"设计-实现落差"提示：角色库/调度协议/失败 SOP/多评审框架为纯 SKILL 文字约定，代码侧仅 `activeAgents`/`agentReports` 状态字段 + test-verifier session_id 校验予以支撑 |

## 触发原因

- 用户明确要求："这些有写进我们的设计文档内吗？设计文档要将设计和实现分开章节写。在写实现的时候要用表格标出来，哪些设计是用哪些方式实现的。"
- 4 个并行 Explore agent 核实各能力模块设计点→实现方式映射时，发现文档多处量化声明（门禁数/扫描器数/CLI 分类数/状态机链长度）与代码现状不一致，且发现 flow_monitor.py 已完整实现而文档仍标"规划中"的矛盾（已通过直接读源码核实，非轻信 agent 报告）。
- 用户经 AskUserQuestion 确认：v3.6 两节"改为已实现"；文档结构采用"每个能力模块内部拆分设计/实现"（非全文档二分）。

## 影响范围

- 仅 `source/docs/ae-sdd-design.md` 一个文档文件的内容重写，不涉及运行时逻辑、门禁行为、CLI 命令、state 机器字段的代码改动——本次是纯文档纠偏，代码侧无变更。
- 版本号未推进（仍为 3.7.3，与当前 MASTER_VERSION 一致），因为本次未产生新的功能性变更，只是让文档追平已经存在的代码事实。
- 无破坏性变更；下游（开发者/LLM Agent/项目接入方）读取本文档将看到更准确的设计-实现映射。

## 验证方式

- `python tools/bin/ae-sdd update-check` 全绿（确认文档重写未引入 UC-01~UC-05 一致性问题）。
- 人工核对新文档 17 个章节均遵循"设计→实现表格→颗粒度与边界"三段式结构。
- 表格中引用的文件:行号/函数名均在重写前通过 Read/Grep 逐条核实（flow_monitor.py、prompt_inject.py、state.py、gates.py、document_storage.py、memory_store.py、memory_gate.py、alignment_audit.py、build_harness.py、tools/bin/ae-sdd）。

## Reviewer

陈聪
