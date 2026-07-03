# 2026-07-03 设计-实现一致性审计与修复

## 触发原因

用户提出「ae-sdd 的启动成本不随任务难度缩放」痛点，要求执行设计-实现一致性检查：既要审设计文档前后自洽，也要审设计与实现对齐。本次为 ae-sdd 自身的 self-update 路由任务。

## 审计方法

双 Explore agent 横扫（设计-实现层 + 设计-设计层）+ 亲读代码定夺两 agent 的矛盾 + 跑 `iteration-check`/`update-check` 机器基线 + 针对 IC-1/UC-16 误报反查 CLI 注册。

## 发现清单（5 项缺陷 + 3 项误报）

| # | 判定 | 项 | 核心矛盾 |
|---|------|----|---------|
| B1 | 🔴 设计-实现不一致 | 微任务 CodeReview | conventions.md:171 声明"CodeReview 报告❌不豁免"，但 state.py:91 微链物理跳过 code-reviewed phase，gate_intercept.py:616 门禁声明却不可达（死代码） |
| B2 | 🟡 语义模糊 | CLI lazy import 挂账 | plan P1 声称改 lazy import，至今未落地；设计文档未追踪挂账；与源 SKILL 瘦身（已落地）被同语系叙述混淆 |
| B3 | 🟡 观测盲点 | runtime_stats 无 scale 维度 | 只按 command 分桶，测不出"微任务 vs 大任务开销比"，无法定位比例失调 |
| B4 | 🟡 注释悬空 | classify.py:16 LLM 承诺 | 注释"v3.1+ 可升级为 LLM"至 v3.8.1 未兑现亦未撤销，v3.5.10/3.5.15 两度用规则绕开 |
| B5 | 🟢 自洽 | 其余设计文档 | 无矛盾 |
| 误报1 | 检查器未适配 | IC-1 "幽灵命令 state lock" | state lock 在 v3.8.1 真实注册（cmd_state_lock + argparse:3275），检查器清单未更新 |
| 误报2 | 检查器未适配 | UC-16 "缺自动化章节" | source/SKILL.md 是 slim entry，章节在 fallback full:232，检查器未适配 slim 架构 |
| 误报3 | runtime 快照过期 | UC-14 "缺 UC 锚点" | fallback:579 完整列出 UC-01~16，重编译后即绿 |

## 修复详情

### B1（P0，用户决策：改实现跟上设计）
- `tools/lib/state.py:90-96`：微链从 7 phase → 8 phase，加回 `code-reviewed`。修正注释。
- `tools/lib/gate_intercept.py:613-620`：微链 coding 入口从 `["G-00"]` → `["G-00","G-07","G-08"]`（Plan-first 不豁免，与大链对齐）。修正注释（原误写"跳过 Task"）。
- `tools/tests/test_state.py:239-247`：`test_micro_chain_has_7_phases` → `test_micro_chain_has_8_phases` + 新增 `test_micro_chain_includes_code_reviewed`。
- `source/docs/ae-sdd-design.md:33,74`：微链 phase 数 7→8；明确"快速通道"仅指 escape hatch，不等于 scale=微 豁免。

### B3（P1，观测盲点）
- `tools/lib/runtime_stats.py`：新增 `_detect_scale()` 从项目 state.json 读 scale；`start_command` 探测 scale 写入 attrs；`to_event` 提升 scale 为顶层字段；`summarize_events` 新增 `byScale` 分桶 + `scaleRatios`（微/小/中 vs 大）。
- `tools/bin/ae-sdd:_perf_advice`：新增比例失调规则（微/大 ≥0.8 告警）+ lazy import 挂账提示（avg>150ms）。
- `tools/bin/ae-sdd:cmd_perf_report`：输出 byScale 与 scaleRatios。
- `tools/tests/test_runtime_stats.py`：新增 4 个测试（scale 分桶、比例失调告警、_detect_scale 读 state.json、无 state 返回 None）。
- `source/docs/ae-sdd-implementation-architecture.md:175-194`：runtime_stats 架构段落加 scale 维度 + P1 lazy import 挂账说明。

### B2（P1，挂账显式化）
- `tools/bin/ae-sdd:_perf_advice`：bootstrap>150ms 时提示"P1 lazy import 未实施，详见 plan"。
- `source/docs/ae-sdd-implementation-architecture.md:177-180`：加 P1 lazy import 挂账说明，区分源瘦身（Agent token，已落地）vs CLI import（进程 ms，未落地）。

### B4（P2，注释治理）
- `tools/lib/classify.py:16`：删除悬空承诺"v3.1+ 可升级为 LLM 辅助判定"，改为诚实记录 LLM 升级路径状态（v3.5.10/3.5.15 两度用规则绕开，路径待评估，不再作版本号承诺）。

### B6（P2，检查器适配 slim 架构）
- `tools/lib/iteration_check.py:73-78`：`_GHOST_COMMANDS` 移除 `"state lock"`（v3.8.1 真实注册）。
- `tools/lib/update_graph.py:1000-1013`：UC-16 检测 `## 🚀 自动化模式` 时合并读取 source/SKILL.md（slim entry）+ skill-fallbacks/SKILL.full.md（fallback），适配 source-slim 架构。

## 验证

- `ae-sdd update-check`：✅ 全绿（"更新闭环完整，无漏更新，AA 全维对齐通过"），UC-14/UC-16 误报清零。
- `ae-sdd iteration-check`：warn 从 3 降至 1（剩余 HS-16 为既有问题，与本次无关）；IC-1 state lock 误报清零。
- `pytest test_runtime_stats.py test_state.py`：63 passed。
- `pytest test_gate_intercept.py test_gates.py`：211 passed。

## 影响范围

- 能力语义变化（B1）：微任务现必须出 CodeReview 报告。已同步 `ae-sdd-design.md`。
- 实现架构变化（B3）：runtime_stats 新增 scale 维度与 byScale/scaleRatios 输出。已同步 `ae-sdd-implementation-architecture.md`。
- 未受影响：实例化体系、Harness 适配层、构建脚本、分发闭环。

## 设计/实现架构文档同步

- `source/docs/ae-sdd-design.md`：已更新（B1 微链 phase 数 + 快速通道语义澄清）。
- `source/docs/ae-sdd-implementation-architecture.md`：已更新（B2 lazy import 挂账 + B3 scale 维度）。

## Reviewer

陈聪（用户确认 B1 方向：微任务也必须出 CodeReview 报告，改实现跟上设计）
