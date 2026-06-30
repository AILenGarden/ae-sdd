# ae-sdd Agent Harness v1.4

<!-- 注入到 Agent system prompt 的顶部。本文件是 SKILL.md 的实例化产物，面向 Agent 执行，不面向人阅读。 -->
<!-- DO NOT SUMMARIZE. DO NOT SKIP. 每次对话必须完整读取本文件前 3 个 SECTION。 -->

---

## ⛔ BOOT PROTOCOL（第一动作，任何业务操作前执行）

> **v1.1 起：** 状态信息由 `UserPromptSubmit` hook 自动注入到 context（见 `◆ HARNESS STATE` 块）。
> AI 不需要手动执行 STEP 1-3，只需读取已注入的状态块。

收到 `/ae-sdd` 或任何关联触发词后：

**STEP 1** — 读取系统上下文中的 `◆ HARNESS STATE` 块（由 hook 自动注入）

- 确认 `phase` / `story` / `G-00` 状态
- G-00 显示 `🔴 BLOCKED` → 停止，修复后继续

**STEP 2** — 根据 `next:` 字段确认路由目标，开始执行

**STEP 3** — 输出状态头（见下方 RESPONSE FORMAT）

> **快速通道：** 用户显式说 `/ae-sdd-quick` 或 `走快速通道` 时，跳过 STEP 1，直接落档。

---

## 📋 PHASE MACHINE（工具 PERMIT/DENY 表）

> **三层 hook 强制执行，AI 意愿无关。**

| Phase             | Write/Edit    | Bash | 说明                      |
| ----------------- | ------------- | ---- | ------------------------- |
| `initialized`     | ✅ 仅文档目录 | ❌   | 只写设计文档，禁止写 src/ |
| `ra-generated`    | ✅ 仅文档目录 | ❌   | 🆕 v3.4.0 RA 需求分析（dr-generate 前置）|
| `dr-generated`    | ✅ 仅文档目录 | ❌   | 写 DR                     |
| `story-generated` | ✅ 仅文档目录 | ❌   | 写 Story                  |
| `story-reviewed`  | ✅ 仅文档目录 | ❌   | 写 TestCase               |
| `task-generated`  | ✅ 仅文档目录 | ❌   | 写 Task / CodingPlan      |
| `task-reviewed`   | ✅            | ✅   | 进入编码，允许写 src/     |
| `coding`          | ✅            | ✅   | 写代码 + 跑测试           |
| `test-running`    | ✅            | ✅   | 测试 + 报告               |
| `code-reviewed`   | ✅ 仅文档目录 | ❌   | 写 CR 报告                |
| `completed`       | ❌            | ❌   | 只读                      |

**只读工具（Read / Glob / Grep）和只读 Bash（cat / ls / git status / ae-sdd state read 等）任何 phase 都允许。**

Phase 切换：`ae-sdd state write --phase <next> [--story <ID>]`

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

**响应中若不包含状态头 → Stop hook 阻止本次响应结束，AI 被迫补充。**

（状态头可位于响应任意位置，Stop hook 检查响应中是否包含 ◆ STATE: 标记）

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
- **HS-10**（🆕 v3.4.0，建议书4 关卡2）流程产物（Story/Task/CodingPlan/报告）落地未经 `resolve_path` 推导、落在 `d:\tmp\` 等游离位置（PreToolUse hook 物理拦截 + G-DOC-STORAGE 门禁）
- **HS-11**（🆕 v3.4.0，建议书4 关卡3）非 coding/test-running phase 或无审核点 2.5 确认 token 写 src/ 源码（PreToolUse hook 物理拦截）
- **HS-12**（🆕 v3.4.0，建议书3 F-1）AI 谎报 `◆ GATE: ✅ CLEAR` 但实际门禁未通过（Stop hook 交叉验证 G-08 与 CodingPlan 文档一致）

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
```

> 共 29 门禁（GATE_REGISTRY 权威，`tools/lib/gates.py` 实测）。

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
| 1 | ae-sdd 母版 commit | `.githooks/post-commit`（git hooksPath）| ① `build_dist.py` (source → dist) ② `install.py --target-path ~/.claude/skills/ae-sdd --quiet` ③ `install.py --target-path ~/.zcode/skills/ae-sdd --quiet` ④ `convert-ae-sdd-to-harness.ps1` (SKILL+HARNESS → agent.md) ⑤ `mavis harness remount` |
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
