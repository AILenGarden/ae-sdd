# ae-sdd Harness 适配产物 (AUTO-GENERATED)

> **🔴 本目录由 Rust `ae-sdd-build harness` 自动生成，请勿手工编辑。**

## 这是什么？

把 ae-sdd（`D:\Item\ae-sdd`）的 source 体系（`SKILL.md` + `HARNESS.md`）转译成宿主可消费的 harness 格式（`agent.md`）。

ae-sdd 本身是 **client-agnostic 独立小 Agent**，可挂载到 Claude Code / Codex / ZCode / Harness 等任意宿主。
本目录只是其中一个宿主的格式适配产物，**ae-sdd 的 SOP 与身份不由本目录定义**（定义见 `source/SKILL.md`）。

## 宿主格式对照

- **Claude Code / Codex / ZCode**：直接读 `source/SKILL.md` slim 入口 + 按需 fallback 到 `source/skill-fallbacks/SKILL.full.md`
- **Harness**：本目录 `agent.md` 是其专用转译产物，由 `harness mount` 注册

## 文件清单

| 文件 | 来源 | 说明 |
|------|------|------|
| `agent.md` | `templates/agent.md.template` 渲染 | 当前宿主用的 harness 主入口 |
| `.adapter.lock` | Rust build tool 生成 | 幂等标记：上次转换的 source input hash + 时间戳 |

## 重新生成

ae-sdd 母版输入升级后（`source/SKILL.md`、`source/HARNESS.md` 或模板变化），跑：

```bash
cargo run -p ae-sdd-build --release -- harness --source "D:\Item\ae-sdd\source\SKILL.md" --source "D:\Item\ae-sdd\source\HARNESS.md" --target "D:\Item\ae-sdd\.harness\agent.md" --title "ae-sdd Agent Harness" --allowed-root "D:\Item\ae-sdd"
```

Rust build tool 会自动：
1. 检测到 source input hash 变化 → 触发重转
2. **identity sanity check**：渲染后扫禁词（Harness 归属句 / orchestrator），命中则 fail-fast
3. 渲染新 `agent.md`（保留旧版本作 `.bak.<timestamp>`）
4. 对支持 mount 的宿主执行 `harness mount` 验证

## 卸载

宿主的反注册操作按宿主自身 SOP（不一定都是 `harness`）：

```bash
# 仅当宿主是 Harness 时
harness unmount ae-sdd
# 然后手动删除本目录（或脚本带 -Clean 参数）
```

## 元数据

- 生成时间：2026-07-23T04:33:48Z
- ae-sdd 版本：3.14.0
- ae-sdd input hash：556be6fb789e9844522edbaefe4aaa225188715de0dd2fff92e291cad25ae9af
- 适配器：v0.3.0
