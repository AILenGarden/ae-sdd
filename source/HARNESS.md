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
- **HS-3** 用模糊回复（"好"/"行"/"OK"）当用户确认 → 必须追问
- **HS-4** 跳过 ⑥bis / ⑦bis 一致性核查
- **HS-5** 猜测业务信息 → 必须标 `{待确认}` 并停下来
- **HS-6** 未经确认修改已审核通过的测试代码

---

## 🚧 门禁快查

```text
G-00 项目资产  G-01 DR文档    G-02 Story文档  G-03 Story Review通过
G-04 TestCase  G-05 Task文档  G-06 Task Review G-07 CodingPlan
G-08 Plan14禁  G-09 测试真实性 G-10 测试报告   G-11 Coding报告
G-12 CR报告    G-13 全链路对称性
```

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
| `UserPromptSubmit` | `user_prompt`                | `{}`     | `systemMessage`                                     |
| `Stop`             | `transcript_path`            | `{}`     | `decision = "block"`, `reason`, `systemMessage`     |

**exit 始终为 0。** Claude Code 通过 JSON 字段判断是否允许，不通过 exit code。

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
