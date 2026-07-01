# ae-sdd Mavis Harness (AUTO-GENERATED)

> **🔴 本目录由 `ae-sdd-harness-adapter` 自动生成，请勿手工编辑。**

## 这是什么？

把 ae-sdd（`D:\Item\ae-sdd`）的 Claude Code 风格 harness（`HARNESS.md` + hook 配置）转译成 Mavis harness 格式（`agent.md`）。

两个 harness 概念不同：
- ae-sdd HARNESS.md → 通过 Claude Code `UserPromptSubmit` hook 注入到 system prompt
- 本目录 agent.md → 通过 `mavis harness mount` 注册为 Mavis 团队级 agent

## 文件清单

| 文件 | 来源 | 说明 |
|------|------|------|
| `agent.md` | `templates/agent.md.template` 渲染 | Mavis harness 主入口 |
| `.adapter.lock` | 脚本生成 | 幂等标记：上次转换的 ae-sdd commit hash + 时间戳 |

## 重新生成

ae-sdd 母版升级后（`D:\Item\ae-sdd\.git` 有新 commit），跑：

```bash
python scripts/build_harness.py --source "D:\Item\ae-sdd"
```

脚本会自动：
1. 检测到 commit hash 变化 → 触发重转
2. 渲染新 `agent.md`（保留旧版本作 `.bak.<timestamp>`）
3. 重新 `mavis harness mount` 验证

## 卸载

```powershell
mavis harness unmount ae-sdd
# 然后手动删除本目录（或脚本带 -Clean 参数）
```

## 元数据

- 生成时间：2026-07-01T03:20:07Z
- ae-sdd 版本：3.7.1
- ae-sdd commit：8b3ca1e
- 适配器：v0.2.0