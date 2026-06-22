# 2026-06-10 ae-sdd 命名彻底统一 + sync 脚本扩到三目标 + 母版根 SKILL.md 入脚本管控

## 变更摘要

承接同日 `2026-06-10-marketplace副本生成+sync脚本扩展+plugin改名.md`，针对用户三个追问展开二轮修复：

1. **`scripts/sync-to-plugin.sh`** 把母版根 `SKILL.md` 加入刷新目标，sync 目标从 2 个变 3 个。
2. **`skills/orchestration/ae-sdd-skill.md` frontmatter** `name: auto-engineering` → `name: ae-sdd`。
3. **`skills/orchestration/ae-sdd-update-skill.md` frontmatter** `name: auto-engineering-update` → `name: ae-sdd-update`。
4. **`ae-sdd-update-skill.md` §默认规则 + §修改后动作 + §同步脚本说明** 三段同步刷新，把母版根 `SKILL.md` 纳入"由脚本生成不手改"语义。
5. **`README.md` 行 5 / 行 583** 补充"marketplace 副本落地 + sync 三目标 + frontmatter 改名"措辞。
6. **撤回上一篇 CHANGELOG「未处理的相关问题 #1」的错误建议**（"sync 脚本不刷母版根"）。母版根 SKILL.md 是 git clone / GitHub 首屏入口，**必须存在且必须由脚本刷新**，否则永远会和 ae-sdd-skill.md 漂移。

## 背景（揭示一个长期隐藏的根因）

上一篇 CHANGELOG 留下三个"独立小问题"，本轮排查发现它们其实**同一个根因**——SKILL.md 入口三处源不一致：

| 文件 | frontmatter `name`（修复前） |
|------|---|
| `D:/Item/ae-sdd/SKILL.md`（母版根） | `ae-sdd` ✅（之前被人手工改过） |
| `D:/Item/ae-sdd/skills/orchestration/ae-sdd-skill.md`（真正的源） | `auto-engineering` ❌ |
| sync 脚本生成的两个 DST SKILL.md | `auto-engineering` ❌（继承自源） |

这解释了：
- **11 字节差异** — `auto-engineering`(16 char) vs `ae-sdd`(6 char) 差 10 字符 + 换行 = 11。
- **Claude 系统提示同时暴露 `ae-sdd` 和 `auto-engineering:ae-sdd`** — 因为目录名 / plugin.json name 已是 `ae-sdd`，但 SKILL frontmatter name 还是 `auto-engineering`，两个识别路径出两个结果。
- **上一轮 sync 后 DST 的 SKILL.md 也是 `name: auto-engineering`** — 因为源没改。

## 详细变更

### 任务 1：扩展 `scripts/sync-to-plugin.sh` 加母版根目标

**改动文件：** `D:\Item\ae-sdd\scripts\sync-to-plugin.sh`

在 `sync_to` 函数调用之前，先把母版根 SKILL.md 刷一遍：

```bash
# 母版根 SKILL.md 也由本脚本刷新（避免有人手改母版根 SKILL.md 而源不变，下次跑脚本被默默覆盖）
MASTER_SKILL_SRC="$SRC/skills/orchestration/ae-sdd-skill.md"
if [[ -f "$MASTER_SKILL_SRC" ]]; then
  cp "$MASTER_SKILL_SRC" "$SRC/SKILL.md"
  echo "✅ 母版根 SKILL.md 已刷新 → $SRC/SKILL.md"
fi
```

**效果：** sync 一次 = 三处 SKILL.md 同源 = 永不漂移。

### 任务 2 + 3：两个 SKILL frontmatter `name` 改名

| 文件 | 旧 | 新 |
|------|---|---|
| `skills/orchestration/ae-sdd-skill.md` | `name: auto-engineering` | `name: ae-sdd` |
| `skills/orchestration/ae-sdd-update-skill.md` | `name: auto-engineering-update` | `name: ae-sdd-update` |

`description` 内的 `auto-engineering-skill` 措辞也同步改为 `ae-sdd-skill`（保持指代一致）。

**未改动的措辞：** README 与 SKILL 正文里"auto-engineering 体系（简称 AE）"等概念性叙述保留——这些指的是"端到端自动化工程"这件事，不是 SKILL 名引用。

### 任务 4：`ae-sdd-update-skill.md` 三段同步刷新

| 章节 | 变更 |
|------|------|
| §默认规则 | 新增一行：母版根 `SKILL.md` "由 `bash scripts/sync-to-plugin.sh` 从 `skills/orchestration/ae-sdd-skill.md` 复制刷新" |
| §修改后动作 行 229-231 | sync 目标从 2 个改为 **3 个**（含母版根）|
| §同步脚本说明 | 脚本职责清单从 4 条扩为 **5 条**，第 1 条新增"刷新母版根 SKILL.md" |

### 任务 5：`README.md` 行 5 / 行 583 措辞

行 5 版本说明、行 583 "最后更新" 都追加：

> 🆕🆕 marketplace 副本 `plugins/ae-sdd/` 实际落地（不再悬空）+ sync 脚本扩展为三目标统一刷新（母版根 SKILL.md / 本机安装 / plugin 副本）+ 修复 sync 脚本入口路径 bug + ae-sdd-skill / ae-sdd-update-skill frontmatter `name` 字段统一改为 `ae-sdd` / `ae-sdd-update`

### 任务 6：撤回上篇错误结论

**上篇** `2026-06-10-marketplace副本生成+sync脚本扩展+plugin改名.md` §未处理的相关问题 #1 写：

> 母版根 SKILL.md (98665) 与 skills/orchestration/ae-sdd-skill.md (98676) 差 11 字节…当前 sync 脚本只刷新 DST，不刷新母版根
> **建议：** 下次修改 ae-sdd-skill.md 时在 update-skill 流程中显式刷新母版根 SKILL.md，或扩展脚本支持 `--refresh-master`

**用户反问"脚本刷母版根干嘛？"——澄清后确立：**

- ❌ **错误结论：** "sync 脚本不刷母版根 / 删掉母版根 SKILL.md"
- ✅ **正确结论：** 母版根 SKILL.md 是 GitHub 首屏可见的 SKILL 入口，**必须存在 + 必须由脚本刷新**，不能让人手改（手改了就和源漂移，下次跑脚本被默默覆盖，且改的内容白丢）

本篇 CHANGELOG 取代上篇 §未处理的相关问题 #1。

## 验证

```bash
$ bash D:/Item/ae-sdd/scripts/sync-to-plugin.sh
✅ 母版根 SKILL.md 已刷新 → /d/Item/ae-sdd/SKILL.md
✅ 本地 Claude skills 同步完成 → C:/Users/EDY/.claude/skills/ae-sdd/skills/ae-sdd
✅ marketplace plugin 副本同步完成 → /d/Item/ae-sdd/plugins/ae-sdd

$ md5sum 4_files
834ecab0a29da0916cd829f2a7035634 *D:/Item/ae-sdd/SKILL.md
834ecab0a29da0916cd829f2a7035634 *D:/Item/ae-sdd/plugins/ae-sdd/SKILL.md
834ecab0a29da0916cd829f2a7035634 *C:/Users/EDY/.claude/skills/ae-sdd/skills/ae-sdd/SKILL.md
834ecab0a29da0916cd829f2a7035634 *D:/Item/ae-sdd/skills/orchestration/ae-sdd-skill.md
```

四处文件 md5 完全相同；frontmatter `name: ae-sdd` 三个 SKILL.md 入口一致。

## 仍未处理（已不在本次范围）

- Claude 当前会话已加载的 SKILL 缓存依旧暴露 `auto-engineering:ae-sdd` 这个 namespace 形式 — 这是会话内快照，**重启 Claude 后**根据新 plugin.json (`name: ae-sdd`) 和新 frontmatter 应统一显示为 `ae-sdd` 或 `ae-sdd:ae-sdd`。

## Reviewer

- 用户 2026-06-10 三个追问：(1)"脚本刷母版根干嘛?" (2)"统一改成 ae-sdd 呀" (3)"调整"
- 关键澄清：用户在 (1) 的追问中明确表达"母版根 SKILL.md 是唯一入口，不该删，否则入口很不清晰" — 这反向确立了"母版根 SKILL.md 必须由脚本刷新"的方案
