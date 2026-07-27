# ae-sdd harness/

本目录存放 Agent Harness 相关文件。

## 文件说明

| 文件 | 用途 |
|------|------|
| `../HARNESS.md` | Agent Harness 主文件（注入到 Agent system prompt） |

## 什么是 Harness？

Harness 是 `SKILL.md`（面向人）的实例化产物，面向 Agent 执行：

- `SKILL.md` = 2000+ 行自然语言，描述"应该怎么做"（给人读）
- `HARNESS.md` = 精简机器格式，执行 PERMIT/DENY 表 + hook 配置（给 Agent 执行）

两个约束层：

- **Layer 1（文本层）** = `HARNESS.md` 注入 system prompt — 约束 AI 决策
- **Layer 2（Hook 层）** = PreToolUse hook → `ae-sdd gate-intercept` — 物理拦截工具调用

Layer 2 是真正的 Harness，Layer 1 是辅助。

## 注入方式

1. 直接粘贴 `HARNESS.md` 到 Agent system prompt 顶部（推荐）
2. 通过 `ae-sdd init-hooks` 自动写入项目 `.claude/settings.json` 的 hooks 配置

## 使用命令

```bash
# 写入 hook 到项目
ae-sdd init-hooks <project-dir>

# 查看 hook 拦截决策
ae-sdd gate-intercept --tool Write
ae-sdd gate-intercept --tool Bash --bash-command "mvn test"

# 查看门禁状态
ae-sdd gates check --json
```

## 相关文件

- `source/HARNESS.md` — Harness 主文件
- `crates/ae-sdd-policy/src/hook.rs` — 拦截决策核心（HookPolicy）
- `bins/ae-sdd-cli/src` — CLI 入口（gate-intercept / init-hooks 子命令）
- `bins/ae-sdd-cli/tests/handshake_and_hook_failure.rs` — hook 拦截测试
