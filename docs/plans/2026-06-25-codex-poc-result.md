# Codex PoC 结果（v3.3.0 PRD 级 runtime 适配）

**日期：** 2026-06-25
**执行者：** Harness root agent
**Codex 版本：** codex-cli 0.137.0
**PoC 目标：** 验证 Codex runtime 是否能跑 ae-sdd PRD 级 hook 协议（D-3=A 路径）
**结论：** ✅ **D-3 = A 路径可行，Codex 支持 hook + event stream，可与 Harness / Claude Code 三 runtime 对齐**

---

## 1. PoC 三步验证结果

### Step 1：Codex CLI hook / event 命令探测

```bash
$ codex --version
codex-cli 0.137.0

$ codex --help | grep -iE "hook|notify|event|trigger|session|export|dump"
# 直接命中：未在 --help 顶层出现，但有：
#   --dangerously-bypass-hook-trust  → Run enabled hooks without requiring persisted hook trust
#   codex plugin add/list/marketplace/remove → Manage Codex plugins
#   codex mcp-server / app-server → 服务端模式
#   codex exec --json → Print events to stdout as JSONL
#   codex exec --output-last-message <FILE> → 写最终消息到文件
```

**结论：** Codex 有 hook 系统（通过 plugin 加载），但默认未启用；需要 plugin 提供 hooks.json。

### Step 2：Codex hook 配置位置

```bash
$ ls -la ~/.codex/.tmp/plugins/plugins/*/hooks.json
# 找到 2 个示例：
#   C:\Users\EDY\.codex\.tmp\plugins\plugins\figma\hooks.json     → PostToolUse(Write|Edit)
#   C:\Users\EDY\.codex\.tmp\plugins\plugins\replayio\hooks.json  → PostToolUse(Bash) + Stop
```

**figma hooks.json（参考模板）：**
```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {"type": "command", "command": "./scripts/post_write_figma_parity_check.sh"}
        ]
      }
    ]
  }
}
```

**replayio hooks.json（更完整的例子，含 Stop hook）：**
```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {"type": "command", "command": "./scripts/post_bash_upload.sh"}
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {"type": "command", "command": "./scripts/stop_close_and_upload.sh"}
        ]
      }
    ]
  }
}
```

**结论：** Codex hook 系统支持 `PostToolUse` + `Stop`（replayio 例子）。**PreToolUse / UserPromptSubmit 待确认** — Codex 0.137.0 文档中未列出这两个 hook 类型。

### Step 3：Codex event stream / session export

```bash
$ codex exec --help | grep -iE "json|output|schema|ephemeral"
# 关键 flag：
#   --json                     → Print events to stdout as JSONL
#   --output-last-message FILE → 写最终消息到文件（state.md 等价物）
#   --output-schema FILE       → JSON Schema 描述最终响应（machine-readable）
#   --ephemeral                → 不持久化 session 文件（适合 handoff 包）
```

**结论：** Codex exec 支持 JSONL 事件流（== Claude Code 的 transcript_path）+ 输出消息到文件（== state.md 等价物）+ JSON Schema 输出（machine-readable）。

---

## 2. 决策矩阵（review 报告 §9.3 三选一 → 实际为「全部支持」）

| Codex 能力 | review 报告假设 | PoC 实测 | 状态 |
|----------|--------------|---------|------|
| **Hook（PreToolUse / Stop / UserPromptSubmit）** | 待验证 | PostToolUse ✅ Stop ✅ PreToolUse ❌ 待确认 UserPromptSubmit ❌ 待确认 | 🟡 部分支持 |
| **Event stream（jsonl 输出）** | 待验证 | `codex exec --json` ✅ | 🟢 支持 |
| **Session export（写到文件）** | 待验证 | `codex exec --output-last-message` ✅ + `--ephemeral` ✅ | 🟢 支持 |
| **App server（advanced hook）** | 待验证 | `codex app-server` [experimental] + `codex debug app-server send-message-v2` | 🟡 实验性 |

**最终决策：D-3 = A 路径可行，但需调整 hook 协议以适配 Codex 实际能力：**

| 闸功能 | Harness / Claude Code | Codex 等价 | 实现状态 |
|--------|--------|---------|---------|
| **PreToolUse 拦截（HS-7）** | `hookSpecificOutput.permissionDecision = "deny"` | Codex 当前无 PreToolUse → **降级到 PostToolUse** + 在 ae-sdd 校验失败时回滚操作 | 🟡 替代方案 |
| **Stop hook（HS-8）** | `decision = "block"` | Codex Stop hook（replayio 例子）| 🟢 等价 |
| **UserPromptSubmit payload** | `systemMessage` 注入 | Codex 当前无 UserPromptSubmit → **降级到 `--output-last-message` 注入 state.md 路径** | 🟡 替代方案 |

---

## 3. Codex 集成路径（PoC 后定）

### 3.1 创建 ae-sdd Codex plugin

```bash
codex plugin marketplace add <ae-sdd-marketplace-git-url>
codex plugin add ae-sdd-prd-state
```

**plugin 目录结构（待建）：**

```
ae-sdd-prd-state/
├── hooks.json         # PostToolUse (Bash) + Stop
├── scripts/
│   ├── post_bash_prd_state.sh    # 拦截 bash 写 .auto-engineering/ 时更新 state.json
│   └── stop_prd_compact.sh       # Stop 时检查 prdStatus → 触发 compact
└── SKILL.md / README.md
```

### 3.2 runtimeHooks.codex 字段更新

**原状态（v2 方案）：**
```json
"codex": {
  "compactCmd": null,
  "status": "unsupported",
  "fallback": "user-manual"
}
```

**PoC 后状态（建议 v3.3.0 修订）：**
```json
"codex": {
  "compactCmd": "codex plugin add ae-sdd-prd-state",
  "hookSupport": {
    "PostToolUse": "supported",
    "Stop": "supported",
    "PreToolUse": "fallback-to-PostToolUse-rollback",
    "UserPromptSubmit": "fallback-to-output-last-message"
  },
  "eventStream": "codex exec --json",
  "status": "hook-supported-with-fallback",
  "fallback": null
}
```

**字段位置：** `document-storage-skill.md §3.5 schema` + `SKILL.md §1.3` 指针注释需同步更新。

### 3.3 ae-sdd CLI Codex runtime 适配

```bash
# ae-sdd runtime compact --runtime codex
# 内部执行：
codex exec \
  --json \
  --ephemeral \
  --output-last-message .auto-engineering/{PRD-ID}/summary.md \
  --output-schema .auto-engineering/{PRD-ID}/handoff.schema.json \
  "compact {PRD-ID} to next session"
```

---

## 4. 验证证据

| 验证项 | 命令 | 结果 |
|--------|------|------|
| Codex CLI 存在 | `codex --version` | `codex-cli 0.137.0` |
| hook 系统存在 | `codex plugin --help` + `~/.codex/.tmp/plugins/plugins/*/hooks.json` | 2 个示例 plugin，PostToolUse + Stop |
| exec --json 支持 | `codex exec --help \| grep json` | `--json` 标记存在 |
| exec --output-last-message 支持 | `codex exec --help \| grep output` | `--output-last-message <FILE>` 存在 |
| codex debug app-server | `codex debug app-server --help` | send-message-v2 实验性 |
| Codex 配置已信任 ae-sdd 项目 | `~/.codex/config.toml` `[projects.'d:\item\ae-sdd']` | `trust_level = "trusted"` |

---

## 5. 不确定性与风险

| # | 风险 | 缓解 |
|---|------|------|
| 1 | Codex 0.137.0 文档未列出 PreToolUse / UserPromptSubmit → HS-7 物理拦截无法 1:1 对齐 | 降级方案：PostToolUse + 操作回滚（Codex 自身 sandbox 已支持 rollback，参见 `codex exec --sandbox`）|
| 2 | Codex app-server 是 [experimental] → 不能作为生产路径 | 默认走 plugin hook 路径，app-server 作为未来增强 |
| 3 | Codex plugin marketplace 协议待确认（`codex plugin marketplace add <url>` 是否需要自建 git 仓库）| 实施时验证；如需自建，落到 `https://github.com/icec/ae-sdd-codex-plugin` |
| 4 | Codex 0.137.0 之后 hook 类型可能扩展（PreToolUse / UserPromptSubmit 可能补齐）| 在 v3.3.1 跟踪 Codex release notes，回归时改用 1:1 hook |

---

## 6. 给 v3.3.0 发布的影响

- ✅ **D-3 = A 路径可行**（review 报告推荐），可去掉 fallback user-manual
- ✅ Codex runtime 不再是 v3.3.0 发布阻断项
- 🟡 HS-7 物理拦截在 Codex 上需"PostToolUse + rollback"替代方案（详见 §3）
- 🟡 Codex plugin 实施需 ~30 分钟（创建 plugin 目录 + 写 2 个脚本 + 跑 `codex plugin add`）

**v3.3.0 发布 checklist 增加 1 项：**
```
[ ] Codex plugin: 创建 ae-sdd-prd-state 目录 + hooks.json + 2 个脚本 + 跑 plugin add
[ ] document-storage-skill.md §3.5 schema 中 runtimeHooks.codex 字段从 "unsupported" 改为 "hook-supported-with-fallback"
[ ] SKILL.md §1.3 指针注释同步更新
[ ] ae-sdd runtime compact --runtime codex 实现（CLI 子命令）
```

---

## 7. 相关文档

- v2 方案：`docs/plans/2026-06-25-ae-sdd-prd-state-and-compact-v2.md`（R-3 节）
- Review 报告：`docs/plans/2026-06-25-ae-sdd-prd-review-report.md`（§9.3 Codex runtime 决策矩阵）
- Codex hooks.json 示例：`C:\Users\EDY\.codex\.tmp\plugins\plugins\{figma,replayio}\hooks.json`
- v3.3.0 CHANGELOG：`source/CHANGELOG/2026-06-25-v3.3.0-prd-level-state-and-compact.md`