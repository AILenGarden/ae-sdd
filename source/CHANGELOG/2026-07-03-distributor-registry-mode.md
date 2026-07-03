# 2026-07-03 分发器注册表模式改造

## 触发原因

用户需求：分发器应改为注册表模式，通过注册和注销管理分发目标。例如当前环境无 mavis daemon，应能注销 mavis 使 dev-sync 不再尝试它。注册过程应扫描 Agent 的 skill 安装模式生成专属分发配置，注销则在注册表除名。

## 改造内容

### 核心架构：外部 JSON 注册表 + 协议模板

从 `scripts/distributors/__init__.py` 的硬编码 `DISTRIBUTORS` Python 列表，改为 `~/.ae-sdd/distributors.json` 外部注册表驱动。

- **注册表位置**：`~/.ae-sdd/distributors.json`（用户环境态，与 plugins/ 同级）
- **首次运行**：无文件时用 `_default_distributors()` 种子初始化（含 claude/codex/zcode/hermes/mavis，mavis 默认 `enabled:false`）
- **协议模板**：`copytree`（CopytreeDistributor，简单 backup→copy→verify）+ `harness_mount`（HarnessMountDistributor，复杂 compile→mount→cleanup）。注册一个 Agent = 选模板 + 填参数构造实例

### 代码改动

| 文件 | 改动 |
| --- | --- |
| `tools/lib/distributor_registry.py`（新增） | 注册表读写 + scan 扫描 + register/unregister/enable/disable 逻辑 |
| `scripts/distributors/_base.py` | `CopytreeDistributor` 改为数据驱动（`__init__` 接受 name/target_path/detect_fn）；新增 `HarnessMountDistributor`（从 mavis.py 迁入逻辑） |
| `scripts/distributors/_registry.py` | 重构：读 JSON 注册表，按 enabled+detect 构造对应协议模板实例 |
| `scripts/distributors/__init__.py` | `DISTRIBUTORS` 列表置空（兼容保留），导出 HarnessMountDistributor |
| `scripts/distributors/claude.py` 等 5 个 | 降级为兼容 shim（继承模板 + 硬编码参数，旧测试不破） |
| `scripts/distribute.py:184` | 旧 --target-path 兼容路径改为直接构造 CopytreeDistributor |
| `tools/bin/ae-sdd` | 新增 `distributor` 子命令组（list/register/unregister/enable/disable/scan） |
| `tools/tests/test_distributor_registry.py`（新增） | 15 个单测覆盖注册/注销/扫描/enable/disable |

### CLI 命令

```
ae-sdd distributor list                              # 列出注册表
ae-sdd distributor register <name> --protocol ... --target-path ...  # 注册
ae-sdd distributor unregister <name>                 # 硬注销（删条目）
ae-sdd distributor enable <name>                     # 启用
ae-sdd distributor disable <name>                    # 软注销（enabled:false）
ae-sdd distributor scan [--register] [--all-agents]  # 扫描建议注册
```

## 验证

- `distributor list`：✅ 5 个分发器，mavis 默认禁用
- `distributor disable/enable/unregister/register`：✅ 全流程工作
- `distributor scan --all-agents`：✅ 扫描到 5 个已知 Agent
- `dev-sync`：✅ **mavis 不再出现在分发汇总**（被 enabled:false 跳过），claude/zcode/hermes 正常分发
- `update-check`：✅ 全绿
- 单测：`test_distributor_registry.py` 15 passed + 旧 `test_*_distributor.py` 12 passed（shim 兼容）

## 影响范围

- 能力语义变化：分发目标管理从代码硬编码改为注册表驱动。已同步 `ae-sdd-implementation-architecture.md`。
- 向后兼容：首次运行种子初始化等价现状；旧 `--target`/`--target-path` 参数保留；旧 `*.py` shim 保留。
- 未受影响：build_dist.py 编译逻辑、post-commit hook、实例化体系。

## 设计决策

- **不生成 .sh/.ps1 脚本**：mavis 的 harness_mount 涉及 build_harness/mavis CLI/sqlite，shell 生成极脆弱。协议模板内置 + 数据填参是"动态生成"的工程化实现，更可控。
- **不委托 Agent 安装**：scan 只探测建议，不调 Agent CLI 装东西。Agent 的 skill 安装是 Agent 自己的事。
- **mavis 默认禁用**：反映当前环境无 daemon，避免 dev-sync 失败。需要时 `ae-sdd distributor enable mavis`。

## Reviewer

陈聪（用户决策：外部 JSON 注册表 + 全动态生成分发脚本 + 显式注册+可选扫描）
