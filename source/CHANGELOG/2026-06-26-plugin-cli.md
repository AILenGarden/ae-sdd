# v3.5.1 — plugin CLI 挂载闭环（2026-06-26）

## 核心目标

把 v3.5.0 完成的 `tools/lib/plugin_loader.py`（35 个单元测试全过）暴露成 CLI 子命令，
让用户/Agent 在终端直接操作三层 SKILL 注册表，**不再需要 import lib 调用**。

## 改动

### 新增 CLI 子命令（4 个 + 11 个单元测试）

```bash
ae-sdd plugin list              # 列出三层注册表所有已注册插件 + 冲突检测
ae-sdd plugin validate          # 校验三层注册表 + 每个 plugin sanity check
ae-sdd plugin trace <target>    # 查某 SKILL 的加载路径（replaces 路径 / provides key）
ae-sdd plugin init --layer {project|global}   # 从模板生成新注册表（--force 覆盖）
```

| 文件 | 变更 |
|------|------|
| `tools/bin/ae-sdd` | import `plugin_loader` + 4 个 `cmd_plugin_xxx` 函数 + parser 注册 + 头注释 CLI 用法 |
| `tools/tests/test_plugin_cli.py` | 11 个 CLI 单元测试（隔离 HOME/USERPROFILE 避免污染用户配置） |
| `tools/lib/plugin_loader.py` | 修 `plugin_registry_path_master` 路径解析（locate_master_source 返回 source/，L3 注册表按设计在仓库根 plugins/） |

### 文档更新

| 文件 | 变更 |
|------|------|
| `source/SKILL.md` | frontmatter 加 v3.5.1 描述行 + version 3.5.0 → 3.5.1 |
| `README.md` | 版本行 v3.5.0 → v3.5.1 + v3.5.0 描述里"CLI 子命令留待 v3.5.1"改为"v3.5.1 已挂载"+ ## 🔌 SKILL 注册与外挂指南 §4 CLI 状态从"v3.5.0 完成 Python 模块"改为"v3.5.1 已完成挂载 — 4 个子命令可用" |

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| CLI 命令风格 | `sub.sub` 二级子命令 + `parents=[common]` | 跟 ae-sdd 现有 CLI 风格一致（state / memory / db / git / assets） |
| 隔离 HOME/USERPROFILE | 测试用 `tempfile.mkdtemp()` 覆盖 | Windows `Path.home()` 读 USERPROFILE，Unix 读 HOME；同时覆盖才能避免污染用户真实配置 |
| plugin init `--layer` | 只支持 `project` / `global`（不支持 master） | master 层是 ae-sdd 团队专用，普通用户用不到 |
| init 默认不覆盖 | 加 `--force` 才覆盖 | 防误操作破坏用户已有注册表 |
| trace target | 接 `replaces` 内置路径 或 `provides` key | 路由算法 step 2.5 也是这样匹配的 |
| L3 master 路径 | `master.parent / "plugins" / "registry.yaml"` | locate_master_source 返回 source/，仓库根 = source.parent |

## 路径解析修复（重要）

`plugin_registry_path_master` 之前实现是 `master / "plugins" / "registry.yaml"`，
但 `paths.locate_master_source()` 返回的是 `source/` 目录（含 SKILL.md），
所以之前会解析成 `<repo>/source/plugins/registry.yaml`——错。

**修复**：`master.parent / "plugins" / "registry.yaml"`（仓库根的 plugins/），
与设计文档 §2.1 L3 仓库根层定义一致。

## 测试矩阵

| 测试 | 覆盖点 |
|------|--------|
| `test_list_no_layers` | 三层都未注册时 JSON 输出结构正确 |
| `test_list_human_readable` | human-readable 输出含三层 label |
| `test_validate_no_layers_passes` | 三层都未注册 → valid=true |
| `test_validate_human_readable_passes` | human-readable 输出含"校验通过" |
| `test_trace_fallback_when_no_registry` | 三层未注册 → fallback 到 L0-builtin |
| `test_trace_human_readable` | human-readable 输出含"fallback" 和 "L0-builtin" |
| `test_trace_requires_target` | 缺 target → argparse error exit 2 |
| `test_init_global_creates_file` | global layer 成功创建注册表 |
| `test_init_global_already_exists_no_force` | 已存在 → 拒绝（exit 1） |
| `test_init_global_with_force_overwrites` | --force 覆盖 |
| `test_init_project_requires_ae_sdd_dir` | project layer 需要 .ae-sdd/ 存在 |

**11/11 CLI 测试通过**，配合 v3.5.0 的 35 个 loader 测试，**46/46 全过**。

## 用户使用闭环

之前（v3.5.0）：
- 用户必须自己写 Python 代码调 `plugin_loader.resolve_skill(...)` 才能验证
- 注册流程靠手动 cp + 写 YAML

现在（v3.5.1）：
```bash
# 1. 生成项目层注册表
ae-sdd plugin init --layer project

# 2. 编辑生成的 registry.yaml + 写外挂 SKILL

# 3. 验证
ae-sdd plugin validate

# 4. 查加载路径
ae-sdd plugin trace source/skills/phase2-coding/coding-skill.md
```

## 验证状态

- ✅ 46/46 单元测试通过（35 loader + 11 CLI）
- ✅ CLI smoke test：4 个子命令 help + execute 全部正常
- ✅ CLI 路径解析修对（L3 指向 `<repo>/plugins/registry.yaml`）
- ✅ 测试隔离 HOME/USERPROFILE，不污染用户真实配置
- 待验证：跑 `ae-sdd update-check` 看 UC-03 警告（plugin 命令已挂载，警告应消失）