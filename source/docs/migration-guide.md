---
name: migration-guide
description: ae-sdd 存量文档迁移指南 — 如何用 migrate-docs.mjs 把 design/、.ae-task/、.ae-plan/、.spec/iterations/ 下的旧文档迁到 ae-sdd-doc/ 统一目录。
---

# ae-sdd 存量文档迁移指南

> **2026-06-17 新建**：配合 `document-storage-skill.md` 统一目录升级（`ae-sdd-doc/`），提供独立迁移工具 `migrate-docs.mjs`。

---

## 0. 何时需要迁移？

| 场景 | 是否需要迁移 |
|------|------------|
| **新建项目**（首次用 AE 流程）| ❌ 直接用新路径 `ae-sdd-doc/`，无需迁移 |
| **老项目首次升级**（已有 `design/` 文档）| ✅ 建议跑一次迁移，把历史文档归档到 `ae-sdd-doc/` |
| **周期性整理**（半年/一年一次）| ✅ 跑迁移把分散文档集中 |
| **修复路径混乱**（设计类文档散落各处）| ✅ 跑迁移统一入口 |

**🔴 原则：** 新文档**必须**写入 `ae-sdd-doc/`，**禁止**写入旧路径。迁移是一次性的"历史归档"动作，不是持续流程。

---

## 1. 旧路径到新路径的映射表

> **🔴 与 `document-storage-skill.md §8.1` 保持同步。** 任何路径映射变更必须同时更新两个文件。

| 旧路径 | 状态 | 新路径 | 备注 |
|--------|------|--------|------|
| `design/dr/{projectKey}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/DR/{doc-id}-v1.0.md` | DR 主文档 |
| `design/story/be/{STORY-ID}.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Story/{STORY-ID}-v1.0.md` | Story 主文档 |
| `design/story/be/{STORY-ID}/{X}.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Story/{STORY-ID}/{X}.md` | Story 子目录（Supplement/WriterReport） |
| `design/story/be/task/{STORY-ID}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Task/{STORY-ID}/*.md` | Task 文档 |
| `design/story/be/coding/{STORY-ID}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Coding/{STORY-ID}/*.md` | Coding 报告/追溯矩阵 |
| `design/story/be/review/{STORY-ID}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/CR/{STORY-ID}/*.md` | Story Review 报告 |
| `design/testcase/be/{STORY-ID}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Test/{STORY-ID}/*.md` | 测试用例/报告 |
| `{工程根}/.ae-task/Task-*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Task/{doc-id}-v1.0.md` | 小任务文档 |
| `{工程根}/.ae-plan/Plan-*/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/Coding/{doc-id}-v1.0.md` | 微任务文档 |
| `.spec/iterations/{iter}/{type}/*.md` | ⚠️ deprecated | `ae-sdd-doc/iterations/{date}/{type}/{doc-id}-v1.0.md` | 历史 Spec 文档 |
| `.auto-engineering/{STORY-ID}/state.json` | ✅ **保留** | （不迁移）| 状态文件路径不变 |

---

## 2. 工具：`migrate-docs.mjs`

### 2.1 文件位置

- **母版源**：`D:\Item\ae-sdd\scripts\migrate-docs.mjs`
- **跨平台**：Node.js 18+（Windows / macOS / Linux 全支持）
- **零依赖**：仅用 Node.js 内置 `fs` / `path` 模块

### 2.2 核心特性

| 特性 | 说明 |
|------|------|
| **🔴 默认 DRY-RUN** | 不加 `--execute` 不会修改任何文件 |
| **旧文件不删** | 迁移 = 复制到新路径，原路径保留 |
| **版本号自动分配** | 设计类文档默认 v1.0 |
| **ChangeLog 自动生成** | 标记 "迁移自旧路径 ..." |
| **报告输出** | 生成 `_migration-report-{date}.md` 报告 |

### 2.3 用法

#### Step 1：DRY-RUN（先看会迁什么）

```bash
node scripts/migrate-docs.mjs --target /path/to/your/project --dry-run
```

**输出：**
- 控制台打印摘要（按 docType 统计）
- 生成 `ae-sdd-doc/iterations/{date}/_migration-report-{date}.md` 完整报告
- **不修改任何文件**

#### Step 2：确认无误后真正执行

```bash
node scripts/migrate-docs.mjs --target /path/to/your/project --execute
```

**实际行为：**
1. 复制每个匹配的 .md 到新路径
2. 追加 ChangeLog 行
3. 旧文件**保留不删**（人工确认后再清理）

#### Step 3：高级选项

```bash
# 指定迭代日期
node scripts/migrate-docs.mjs --target /path/to/project --date 2026-06-17 --execute

# 指定 ChangeLog 作者
node scripts/migrate-docs.mjs --target /path/to/project --author "cong.chen" --execute

# 查看帮助
node scripts/migrate-docs.mjs --help
```

### 2.4 报告格式

`_migration-report-{date}.md` 包含：

```markdown
# Migration Report - {projectKey} - 2026-06-17

## 模式: DRY-RUN（不执行）

## 扫描结果（按 docType）

| docType | 文件数 |
|---------|--------|
| Story | 12 |
| Task | 8 |
| Coding | 15 |
| ... | ... |
| **合计** | **38** |

## 迁移计划

| # | 源路径 | 目标路径 | doc_id | doc_type |
|---|--------|---------|--------|----------|
| 1 | design/dr/icec-cloud-boss/DR-001.md | ae-sdd-doc/iterations/2026-06-17/DR/DR-001-v1.0.md | DR-001 | DR |
| 2 | design/story/be/STORY-001-BE.md | ae-sdd-doc/iterations/2026-06-17/Story/STORY-001-BE-v1.0.md | STORY-001-BE | Story |
| ... | ... | ... | ... | ... |

## 注意事项
- 旧目录（design/、.ae-task/、.ae-plan/、.spec/iterations/）保留不删除
- ChangeLog 初始行标记 "迁移自旧路径 ..."
- 所有迁移文件默认 v1.0
- 如需回滚：直接删除 ae-sdd-doc/iterations/{date}/ 下的对应文件即可
```

---

## 3. 典型使用流程

### 3.1 老项目首次升级（推荐）

```bash
# 1. 拉取最新 SKILL 母版
cd D:\Item\ae-sdd
git pull

# 2. 复制脚本到目标工程（或通过相对路径调用）
cp scripts/migrate-docs.mjs /d/Item/icec-cloud-boss/scripts/migrate-docs.mjs

# 3. 跑 DRY-RUN 看会迁什么
cd /d/Item/icec-cloud-boss
node ../ae-sdd/scripts/migrate-docs.mjs --target . --dry-run

# 4. 查看报告
cat ae-sdd-doc/iterations/2026-06-17/_migration-report-2026-06-17.md

# 5. 人工确认后真正执行
node ../ae-sdd/scripts/migrate-docs.mjs --target . --execute

# 6. 验证
ls ae-sdd-doc/iterations/2026-06-17/

# 7. （可选）手动删除旧目录
rm -rf design/ .ae-task/ .ae-plan/ .spec/

# 8. 提交
git add -A
git commit -m "refactor: 迁移历史文档到 ae-sdd-doc/"
```

### 3.2 小项目/微任务（直跑一次）

```bash
node scripts/migrate-docs.mjs --target . --date 2026-06-17 --execute --author "cong.chen"
```

### 3.3 批量迁移多个工程

```bash
for project in project1 project2 project3; do
  echo "=== Migrating $project ==="
  cd /d/Item/$project
  node ../ae-sdd/scripts/migrate-docs.mjs --target . --date 2026-06-17 --execute
done
```

---

## 4. 与 `document-storage-skill` 的协同

| 关注点 | `document-storage-skill.md` | `migrate-docs.mjs` |
|--------|---------------------------|-------------------|
| **职责** | AE 流程**新文档**的路径/命名/版本/ChangeLog 标准 | **存量文档**的批量迁移工具 |
| **触发时机** | `save_doc()` API 每次写新文档 | 用户显式 `--execute` |
| **路径** | `ae-sdd-doc/iterations/{date}/{DocType}/` | 同上（迁移目标）|
| **版本号** | `get_latest_version()` 自动递增 | 固定 v1.0（迁移视为首次创建）|
| **ChangeLog** | `save_doc()` 自动追加 | 工具自动追加 "迁移自旧路径 ..." 行 |

**关键协同：**
- 迁移后，文档已经在新路径，AE 流程的 `save_doc()` 会**自动识别最新版本**，不会重复创建
- 迁移不会破坏旧路径的引用（如果其他文档还在引用旧路径，旧文件仍可读）
- 迁移**不会**修改 `.auto-engineering/{STORY-ID}/state.json`（状态文件不迁移）

---

## 5. 回滚

如果迁移后发现问题，回滚很简单（**这就是为什么默认不删旧文件**）：

### 5.1 软回滚（仅删除新路径）

```bash
# 删除新路径下的迁移产物
rm -rf ae-sdd-doc/iterations/{date}/

# 旧文件还在，AE 流程下次再跑会重新写入
```

### 5.2 硬回滚（如果已经手动删了旧文件）

如果有 git 历史：

```bash
# 找回旧文件
git checkout HEAD~1 -- design/ .ae-task/ .ae-plan/ .spec/

# 删除新路径
rm -rf ae-sdd-doc/iterations/{date}/
```

### 5.3 重新跑迁移（修正参数后）

```bash
# 删除上次迁移产物
rm -rf ae-sdd-doc/iterations/{date}/

# 重新跑（带修正后的参数）
node scripts/migrate-docs.mjs --target . --date 2026-06-17 --execute
```

---

## 6. 常见问题

### Q1：迁移会不会破坏 git 历史？

**A：不会。** 工具是"复制"而非"移动"，旧文件还在原位，git 历史完整保留。

### Q2：迁移后旧文件还需要手动删吗？

**A：是的。** 这是**有意为之**——避免误删。迁移完成后，**人工 review 一遍**（确保新路径的文档没问题），再手动删除旧目录。

### Q3：如果同一个 .md 在旧路径下有 v1.0 和 v1.1，迁移会怎么处理？

**A：v1.0 和 v1.1 都会迁移。** 工具按文件名识别，不合并。迁移后两个文件都在新路径，分别带 `-v1.0` / `-v1.1` 后缀。

### Q4：迁移会触发 `check_and_update_gitignore()` 吗？

**A：不会。** 迁移**不**修改 `.gitignore`。需要手动执行 `documentStorage.check_and_update_gitignore()`（或直接 `save_doc()` 一次会自动维护）。

### Q5：迁移报告 `_migration-report-{date}.md` 是什么？

**A：迁移执行的完整记录。** 包含扫描结果、迁移计划、执行结果。下次跑迁移会**覆盖**同名报告（同一 date）。建议作为审计材料保留。

### Q6：可以只迁移某类文档吗？

**A：当前版本不支持。** 如需要过滤，请编辑脚本顶部的 `MIGRATION_RULES` 数组，注释掉不需要的规则。

---

## 7. 维护

- **维护人：** 架构组
- **更新频率：** 路径映射规则变化时（必须与 `document-storage-skill.md §8.1` 同步更新）
- **同步对象：**
  - `document-storage-skill.md §8.1`（路径映射表）
  - `document-storage-skill.md §0.6.14`（`migrate_old_docs()` API 文档）
  - 母版 CHANGELOG（每次规则变更需登记）

---

## 8. 相关文件

| 文件 | 用途 |
|------|------|
| `D:\Item\ae-sdd\scripts\migrate-docs.mjs` | 迁移工具脚本 |
| `D:\Item\ae-sdd\docs\migration-guide.md` | 本文件 |
| `D:\Item\ae-sdd\skills\cross-cutting\document-storage-skill.md` | 文档存放标准（含 §8 存量迁移 API）|
| `D:\Item\ae-sdd\CHANGELOG\` | 母版变更日志 |
