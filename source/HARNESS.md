# ae-sdd Agent Harness v1.5

<!-- 注入到 Agent system prompt 的顶部。本文件是 SKILL.md 的实例化产物，面向 Agent 执行，不面向人阅读。 -->
<!-- DO NOT SUMMARIZE. DO NOT SKIP. 每次对话必须完整读取本文件前 4 个 SECTION。 -->

---

## ⛔ BOOT PROTOCOL（第一动作，任何业务操作前执行）

> **v1.1 起：** 状态信息由 `UserPromptSubmit` hook 自动注入到 context（见 `◆ HARNESS STATE` 块）。
> AI 不需要手动执行 STEP 1-3，只需读取已注入的状态块。

收到 `/ae-sdd`、任何关联触发词或下述 Coding 写入意图后：

**STEP 1** — 读取系统上下文中的 `◆ HARNESS STATE` 块（由 hook 自动注入）

- 确认 `phase` / `story` / `G-00` 状态
- G-00 显示 `🔴 BLOCKED` → 停止，修复后继续

**STEP 2** — 根据 `next:` 字段确认路由目标，开始执行

**STEP 3** — 输出状态头（见下方 RESPONSE FORMAT）

> **快速通道：** 用户显式说 `/ae-sdd-quick` 或 `走快速通道` 时，跳过 STEP 1，直接落档。

---

## 🔒 CODING ENTRY CONTRACT（所有仓库写入型 Coding 请求强制进入 ae-sdd）

凡请求可能新增、修改、删除、重构、优化或生成生产代码、测试代码、配置、Schema、Migration、构建脚本或其他实现制品，Agent 必须：

1. 在制定实现计划或首次写入前加载并调用 `ae-sdd`；先判定任务规模、入口节点和执行路由，再开始实现。
2. 严格执行所选路由规定的上下文加载、阶段转换、审核点、阶段记忆、文档产物、测试、Review 和门禁；只有该路由明确豁免的步骤才可跳过。
3. 以 `ae-sdd` CLI 返回的 state、next-step 和 gate 结果为事实来源；CLI 与对话推断或提示词摘要冲突时，以 CLI 为准。
4. 任一 blocker gate 失败时立即停止后续写入，报告 gate ID 和失败原因，并执行规定的修复路径；禁止猜测、伪造或口头声明通过。
5. 在所选路由到达合法终态且所需验证证据落地前，禁止声称任务完成。

**禁止自行豁免：** “改动很小”“只有一行”“很紧急”“方案显而易见”均不是绕过理由；应进入对应的大/中/小/微、OPTIMIZE 或 CODE_REVIEW 路由。

**只读边界：** 纯解释、代码查询、状态报告和不修改工作区的审查不进入 Coding 流程；一旦请求转为实施或写入，必须在第一次编辑前进入 `ae-sdd`。

**失败关闭：** `ae-sdd` 未安装、状态不可定位或 CLI 无法运行时，不得降级为自由编码；应报告阻塞并先修复运行环境。

**唯一显式旁路：** 仅用户主动指定 `/ae-sdd-quick` 或“走快速通道”时按快速通道执行；Agent 不得自行选择该旁路，且快速通道不免除落档义务。

> **L2 会话级纪律**：本契约（L1）的会话级展开版见 `source/L2-DISCIPLINE.md`（SSOT 单点）。各 agent 全局指令文件（ZCode AGENTS.md / Codex AGENTS.md / Claude CLAUDE.md）的 ae-sdd 锚点区间由 `scripts/l2_inject.py` 从该 SSOT 派生注入，三家措辞同源、触发语义一致（改动级加载）。

---

## 📋 PHASE MACHINE（工具 PERMIT/DENY 表）

> **三层 hook 在 ae-sdd turn 内强制执行，AI 意愿无关；普通 turn 默认 inactive。**

`UserPromptSubmit` 只有在当前消息显式进入 `/ae-sdd` 或触发明确 ae-sdd 写流程时才创建 session 级 turn token。普通问题、Story 文档查阅和普通 Bash 不解析 Work Item、不注入状态、不触发 phase 门禁。Stop 成功或下一条普通消息会清理残留 token，Stop 阻断重试时保留 token。

| Phase             | Write/Edit    | Bash | 说明                      |
| ----------------- | ------------- | ---- | ------------------------- |
| `initialized`     | ✅ 仅文档目录 | ❌   | 只写设计文档，禁止写 src/ |
| `ra-generated`    | ✅ 仅文档目录 | ❌   | 🆕 v3.4.0 RA 需求分析（dr-generate 前置）|
| `dr-generated`    | ✅ 仅文档目录 | ❌   | 写 DR                     |
| `story-generated` | ✅ 仅文档目录 | ❌   | 写 Story                  |
| `story-reviewed`  | ✅ 仅文档目录 | ❌   | 写 TestCase               |
| `testcase-generated` | ✅ 仅文档目录 | ❌ | 🆕 v3.7.0 TestCase 生成完毕，待 Review |
| `testcase-reviewed`  | ✅ 仅文档目录 | ❌ | 🆕 v3.7.0 TestCase Review 通过，写 Task |
| `task-generated`  | ✅ 仅文档目录 | ❌   | 写 Task / CodingPlan      |
| `task-reviewed`   | ✅            | ✅   | 进入编码，允许写 src/     |
| `coding-process`  | ✅ 仅文档目录 | ❌   | CodingPlan 分析，禁止写 src/ |
| `coding`          | ✅            | ✅   | 写代码 + 跑测试           |
| `test-running`    | ✅            | ✅   | 测试 + 报告               |
| `code-reviewed`   | ✅ 仅文档目录 | ❌   | 写 CR 报告                |
| `completed`       | ❌            | ❌   | 只读                      |

**只读工具（Read / Glob / Grep）和只读 Bash（cat / ls / git status / ae-sdd state read 等）任何 phase 都允许。**

新需求入口：`ae-sdd state new --id <ID> --name "<需求名>"`（创建 `.auto-engineering/{ID}--{name}/state.json` 并设为 active）。

Phase 切换：`ae-sdd state write --phase <next>` 默认跟随 active work item；显式定位可用 `--work-item <ID或KEY>`，维护 legacy 项目级 state 才使用 `--project-state`。

→ hook 自动运行进入条件 gate 验证，不通过则物理拒绝切换

---

## 📡 RESPONSE FORMAT（每次响应必须以状态头开始）

```text
◆ STATE:  <phase>/<currentStory>
◆ GATE:   ✅ CLEAR | 🔴 BLOCKED(<gate-id>)
◆ LAST:   <刚完成的操作>
◆ NEXT:   <下一个必须做的操作>
```

示例：

```text
◆ STATE:  coding/STORY-023-BE
◆ GATE:   ✅ CLEAR
◆ LAST:   Task-3 AppService 编写完成，编译通过
◆ NEXT:   继续 Task-4 Repository Impl
```

**状态头仍是必填响应格式。** 但 v3.6 起 Stop hook 已废弃对 `◆ STATE:` / `◆ GATE:` 自报标记的物理校验；真实状态与门禁以 `UserPromptSubmit` 注入态、`flow_monitor` 和 `ae-sdd gates check` 为准。

---

## ❌ HARD STOPS（永远禁止，无例外）

- **HS-1** 没有 CodingPlan 写源码（PreToolUse hook 物理拦截）
- **HS-2** `ae-sdd state write` 跨步跳跃（PreToolUse hook 物理拦截）
- **HS-3** 用模糊回复（"好"/"行"/"OK"）当用户确认 → 必须追问（🆕 v3.4.0：声明但无物理实现，靠 agent 自律 + 审核点 token 机制 `ae-sdd state confirm` 兜底防 AI 自填）
- **HS-4** 跳过 ⑥bis / ⑦bis 一致性核查（🆕 v3.5.4：声明但无物理实现，靠 CodeReview 流程纪律 + ⑥/⑦ 闸文约束兜底；待 events 接入业务调用方后补物理拦截）
- **HS-5** 猜测业务信息 → 必须标 `{待确认}` 并停下来（声明但无物理实现，靠 G-CODEPLAN-SRC 源码核对门禁兜底）
- **HS-6** 未经确认修改已审核通过的测试代码（🆕 v3.5.4：声明但无物理实现——"已审核测试代码"判定边界模糊且无现成标记机制，靠 CodeReview ⑦ 闸兜底；待 state.testCodeAuditedAt 标记机制落地后补物理拦截）
- **HS-7**（🆕 v3.3.0，🆕 v3.5.4 补物理实现）未通过 4 层 AND 闸就触发 `ae-sdd state prd-complete`（PreToolUse hook 物理阻断：`tools/lib/gate_intercept.py:check_intercept` + `tools/lib/state.py:check_prd_4_layers` 实时校验）
- **HS-8**（🆕 v3.3.0，🆕 v3.5.4 补物理实现）PRD 级 compact 失败时未保留旧 PRD state.json（Stop hook 阻断 + 报警：`tools/lib/stop_check.py:_check_compact_failure` 检测 `prdStatus=awaiting_compact` 但无 `summary.md` 的卡住态）
- **HS-9**（🆕 v3.4.0，建议书4 关卡1）收到 `/ae-sdd` 触发后未跑 `ae-sdd enter` 领 entry token 就落地流程产物（UserPromptSubmit 注入强提醒 + 关卡2/3 物理拦截兜底）
- **HS-10**（🆕 v3.4.0，建议书4 关卡2；🆕 v3.8.1 兜底层增强）流程产物（Story/Task/CodingPlan/报告）落地未经 `resolve_path` 推导、落在 `d:\tmp\` 等游离位置（PreToolUse hook 用 `document_storage.resolve_path`/docWorkspace 语义做物理拦截；G-DOC-STORAGE 复用 `paths.resolve_doc_workspace` 解析新旧资产路径，并追加系统临时目录当前 Story 专项探针）
- **HS-11**（🆕 v3.4.0，建议书4 关卡3）非 coding/test-running phase 或无审核点 2.5 确认 token 写 src/ 源码（PreToolUse hook 物理拦截）
- **HS-12**（🆕 v3.4.0，建议书3 F-1；🆕 v3.6 决策1B诚实降级）AI 谎报 `◆ GATE: ✅ CLEAR` 但实际门禁未通过（声明但无 Stop 物理实现：Stop hook 已废弃自报标记检测，靠 UserPromptSubmit hook + flow_monitor + `ae-sdd gates check` 兜底）
- **HS-13** 暂离期间写源码/运行编译测试命令（声明但无物理实现——hook 无法感知"讨论模式"；靠 SKILL.md §🔀 暂离声明约束 AI 自律；未来可通过 `.ae-sdd/.detour_mode` 标记文件补物理拦截）
- **HS-14** 检测到编码意图词但未执行回归门直接写代码（声明但无物理实现；靠 SKILL.md §🔀 编码意图检测 + 回归门协议约束；AI 必须先输出`【主流程监管器 ❌ 阻断】`并执行 `ae-sdd state read`）
- **HS-15**（🆕 v3.8.0）自动化模式下未写 `reviewConsensus[point]` 就推进 review 节点 phase（声明但无物理实现——靠 G-AUTO-CONSENSUS 门禁兜底：`tools/lib/gates.py:check_g_auto_consensus` 校验 `state.reviewConsensus[point].passed=true` + reviewer 独立性复用 G-09B；非自动化模式 skip）
- **HS-16**（🆕 v3.8.1）人工审核点对话内呈现缺少必要结构（审核点 1/1.5/2/2.5/4 只给文件路径或空泛摘要）：Stop hook 物理重试拦截（`tools/lib/stop_check.py:_check_manual_review_point_format`），自动化模式 `automation.enabled=true` 时跳过；为降低格式正则误伤，复用 `MAX_RETRY=2` 达上限放行。

---

## 🚧 门禁快查

```text
G-00 项目资产  G-01 DR文档    G-02 Story文档  G-03 Story Review通过
G-04 TestCase  G-05 Task文档  G-06 Task Review G-07 CodingPlan
G-08 Plan14禁  G-09 测试真实性 G-09B reviewer独立性 G-10 测试报告
G-11 Coding报告 G-12 CR报告   G-13 全链路对称性 G-14 CP-Story一致性
G-CODEPLAN-SRC 源码核对  G-DOC-STORAGE 文档存放  G-DOC-CONSISTENCY 记忆-配置一致
G-PATH 路径越界  G-CODE-1 Coding真实性
G-RA-1 RA文档存在  G-RA-2 RA维度完整  G-RA-3 RA衍生章节  G-RA-4 RA真实性  G-RA-5 RA派生深度  G-RA-6 RA实现视角
G-RA-FLOW-VIOLATION RA流程违规  G-REVIEW-LOOP review-loop退出条件
G-AUTO-CONSENSUS 自动化联审共识
```

> 共 30 门禁（GATE_REGISTRY 权威，`tools/lib/gates.py` 实测）。

一键检查：`ae-sdd gates check --json`

---

## 🪝 三 Hook 配置（由 ae-sdd init-hooks 自动写入）

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Write|Edit|MultiEdit|Bash",
      "hooks": [{"type": "command", "command": "ae-sdd gate-intercept"}]
    }],
    "UserPromptSubmit": [{
      "hooks": [{"type": "command", "command": "ae-sdd prompt-inject"}]
    }],
    "Stop": [{
      "hooks": [{"type": "command", "command": "ae-sdd stop-check"}]
    }]
  }
}
```

**v1.2 数据格式（符合 Claude Code 官方 hook 规范，来源：hookify 实现）：**

| Hook               | stdin 关键字段               | 允许响应 | 拒绝/注入响应关键字段                               |
| ------------------ | ---------------------------- | -------- | --------------------------------------------------- |
| `PreToolUse`       | `tool_name`, `tool_input`    | `{}`     | `hookSpecificOutput.permissionDecision = "deny"`    |
| `UserPromptSubmit` | `user_prompt`                | `{}`     | `hookSpecificOutput.additionalContext`（v3.5.8 起字段从 `systemMessage` 改为 `additionalContext`）|
| `Stop`             | `transcript_path`            | `{}`     | `decision = "block"`, `reason`, `systemMessage`     |

**exit 始终为 0。** Claude Code 通过 JSON 字段判断是否允许，不通过 exit code。

**v3.3.0 UserPromptSubmit PRD 级 payload（🆕）：**

`UserPromptSubmit` 在 PRD 完成场景下，注入 `systemMessage` 字段补充：

```json
{
  "systemMessage": "PRD 级状态：prdStatus=awaiting_compact；下一动作：`ae-sdd state prd-complete --prd {PRD-ID} --runtime {runtime-name}`；详见 .auto-engineering/{PRD-ID}/state.md"
}
```

**触发条件：** `state.json.prdStatus ∈ {prd_complete_pending_user, awaiting_compact}` 时由 `prompt-inject` hook 自动注入；其他状态不注入。

**Mavis 等价物：** 复用 `mavis session rotate --handoff-file` 协议，由 `ae-sdd runtime compact --runtime mavis` 调用后自动注入下个 session context。

**Codex 等价物：** 由 R-3 Codex PoC 决定具体 payload 协议（待 PoC 落档 `docs/plans/2026-06-25-codex-poc-result.md`）。

---

## 🔄 分发闭环（v3.4.0+ 必备）

> **🔴 v3.4.0 之前的问题**：母版 `source/` → `dist/ae-sdd/` → `~/.claude/skills/ae-sdd` + `~/.zcode/skills/ae-sdd` → 业务仓 `.ae-sdd/` 四层之间**没有任何自动触发器**，全靠开发者手工跑 `dev-sync.sh`。
> 结果：12 次 commit 之间没人跑 → `harness/.harness/agent.md` 停在 v3.1.2、`~/.zcode/skills/ae-sdd/` 停在 v3.2.3、`.adapter.lock` 漂移到 3.3.0。

### 自动触发链

| # | 事件 | 触发器 | 动作 |
|---|------|--------|------|
| 1 | ae-sdd 母版 commit | `.githooks/post-commit`（git hooksPath）| 单入口 `distribute.py`：① `build_dist.py` (source → dist) ② 遍历 `DISTRIBUTORS` 注册表逐个 compile/install/verify（copytree 类：claude/codex/zcode/hermes；harness_mount 类：mavis，内部含 `convert-ae-sdd-to-harness.ps1` + `mavis harness remount`）|
| 2 | 业务仓 `ae-sdd init` | `init.py` | 写 `.claude/settings.json` (3 hook) + `.ae-sdd/config.yaml` (master.version = 母版实时 frontmatter version) |
| 3 | 业务仓每次 UserPromptSubmit | `prompt_inject.py` | 若 `installed MASTER_VERSION < config.yaml master.version` → 注入 `⚠️ master-freshness` 文本（不阻断）|
| 4 | 任意位置跑 `ae-sdd health` | CLI | 比对 4 个版本源（CLI / source / dist / installed），输出 `master-freshness` 报告 + 修复建议 |

### 跳过策略（post-commit hook）

- 非母版仓库（无 `source/SKILL.md`）→ 静默退出 0
- 仅修改 `source/CHANGELOG/` 或 `README.md` → 跳过（无功能性变更）
- `SKIP_AE_SDD_HOOK=1` 环境变量 → 跳过（紧急旁路）
- 任何步骤失败 → 不回滚 commit（commit 已落库），仅输出错误，不阻断 git

### 兜底人工流程

```bash
# 1. git hook 失效时（开发机禁用 hook / Windows 权限问题）
bash scripts/dev-sync.sh                       # build + install + harness adapter

# 2. Harness 适配器独立重转
powershell -File "$HOME/.zcode/skills/ae-sdd-harness-adapter/scripts/convert-ae-sdd-to-harness.ps1" \
  -Source "D:\Item\ae-sdd"

# 3. 健康度自检（含分发闭环检查）
ae-sdd health
```

### 验收清单（手动触发一次完整 commit 验证）

```bash
cd D:\Item\ae-sdd
touch source/SKILL.md                            # 触发功能性 commit
git add . && git commit -m "test: post-commit hook"
# 应看到 4-5 行 "✅" 输出（build / install / harness / mavis）

ae-sdd health
# 应看到 9 项健康度，其中 master-freshness 显示 "全部一致"
```

---

## ⚠️ 升级注意

若已用 `ae-sdd init-hooks --use-python` 配置 hook，重装 CLI（`python scripts/install_cli.py`）后 hook 内的 Python 路径可能失效。
安装完成后请运行 `python scripts/install_cli.py --check` 验证路径。
若 hook 仍指向旧路径，重跑：`ae-sdd init-hooks --use-python --force`。

---

## 🔧 高级配置

### 快速通道（跳过 gate 拦截）

| 触发方式 | 作用域 | 场景 |
| --- | --- | --- |
| 消息含 `/ae-sdd-quick` 或 `走快速通道` | 当次对话（清除即失效） | 紧急修复，补救操作 |
| `AE_SDD_QUICK=1` 环境变量 | 进程级（Shell 会话期间） | CI/CD pipeline、Docker 容器 |

快速通道绕过 PreToolUse gate 拦截，但 **不免除落档义务**，操作结束后仍需补写设计文档。
