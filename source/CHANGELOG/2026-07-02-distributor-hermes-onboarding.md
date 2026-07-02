# 2026-07-02 | ae-sdd - 分发闭环新增 Hermes 分发器

## Summary

用户要求"以后分发还要加上 hermes"。ae-sdd 的自动分发闭环（`distribute.py` + `scripts/distributors/`）此前支持 claude/codex/zcode（copytree 协议）+ mavis（harness_mount 协议）4 个分发目标。本次按插件式分发器架构的既定 3 步法（新建 `<agent>.py` → 注册进 `DISTRIBUTORS` → 可选 compile）新增 `HermesDistributor`，完全对齐 codex/zcode 的标准 copytree 模式（安装目标 `~/.hermes/skills/ae-sdd`，`detect()` 判定目录已存在或 `hermes` CLI 可用）。同步更新 `install.py` 的卸载目标列表/Agent 检测/帮助文案，以及 README、HARNESS.md、ae-sdd-install-skill.md、post-commit hook 注释中列举分发目标的位置。

## Changes

| Area | Change |
|---|---|
| `scripts/distributors/hermes.py` | 新建 `HermesDistributor(CopytreeDistributor)`，`target_path()`=`~/.hermes/skills/ae-sdd`，`detect()`=目录存在或 `hermes`/`hermes.exe` CLI 可用（对齐 codex.py/zcode.py 写法） |
| `scripts/distributors/__init__.py` | `DISTRIBUTORS` 注册表新增 import + 注册 `HermesDistributor`，顺序置于 zcode 之后、mavis 之前（copytree 类都在 mavis 前） |
| `scripts/install.py` | 新增 `HERMES_DST` 常量；`_target_paths()` 新增 `hermes` 分支 + auto 模式检测追加；`_detect_agents()` 新增 hermes CLI 检测；`--target` help 文案、`--uninstall` 合法目标白名单及报错文案追加 hermes |
| `scripts/distributors/_base.py` / `_example.py` | docstring 中列举 copytree 分发器实现的注释追加 hermes（纯说明性，非白名单枚举） |
| `source/skills/orchestration/ae-sdd-install-skill.md` | §0 触发场景表补充 Hermes 安装路径 |
| `source/HARNESS.md` | §自动触发链表格改写为准确反映当前 `distribute.py` 单入口 + `DISTRIBUTORS` 遍历机制的描述（原表述是 v3.4.0 之前逐个 `install.py --target-path` 硬编码调用的过时描述，且漏了 codex；顺带修正） |
| `README.md` | 安装路径清单补充 Hermes 一行 |
| `.githooks/post-commit` | 注释中的分发器列举追加 hermes |

## 触发原因

- 用户显式指示："以后分发还要加上 hermes"

## 影响范围

- 纯新增分发目标，不改变现有 claude/codex/zcode/mavis 4 个分发器的行为
- `tools/lib/paths.py` 的 `locate_master_source()` 安装路径候选列表本身只覆盖 claude/codex（连 zcode 都未覆盖），是 bootstrapping 逻辑与分发链路的既有缺口，不属于本次"分发新增 hermes"任务范围，未处理，留待后续单独修复
- 不影响母版版本号（`source/SKILL.md` version 未变），UC-01 三处一致性不受影响
- 不改变门禁注册、CLI 命令契约、state 字段

## 验证方式

- `python -c "from distributors import DISTRIBUTORS; print([d().name for d in DISTRIBUTORS])"` → `['claude', 'codex', 'zcode', 'hermes', 'mavis']`
- `get_active_distributors(target_filter='hermes')` → 命中 1 个，`target_path()` = `~/.hermes/skills/ae-sdd`
- `python tools/bin/ae-sdd update-check` UC-01~13 全绿
- `python tools/tests/run.py` 380 tests，16 errors/1 skipped——与改动前（`git stash` 基线）完全一致，确认无新增回归（16 个失败均为 Windows GBK 编码 + 缺 pytest 依赖的预置环境问题，与本次改动无关）

## Reviewer

待用户确认。
