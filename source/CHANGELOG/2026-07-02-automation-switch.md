# 2026-07-02 | ae-sdd v3.8.0 - 自动化开关配置与全自动化模式

## Summary

ae-sdd 此前每个审核点都需用户人工✅，无法做到"输入→结果"的端到端自动化。本次新增**自动化开关配置**（`.ae-sdd/config.yaml` 的 `automation` 段，默认关闭），开启后 6 个人工审核点（1/1.5/2/2.5/4/5）改走 **Tier 3 多 reviewer 联审共识**，跳过所有人工✅，实现全自动化。联审机制复用已存在的 `agent-orchestration-skill §8.4`（Tier 判定+视角正交+交叉对比+降级规则），不重新发明。开工前通过 `ae-sdd preflight collect` 一次性向用户收集所有必需信息（第三方凭证/复用选择/环境配置/命名约定/对接方/数据初始化），开工后不再打断；联审 3 轮矫正未决 → `state.phase=paused` 等用户介入，避免 AI 带病狂奔。

核心立场：默认关闭；开启即强制 Tier 3 物理三审（禁逻辑多视角降级）；AI 不得自行 enable（`enabledAt` 审计时间戳由 CLI 写入）。

## Changes

| Area | Change |
|---|---|
| tools/lib/config.py（新建）| 新增 `AUTOMATION_DEFAULTS` + `load_automation_config()` + `is_automation_enabled()`/`get_reviewer_tier()`/`get_automated_points()` 便捷函数，读 `.ae-sdd/config.yaml` 合并默认值 |
| scripts/init.py | CONFIG_TEMPLATE 追加 `automation:` 段（默认 `enabled: false` + reviewerTier:3 + preflightInfoCollection + onConsensusStall + automatedReviewPoints + enabledAt）|
| source/SKILL.md | frontmatter v3.7.4→3.8.0；Step1 后加自动化检测；新增 Step1.5 开工前信息预收集协议；监管器步骤4 双模式（默认人工/自动化联审共识）；新增 §🚀 自动化模式章节；新增 G-AUTO-CONSENSUS 门禁速查；工具API速查加 automation/preflight/register-review-consensus + 29→30门禁 |
| agent-orchestration-skill.md | §8.4.1 Tier 判定输入来源加"自动化模式强制 Tier 3"行；§8.4.5 禁止的降级加"自动化模式禁逻辑多视角降级"项 |
| tools/lib/state.py | 新增 `reviewConsensus[point]` 字段 + `register_review_consensus()`/`get_review_consensus()` 写读函数 |
| tools/lib/gates.py | GATE_REGISTRY 追加 G-AUTO-CONSENSUS（30 门禁）；头注释 29→30；新增 `check_g_auto_consensus()`（自动化模式校验 reviewConsensus.passed + reviewer 独立性）；CHECK_FUNCS 注册；修复 reviewConsensus key 查找兼容 str(int)/str(float) |
| tools/tests/test_gates.py | 头注释 29→30；test_check_all_returns_all/test_summarize 断言 29→30；新增 TestGAutoConsensus 6 用例 |
| tools/bin/ae-sdd | 头注释门禁数 29→30 + 用法示例加 automation/preflight/register-review-consensus；import config 模块；新增 `cmd_automation_status/enable/disable` + `cmd_preflight_collect` + `cmd_state_register_review_consensus` + `_write_automation_config`/`_now_iso` 辅助函数；argparse 注册 automation/preflight 子命令组 + state register-review-consensus 子命令；gates help 29→30；修复 `--passed` 字符串转 bool |
| source/standards/update-graph.json | 新增 UG-20（automation switch and auto-consensus gate，trigger 含 config.py/init.py/gates.py/state.py/ae-sdd，checks UC-02/03/16）|
| tools/lib/update_graph.py | HEALTH_CHECKLIST_REQUIRED 加 G-AUTO-CONSENSUS + UC-16 项；新增 `check_uc16_automation_cascade()`（校验六处齐备）；CHECK_FUNCS 注册 UC-16 |
| source/skills/orchestration/ae-sdd-update-skill.md | 机器同步锚点行加 UG-20/UC-16；健康度清单门禁计数 22→30 + 设计模块计数 12→19；新增 v3.8.0 自动化勾选项 |
| source/docs/ae-sdd-design.md | 新增 ## 19 自动化模式能力模块（设计+实现表）|
| source/HARNESS.md | HS 规则表加 HS-15（自动化模式未写 reviewConsensus 推进 phase，诚实降级标注靠 G-AUTO-CONSENSUS 门禁兜底）|
| README.md | L130 门禁数 29→30 + 自动化模式描述；新增 §🚀 自动化模式章节（配置/用法/行为分叉/预收集/阻断出口）|
| tools/tests/test_automation_cli.py（新建）| 15 用例覆盖 automation status/enable/disable + preflight collect + state register-review-consensus 端到端 |

## 触发原因

用户需求："做一个自动化开关，做成 ae-sdd 的配置文件。自动化默认关闭，开启时跳过所有人工审核，由多 Agent 联审原本由人工审核的环节，使得 ae-sdd 全自动化（输入-结果）。实在是需要用户提供信息的任务，需要在开始时就找用户要——开工前列一个清单给用户，让用户补充清楚所有所需信息。"

## 影响范围

- **配置层**：`.ae-sdd/config.yaml` 新增 `automation` 段（init.py 生成，默认关）
- **流程层**：6 审核点行为分叉（默认人工 / 自动化联审共识），不新增 phase，复用 PHASE_FLOWS
- **门禁层**：新增 G-AUTO-CONSENSUS（30 门禁），复用 G-09B/G-REVIEW-LOOP
- **状态层**：新增 reviewConsensus 字段，不动状态机骨架
- **CLI 层**：新增 automation/preflight/register-review-consensus 三组子命令
- **级联层**：新增 UG-20/UC-16，update-check 16 项全绿
- **兼容性**：默认关闭，不影响现有项目；开启需用户显式 `ae-sdd automation enable`

## 验证方式

```bash
ae-sdd automation status                    # 未 init 报错；init 后 enabled:false
ae-sdd automation enable                    # 写 enabled:true + enabledAt
ae-sdd preflight collect                    # 扫描材料生成待补信息清单
ae-sdd state register-review-consensus --point 1 --passed true  # 写联审共识
ae-sdd gates check --only G-AUTO-CONSENSUS  # 非自动化模式 skip
ae-sdd update-check                         # UC-01~16 全绿（重点 UC-02/03/14/16）
python -m pytest tools/tests/test_gates.py tools/tests/test_automation_cli.py  # 全过
```

## Reviewer

陈聪（ae-sdd 维护者）
