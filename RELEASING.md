# 发版包指南（Release Package Guide）

> 本文件面向 **ae-sdd 维护者**，定义发版包 `dist/ae-sdd/` 的构成、边界、版本策略与发布流程。
> 用户安装指引见 [`README.md`](README.md) §🚀 快速安装；本文不重复。

---

## 1. 发版包构成

`dist/ae-sdd/` 是 `scripts/build_dist.py` 生成的**编译后运行包**，安装后即 ae-sdd 运行时本体。固定 13 项：

| 项 | 来源 | 说明 |
|---|---|---|
| `SKILL.md` | `source/SKILL.md`（字节级一致） | 主入口 bootloader |
| `HARNESS.md` | `source/HARNESS.md` | Harness 适配契约 |
| `VERSION` | **注入** | `<version>\n<build-timestamp-UTC>` |
| `.claude-plugin/plugin.json` | **注入** | plugin 自描述元数据（name/version/description） |
| `runtime/` | `compile_skill_runtime.py` 生成 | 编译后 compact slices（boot/flow/gates/route/macros/subskills + manifest.json + fallback/） |
| `skills/` | `source/skills/` | 28 个子 SKILL slim entry |
| `skill-fallbacks/` | `source/skill-fallbacks/` | slim entry 的 full 语义回退 |
| `standards/` | `source/standards/` | 约束 + 标准 |
| `templates/` | `source/templates/` | 21 份模板 |
| `assets/` | `source/assets/` | 项目资产 |
| `harness/` | `source/harness/` | harness 资产 |
| `docs/` | `source/docs/`（仅 `ae-sdd-conventions.md`） | 面向用户的约定文档（`docs/` 整体剔除，仅此一份例外保留） |
| `scripts/` | `scripts/`（白名单） | runtime 扫描器（`*_authenticity_scan.py` / `ra_*.py`） |
| `tools/` | `tools/`（剔除 `tests/`） | CLI `bin/ae-sdd` + `lib/` 运行时模块 |

> 校验当前发版包：`bash scripts/build-dist.sh && ls dist/ae-sdd/`

---

## 2. 发版包边界（核心规则）

**这是 ae-sdd 最关键的一条发版纪律。**

### 2.1 取材来源（只这两个）

```
source/  ──(剔除 CHANGELOG/ docs/ .idea/ .claude-plugin/marketplace.json)──┐
                                                                            ├──> dist/ae-sdd/
tools/    ──(剔除 tests/)──────────────────────────────────────────────────┘
                                                                 + 注入 VERSION / plugin.json
                                                                 + 编译 runtime/
```

对应 `scripts/build_dist.py`：
- `EXCLUDE_DIRS = {"CHANGELOG", "docs", ".idea"}`
- `EXCLUDE_FILES = [".claude-plugin/marketplace.json"]`
- `DOCS_KEEP = ["docs/ae-sdd-conventions.md"]`（唯一例外保留）
- `_copy_tools_to_dist`：剔除 `tests/`
- `_copy_runtime_scripts_to_dist`：按白名单复制 runtime 扫描器

### 2.2 顶层目录归类（哪些不进发版包）

| 顶层目录 | 是否进 `dist/ae-sdd/` | 原因 |
|---|---|---|
| `source/` | ✅ 进（取材源，剔除部分） | 母版 SSOT |
| `tools/` | ✅ 进（剔除 `tests/`） | 运行时工具 |
| `dist/` | — | 它本身就是产物，git ignored |
| `scripts/` | ❌ 不进（仅白名单扫描器进） | 构建/安装脚本，属于仓库级工具 |
| `apps/` | ❌ 不进 | 配套可视化应用，独立交付 |
| `docs/` | ❌ 不进 | 仓库级规划文档（归档的 plan） |
| `plugins/` | ❌ 不进 | 外挂 SKILL 注册表 |
| `references/` | ❌ 不进 | 第三方参考资料 |
| `standalone-skills/` | ❌ 不进 | 可复制到其它仓库的独立 SKILL |

**结论：发版包只装 ae-sdd 本体，零杂物。**

### 2.3 安装链路如何指向发版包

```
scripts/install.{sh,ps1}  →  scripts/install.py  →  scripts/distribute.py
   ↓（每次重新构建，保证装的是最新编译产物）
scripts/build_dist.py  →  scripts/compile_skill_runtime.py
   ↓
dist/ae-sdd/  ──(copy)──>  ~/.<agent>/skills/ae-sdd/
```

**门禁保证**：`distribute.py` 的 `runtime verify` 拒绝安装任何不含 `runtime/manifest.json` + `runtime/boot.compact.md` 的包——**即无法绕过 dist 直接装 source/**。分发器只能安装编译后 package。

---

## 3. 版本策略

### 3.1 语义化版本（SemVer 2.0.0）

`MAJOR.MINOR.PATCH`，参见 https://semver.org

- **MAJOR**：不兼容的破坏性变更（重置 MINOR/PATCH）
- **MINOR**：向后兼容的新功能（重置 PATCH）
- **PATCH**：向后兼容的缺陷修复

### 3.2 三处版本号同步（UC-01 一致性门禁）

版本号必须三处一致，由 `tools/lib/update_graph.py:check_uc01_version` 校验：

| 位置 | 字段 |
|---|---|
| `source/SKILL.md` | frontmatter `version:` |
| `tools/lib/paths.py` | `MASTER_VERSION` |
| `README.md` | 第 5 行版本行 |

bump 时统一执行 `ae-sdd bump <version>`（或手动改三处）。**不允许三处漂移**。

> ⚠️ 已知：截至本次整理，`paths.py:MASTER_VERSION`（3.9.11）落后于 `SKILL.md`（3.9.12）。属历史遗留，待下一次 bump 一并修正。

### 3.3 CHANGELOG（一版一文件）

遵循 [Keep a Changelog](https://keepachangelog.com) 精神，但采用**一版一文件**而非单文件追加：

- 每个 release 一个文件：`source/CHANGELOG/YYYY-MM-DD-vX.Y.Z-<slug>.md`
- 正文按 **Added / Changed / Deprecated / Removed / Fixed / Security** 分节
- 设计/架构/模板正文**不写历史变更**（见 AGENTS.md 红线 11）；正文内仅写"详见 CHANGELOG/..."

---

## 4. Cut a Release（发布流程）

```bash
# 1. 确认 source/ 改动完成、三处版本号一致
#    （SKILL.md frontmatter / paths.py MASTER_VERSION / README.md:5）

# 2. 重新构建发版包
bash scripts/build-dist.sh

# 3. 跑一致性校验（UC-01 版本 / UC-07 分发闭包 / UC-14 update-skill 级联）
#    等价：ae-sdd iteration-check
python tools/lib/update_graph.py

# 4. 确认发版包干净（13 项，无杂物）
ls dist/ae-sdd/

# 5. 写 CHANGELOG（一版一文件）
#    source/CHANGELOG/YYYY-MM-DD-vX.Y.Z-<slug>.md

# 6. 提交 + 打 tag + 推送
git add -A && git commit -m "release: vX.Y.Z <slug>"
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin <branch> --tags
```

---

## 5. 改了顶层结构后必须做什么

**这条保证「以后每次更改都不影响发版包结构」**，由 `ae-sdd-update-skill` 级联驱动。

当你**新增/移动/删除顶层目录**时，必须级联：

1. **`source/standards/update-graph.json`** 新增/更新 `repo_layout` 规则节点——声明顶层允许留存的目录白名单。任何未登记的顶层目录会被 `ae-sdd-update-skill` 标记为需要处理。
2. **`tools/lib/update_graph.py` UC-07**（`check_uc07_distribution_closure`）的 `check_repo_layout_contract` 子检查会断言：顶层不存在 scratch 残留（`nul` / `_tmp_*` / `README.docx` / `update-doc` / `logs`），违反则 FAIL。
3. **本文件 §1/§2.2** 的构成表与归类表同步更新。
4. **`README.md` §📦 仓库结构** 的 ASCII 树同步更新。

这四步是 `ae-sdd-update-skill` 的机器可读契约（`source/standards/update-graph.json`），不是靠人记。

---

## 参考

- 设计契约：[`source/docs/ae-sdd-implementation-architecture.md`](source/docs/ae-sdd-implementation-architecture.md) §8 构建与分发
- 编译与 Runtime IR：[`source/docs/ae-sdd-design.md`](source/docs/ae-sdd-design.md) §18
- 级联机制：[`source/skills/orchestration/ae-sdd-update-skill.md`](source/skills/orchestration/ae-sdd-update-skill.md)
- 版本号一致性门禁：`tools/lib/update_graph.py` UC-01
