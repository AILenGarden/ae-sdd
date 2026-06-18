# 2026-06-10 marketplace 副本生成 + sync 脚本扩展 + plugin.json 改名

## 变更摘要

围绕「`D:\Item\ae-sdd\.claude-plugin\marketplace.json` 与仓库实际结构不一致」的对齐工作，集中完成 4 类整改：

1. **母版** 新增 `.claude-plugin/plugin.json`（name=ae-sdd），与 `marketplace.json` 同级，为 plugin 副本提供自描述元数据来源。
2. **同步脚本** `scripts/sync-to-plugin.sh` 从「单目标 + 路径 bug」扩展为「双目标 + 防递归 + 入口路径修正」。
3. **本机已装 plugin** `~/.claude/skills/ae-sdd/.claude-plugin/plugin.json` 的 `name` 字段从历史值 `auto-engineering` 改为 `ae-sdd`。
4. **SKILL 维护规范** `skills/orchestration/ae-sdd-update-skill.md` 同步刷新路径与脚本职责描述（之前残留 `auto-engineering` 路径 + 错误的"脚本不负责维护 plugins/ae-sdd/"描述）。

## 背景

`commit 4a5f894 chore: register Claude Code marketplace` 在 `.claude-plugin/marketplace.json` 中预声明了 `plugins[0].source = "./plugins/ae-sdd"`，并备注「plugins/ae-sdd/ 副本尚未生成，由后续发布流程产出」——直到本次整改前，`plugins/` 目录从未生成，`marketplace.json` 一直处于"悬空引用"状态。

同时核查发现：
- 本机 `~/.claude/skills/ae-sdd/.claude-plugin/plugin.json` 内 `name` 仍为旧名 `auto-engineering`（目录名已是 `ae-sdd`），改名只走完了一半。
- `scripts/sync-to-plugin.sh` 第 16 行 `cp "$DST/ae-sdd-skill.md" "$DST/SKILL.md"` 引用的源文件路径错误：`ae-sdd-skill.md` 实际在 `skills/orchestration/` 下，不在根。脚本会在该行因 `set -e` 退出。

## 详细变更

### 任务 1：母版新增 `.claude-plugin/plugin.json`

**新文件：** `D:\Item\ae-sdd\.claude-plugin\plugin.json`

```json
{
  "name": "ae-sdd",
  "description": "AE 端到端自动化工程 SKILL 家族 — DR→Story→Task→Coding→Test 全流程",
  "version": "1.0.0",
  "author": { "name": "AILenGarden" },
  "homepage": "https://github.com/AILenGarden/ae-sdd",
  "repository": "https://github.com/AILenGarden/ae-sdd",
  "license": "MIT",
  "keywords": ["auto-engineering", "ae-sdd", "skill", "sdd", "story", "task", "coding", "code-review"],
  "category": "development"
}
```

**作用：** 作为 plugin 副本的元数据源，被 `sync-to-plugin.sh` 携带进 `plugins/ae-sdd/.claude-plugin/plugin.json` 和 `~/.claude/skills/ae-sdd/skills/ae-sdd/.claude-plugin/plugin.json`。

### 任务 2：扩展 `scripts/sync-to-plugin.sh`

**改动文件：** `D:\Item\ae-sdd\scripts\sync-to-plugin.sh`

**核心变化：**

| 维度 | 旧 | 新 |
|------|---|---|
| 同步目标数量 | 1（本机 Claude skills） | **2**（+ 仓库内 `plugins/ae-sdd/`） |
| 复制方式 | `cp -r "$SRC/." "$DST/"` | `tar` 管道复制并 `--exclude='./plugins' --exclude='./.git'`（避免 DST 在 SRC 子目录时递归） |
| SKILL.md 入口源 | `$DST/ae-sdd-skill.md`（**bug：源不在根**） | `$DST/skills/orchestration/ae-sdd-skill.md` |
| 副本剥离 | 无 | 显式 `rm -f $DST/.claude-plugin/marketplace.json`（plugin 副本不持有 marketplace 注册表）；保留 `plugin.json` |
| 入口源缺失 | `set -e` 退出 | 改为 warning，不中断脚本 |

**关键设计点：**
- `plugins/` 必须被 tar `--exclude`，否则 `cp/tar` 把 `plugins/ae-sdd/` 复制到 `plugins/ae-sdd/plugins/ae-sdd/`，无限递归。
- `marketplace.json` **只在仓库根**，不进副本（plugin 副本是 marketplace 的被引用方，不是引用方）。
- `plugin.json` **必须进副本**（每个 plugin 自描述）。

### 任务 3：修复本机安装 `plugin.json` 的 `name`

**改动文件：** `C:\Users\EDY\.claude\skills\ae-sdd\.claude-plugin\plugin.json`

```diff
- "name": "auto-engineering",
+ "name": "ae-sdd",
```

**影响：** 与目录名、marketplace.json 中 `plugins[0].name` 完全对齐，消除三处命名不一致。

### 任务 4：同步刷新 `ae-sdd-update-skill.md`

**改动文件：** `D:\Item\ae-sdd\skills\orchestration\ae-sdd-update-skill.md`

| 章节/行 | 旧 | 新 |
|---------|---|---|
| §默认规则 行 213 | `plugins/ae-sdd/` "不手工改；由发布流程从母版生成" | "由 `bash scripts/sync-to-plugin.sh` 从母版生成"（指明就是这个脚本） |
| §默认规则 行 214 | `…/skills/ae-sdd/skills/auto-engineering` | `…/skills/ae-sdd/skills/ae-sdd` |
| §默认规则 行 215 | `SKILL.md` "由 ae-sdd-skill.md 复制生成" | "由 `skills/orchestration/ae-sdd-skill.md` 复制生成"（明确源路径） |
| §修改后动作 行 229-231 | 同步目标 1 个 | 同步目标 **2 个**（本机 Claude skills + `plugins/ae-sdd/`） |
| §同步脚本说明 行 238-240 | "脚本不负责维护仓库内 `plugins/ae-sdd/` 副本" | 改为 4 条职责清单，明确双目标 + 剥离规则 + 入口刷新 |

## 验证

```bash
$ bash D:/Item/ae-sdd/scripts/sync-to-plugin.sh
✅ 本地 Claude skills 同步完成 → C:/Users/EDY/.claude/skills/ae-sdd/skills/ae-sdd
✅ marketplace plugin 副本同步完成 → /d/Item/ae-sdd/plugins/ae-sdd
```

**结构验证（两个目标均通过）：**

- `.claude-plugin/` 内只有 `plugin.json`（无 `marketplace.json`）
- 根下没有 `plugins/` 子目录（防递归 PASS）
- `SKILL.md` 字节数 = `skills/orchestration/ae-sdd-skill.md` 字节数 (98676)
- `plugin.json` 内 `name = "ae-sdd"`

## 未处理的相关问题（不在本次范围）

| 问题 | 说明 | 建议 |
|------|------|------|
| 母版根 `SKILL.md` (98665) 与 `skills/orchestration/ae-sdd-skill.md` (98676) 差 11 字节 | 按 update-skill §215 规定，母版根 `SKILL.md` 应由 `ae-sdd-skill.md` 复制生成；但当前 sync 脚本只刷新 DST，不刷新母版根 | 下次修改 ae-sdd-skill.md 时在 update-skill 流程中显式刷新母版根 SKILL.md，或扩展脚本支持 `--refresh-master` |
| 母版 `skills/orchestration/ae-sdd-skill.md` frontmatter `name: auto-engineering` | 与 plugin name/目录名不一致；Claude 加载 SKILL 时同时暴露 `ae-sdd` 与 `auto-engineering:ae-sdd` 两个名 | 另起 PR 改 frontmatter，并扫所有交叉引用 |
| `README.md` 行 5 与行 583 提到的 `plugins/ae-sdd/同步` | 本次脚本扩展后名副其实，但描述仍是预声明语气 | 下次 README 整理时同步措辞 |

## Reviewer

- 用户在 2026-06-10 选定全部 4 项整改（含本机 plugin.json 改 name + sync 脚本扩展 + 母版加 plugin.json + 跑脚本验证），并要求顺便检查 ae-sdd-update-skill 中 sync 相关描述
- 用户提示：sync-to-plugin.sh 是已存在的关键资产，扩展时不要破坏既有行为
