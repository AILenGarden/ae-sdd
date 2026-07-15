# ae-sdd Harness 适配产物 (AUTO-GENERATED)

> **🔴 本目录由 `ae-sdd-harness-adapter` 自动生成，请勿手工编辑。**

## 这是什么？

把 ae-sdd（`D:\Item\ae-sdd`）的 source 体系（`SKILL.md` + `HARNESS.md`）转译成宿主可消费的 harness 格式（`agent.md`）。

ae-sdd 本身是 **client-agnostic 独立小 Agent**，可挂载到 Claude Code / Codex / ZCode / Mavis 等任意宿主。
本目录只是其中一个宿主的格式适配产物，**ae-sdd 的 SOP 与身份不由本目录定义**（定义见 `source/SKILL.md`）。

## 宿主格式对照

- **Claude Code / Codex / ZCode**：直接读 `source/SKILL.md` slim 入口 + 按需 fallback 到 `source/skill-fallbacks/SKILL.full.md`
- **Mavis harness**：本目录 `agent.md` 是其专用转译产物，由 `mavis harness mount` 注册

## 文件清单

| 文件 | 来源 | 说明 |
|------|------|------|
| `agent.md` | `templates/agent.md.template` 渲染 | 当前宿主用的 harness 主入口 |
| `.adapter.lock` | 脚本生成 | 幂等标记：上次转换的 source input hash + 时间戳 |

## 重新生成

ae-sdd 母版输入升级后（`source/SKILL.md`、`source/HARNESS.md` 或模板变化），跑：

```bash
python scripts/build_harness.py --source "D:\Item\ae-sdd"
```

脚本会自动：
1. 检测到 source input hash 变化 → 触发重转
2. **identity sanity check**：渲染后扫禁词（Mavis Harness / orchestrator），命中则 fail-fast
3. 渲染新 `agent.md`（保留旧版本作 `.bak.<timestamp>`）
4. 对支持 mount 的宿主执行 `mavis harness mount` 验证

## 卸载

宿主的反注册操作按宿主自身 SOP（不一定都是 `mavis`）：

```bash
# 仅当宿主是 Mavis 时
mavis harness unmount ae-sdd
# 然后手动删除本目录（或脚本带 -Clean 参数）
```

## 元数据

- 生成时间：2026-07-15T14:07:30Z
- ae-sdd 版本：3.11.4
- ae-sdd input hash：09037c3576e06b007c85b2e9fec10f5d4c50b58f724a4d799a7004407bd8db08
- 适配器：v0.3.0