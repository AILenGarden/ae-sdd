# 2026-07-01 | document_storage 激活阶段2：276 处文档规范化（v3.7.2）

## Summary

把 20 个 SKILL 文档里的 276 处 `save_doc()`/`resolve_path()` 伪代码调用规范为真实可执行的 `ae-sdd doc save/resolve/finalize` CLI 命令引用。LLM 读到 SKILL 后可直接执行命令，而非读伪代码手拼路径。这是路线B（CLI + hook + 文档规范化）的阶段2。

承接阶段1（已让 `ae-sdd doc` CLI 可用 + 修 2 个 bug），本次让文档层的"调用契约"与代码层的"执行入口"对齐。

## Changes

### 核心改造：伪代码 → CLI 命令

每处调用按用途规范化为两类命令：
- **写入用途**（"落地文档"步骤）→ `ae-sdd doc save --intent X --content-file 草稿.md`
- **读取用途**（"读取上下文"步骤）→ `ae-sdd doc resolve --intent X`

### 按批次（4 批 20 文件）

| 批次 | 文件 | 改动量 |
|------|------|--------|
| 第1批 高密度核心 | requirement-analysis / task-generate / coding-process / code-review | 45+17+23+16 处 |
| 第2批 中密度 | proposal / coding-report / dr-generate / dr-review / dr-update | 20+12+15+13+12 处 |
| 第3批 低密度 | story-generate / story-review / story-update / testcase-generate / testcase-review / test-generate / test-review / agent-orchestration / project-assets-update | 22 处 |
| 第4批 契约文档 | document-storage-skill.md §4.0 新增 CLI 入口小节 | 1 节 |

### 规范化规则

| 问题 | 规范化后 |
|------|---------|
| `save_doc(intent="STORY", storyId, version={major,minor})` | `ae-sdd doc save --intent STORY --story-id {S} --content-file 草稿.md` |
| `resolve_path(intent="TASK", storyId, taskId)` | `ae-sdd doc resolve --intent TASK --story-id {S} --doc-id {taskId}` |
| 驼峰名 `storyId`/`taskId`/`raId` | 统一 CLI flag：`--story-id`/`--doc-id`（raId/prdId/drId/taskId 归一到 `--doc-id`）|
| 错误参数 `docType`/`title`/`doc={dict}` | 删除（intent 已含类型语义；content 走 --content-file）|
| 缺 ade_sdd/project_key | 不写（CLI 从 cwd 自动推导）|
| 📝 未实现 intent（STORY_SUPPLEMENT/DR_SUPPLEMENT 等）| 标注"手写 + `ae-sdd doc finalize` 补登记"降级路径 |

### 每个 SKILL 的"📦 文档存放前置调用"段统一改为 3 步 SOP

```
1. Write 草稿：用 Write 工具把内容写到 .ae-sdd/tmp/{doc-id}-draft.md
2. 存文档：ae-sdd doc save --intent X --content-file 草稿.md
3. 确认输出：记录最终路径到产出清单
```

### document-storage-skill.md §4.0 新增 CLI 入口

在 §4 API 契约开头新增 §4.0 小节，说明：
- 14 个 API 已封装为 `ae-sdd doc save/resolve/finalize` 三命令
- LLM/用户优先通过 CLI 调用
- §4.1-§4.11 保留为 Python 函数签名的代码层契约（供实现对齐）

## 验证

- ✅ 新 CLI 命令引用（`ae-sdd doc save/resolve/finalize`）：**180 处**已就位
- ✅ 旧式伪代码残留：仅 document-storage-skill.md §9.1 调用矩阵 13 处（合法保留——API 契约描述，非执行指令）
- ✅ 4 个测试套件 171 用例全过（test_document_storage 13 + test_cli_doc 8 + test_state 44 + test_gates 106）

## 架构变化

```
阶段2 前（文档契约与代码脱节）：
  SKILL 文档 save_doc(intent=...) → LLM 读伪代码 → 手拼路径 → Write
  （document_storage.py CLI 虽可用，但文档没教 LLM 怎么调）

阶段2 后（文档契约 = 代码入口）：
  SKILL 文档 ae-sdd doc save ... → LLM 执行命令 → document_storage.save_doc()
  → resolve + 写 + 版本 + ChangeLog + STORING + gitignore + 删草稿
  （LLM 只生成内容，路径/版本/索引全由代码负责）
```

## 后续阶段（可选）

- **阶段3**：PreToolUse hook（gate_intercept）升级，产物落地校验指向 `ae-sdd doc save`
- **阶段4**：gates.py G-DOC-STORAGE 门禁从硬编码合规根改为 `paths.resolve_doc_workspace()` 真值校验

## Sync

- 本次修改 `source/skills/` 下 20 个 SKILL 文档（不含代码改动）。
- `dev-sync` 需在 update-check 通过后执行。
